use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time;
use tracing::{debug, warn};

use crate::agent::codex_protocol::{
    ClientRequest, ClientResponse, ServerMessage, extract_thread_id, extract_turn_id,
    github_issue_tool_spec,
};
use crate::agent::tools::github_issue;
use crate::agent::{AgentRunOutcome, AgentRunRequest, AgentRunner, EventSink};
use crate::error::{Result, SymphonyError};
use crate::events::RuntimeEvent;
use crate::path_safety::ensure_cwd;

#[derive(Debug, Default)]
pub struct CodexRunner;

#[async_trait]
impl AgentRunner for CodexRunner {
    async fn run(&self, request: AgentRunRequest, events: EventSink) -> Result<AgentRunOutcome> {
        ensure_cwd(&request.workspace_path, &request.workspace_path)
            .map_err(|error| SymphonyError::agent("invalid_workspace_cwd", error.to_string()))?;
        ensure_codex_available().await?;

        let mut session =
            CodexSession::start(&request.codex.command, &request.workspace_path).await?;
        let mut next_id = 1u64;
        session.send(ClientRequest::initialize(next_id)).await?;
        session
            .read_response(
                next_id,
                Duration::from_millis(request.codex.read_timeout_ms),
                &events,
                &request.issue.id,
                request.tracker.as_ref(),
            )
            .await?;

        next_id += 1;
        let cwd = request.workspace_path.to_string_lossy().to_string();
        session
            .send(ClientRequest::thread_start(
                next_id,
                &cwd,
                &request.codex.approval_policy,
                &request.codex.thread_sandbox,
                vec![github_issue_tool_spec()],
            ))
            .await?;
        let thread_result = session
            .read_response(
                next_id,
                Duration::from_millis(request.codex.read_timeout_ms),
                &events,
                &request.issue.id,
                request.tracker.as_ref(),
            )
            .await?;
        let thread_id = extract_thread_id(&thread_result).ok_or_else(|| {
            SymphonyError::agent(
                "response_error",
                "thread/start response did not include thread.id",
            )
        })?;
        events(RuntimeEvent::SessionStarted {
            issue_id: request.issue.id.clone(),
            thread_id: thread_id.clone(),
            at: Utc::now(),
        });

        let mut outcome = AgentRunOutcome {
            normal: true,
            thread_id: Some(thread_id.clone()),
            ..AgentRunOutcome::default()
        };

        for turn_index in 0..request.max_turns {
            let prompt = if turn_index == 0 {
                request.prompt.as_str()
            } else {
                request.continuation_prompt.as_str()
            };
            next_id += 1;
            session
                .send(ClientRequest::turn_start(
                    next_id,
                    &thread_id,
                    &cwd,
                    prompt,
                    &request.codex.approval_policy,
                ))
                .await?;
            let turn_result = session
                .read_response(
                    next_id,
                    Duration::from_millis(request.codex.read_timeout_ms),
                    &events,
                    &request.issue.id,
                    request.tracker.as_ref(),
                )
                .await?;
            let turn_id = extract_turn_id(&turn_result).ok_or_else(|| {
                SymphonyError::agent(
                    "response_error",
                    "turn/start response did not include turn.id",
                )
            })?;
            outcome.last_turn_id = Some(turn_id.clone());
            session
                .wait_for_turn_completion(
                    TurnWait {
                        turn_id: &turn_id,
                        turn_timeout: Duration::from_millis(request.codex.turn_timeout_ms),
                        stall_timeout_ms: request.codex.stall_timeout_ms,
                        events: &events,
                        issue_id: &request.issue.id,
                        thread_id: &thread_id,
                        tracker: request.tracker.as_ref(),
                    },
                    &mut outcome,
                )
                .await?;
            outcome.turn_count += 1;
        }

        session.shutdown().await;
        Ok(outcome)
    }
}

struct CodexSession {
    child: Child,
    stdin: ChildStdin,
    stdout: tokio::io::Lines<BufReader<ChildStdout>>,
}

impl CodexSession {
    async fn start(command: &str, cwd: &Path) -> Result<Self> {
        let mut child = Command::new("bash")
            .arg("-lc")
            .arg(command)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| SymphonyError::agent("port_exit", error.to_string()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| SymphonyError::agent("port_exit", "failed to open codex stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SymphonyError::agent("port_exit", "failed to open codex stdout"))?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
        })
    }

    async fn send(&mut self, request: ClientRequest) -> Result<()> {
        let line = serde_json::to_string(&request)?;
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn send_response(&mut self, response: ClientResponse) -> Result<()> {
        let line = serde_json::to_string(&response)?;
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn read_response(
        &mut self,
        id: u64,
        timeout: Duration,
        events: &EventSink,
        issue_id: &str,
        tracker: Option<&std::sync::Arc<dyn crate::tracker::Tracker>>,
    ) -> Result<serde_json::Value> {
        let deadline = time::Instant::now() + timeout;
        loop {
            let now = time::Instant::now();
            if now >= deadline {
                return Err(SymphonyError::agent(
                    "response_timeout",
                    format!("timed out waiting for response {id}"),
                ));
            }
            let line = time::timeout_at(deadline, self.stdout.next_line())
                .await
                .map_err(|_| {
                    SymphonyError::agent(
                        "response_timeout",
                        format!("timed out waiting for response {id}"),
                    )
                })??;
            let Some(line) = line else {
                return Err(SymphonyError::agent(
                    "port_exit",
                    "codex app-server stdout closed",
                ));
            };
            let message = parse_message(&line, events, issue_id)?;
            if message.method.is_some() && message.id.is_some() {
                self.handle_server_request(message, events, issue_id, tracker)
                    .await?;
                continue;
            }
            if message.id.as_ref() == Some(&serde_json::Value::from(id)) {
                if let Some(error) = message.error {
                    return Err(SymphonyError::agent(
                        "response_error",
                        format!("{} ({:?})", error.message, error.code),
                    ));
                }
                return Ok(message.result.unwrap_or_default());
            }
            handle_notification(message, events, issue_id, None, None)?;
        }
    }

    async fn wait_for_turn_completion(
        &mut self,
        wait: TurnWait<'_>,
        outcome: &mut AgentRunOutcome,
    ) -> Result<()> {
        let turn_deadline = time::Instant::now() + wait.turn_timeout;
        let stall = (wait.stall_timeout_ms > 0)
            .then(|| Duration::from_millis(wait.stall_timeout_ms as u64));
        loop {
            let next_deadline = stall
                .map(|duration| (time::Instant::now() + duration).min(turn_deadline))
                .unwrap_or(turn_deadline);
            let line = time::timeout_at(next_deadline, self.stdout.next_line())
                .await
                .map_err(|_| {
                    if time::Instant::now() >= turn_deadline {
                        SymphonyError::agent(
                            "turn_timeout",
                            format!("turn {} timed out", wait.turn_id),
                        )
                    } else {
                        SymphonyError::agent(
                            "turn_timeout",
                            format!("turn {} stalled", wait.turn_id),
                        )
                    }
                })??;
            let Some(line) = line else {
                return Err(SymphonyError::agent(
                    "port_exit",
                    "codex app-server stdout closed",
                ));
            };
            let message = parse_message(&line, wait.events, wait.issue_id)?;
            if message.method.is_some() && message.id.is_some() {
                self.handle_server_request(message, wait.events, wait.issue_id, wait.tracker)
                    .await?;
                continue;
            }
            if handle_notification(
                message,
                wait.events,
                wait.issue_id,
                Some(wait.thread_id),
                Some(outcome),
            )? == TurnSignal::Completed(wait.turn_id.to_string())
            {
                return Ok(());
            }
        }
    }

    async fn shutdown(mut self) {
        if let Err(error) = self.stdin.shutdown().await {
            debug!(%error, "failed to shutdown codex stdin");
        }
        if let Err(error) = self.child.kill().await {
            warn!(%error, "failed to stop codex app-server");
        }
    }

    async fn handle_server_request(
        &mut self,
        message: ServerMessage,
        events: &EventSink,
        issue_id: &str,
        tracker: Option<&std::sync::Arc<dyn crate::tracker::Tracker>>,
    ) -> Result<()> {
        let id = message
            .id
            .clone()
            .ok_or_else(|| SymphonyError::agent("response_error", "server request missing id"))?;
        let method = message.method.as_deref().unwrap_or_default();
        let result = match method {
            "item/tool/call" => {
                self.handle_dynamic_tool_call(&message, events, issue_id, tracker)
                    .await
            }
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
                events(RuntimeEvent::ApprovalAutoApproved {
                    issue_id: issue_id.to_string(),
                    at: Utc::now(),
                });
                serde_json::json!({ "decision": "acceptForSession" })
            }
            "item/tool/requestUserInput" => {
                events(RuntimeEvent::TurnInputRequired {
                    issue_id: issue_id.to_string(),
                    at: Utc::now(),
                });
                serde_json::json!({ "answer": { "type": "cancel" } })
            }
            other => {
                events(RuntimeEvent::UnsupportedToolCall {
                    issue_id: issue_id.to_string(),
                    tool_name: other.to_string(),
                    at: Utc::now(),
                });
                serde_json::json!({})
            }
        };
        self.send_response(ClientResponse::ok(id, result)).await
    }

    async fn handle_dynamic_tool_call(
        &mut self,
        message: &ServerMessage,
        events: &EventSink,
        issue_id: &str,
        tracker: Option<&std::sync::Arc<dyn crate::tracker::Tracker>>,
    ) -> serde_json::Value {
        let tool = message
            .params
            .get("tool")
            .and_then(serde_json::Value::as_str);
        if tool != Some("github_issue") {
            let tool_name = tool.unwrap_or("unknown").to_string();
            events(RuntimeEvent::UnsupportedToolCall {
                issue_id: issue_id.to_string(),
                tool_name: tool_name.clone(),
                at: Utc::now(),
            });
            return dynamic_tool_result(
                false,
                serde_json::json!({
                    "error": {
                        "code": "unsupported_tool_call",
                        "message": format!("unsupported dynamic tool {tool_name:?}")
                    }
                }),
            );
        }
        let Some(tracker) = tracker else {
            return dynamic_tool_result(
                false,
                serde_json::json!({
                    "error": {
                        "code": "missing_tracker",
                        "message": "github_issue tool requires an attached tracker"
                    }
                }),
            );
        };
        let input = message.params.get("arguments").cloned().unwrap_or_default();
        match github_issue::execute((*tracker).clone(), input).await {
            Ok(value) => dynamic_tool_result(true, value),
            Err(error) => dynamic_tool_result(
                false,
                serde_json::json!({
                    "error": {
                        "code": "github_issue_tool_failed",
                        "message": error.to_string()
                    }
                }),
            ),
        }
    }
}

struct TurnWait<'a> {
    turn_id: &'a str,
    turn_timeout: Duration,
    stall_timeout_ms: i64,
    events: &'a EventSink,
    issue_id: &'a str,
    thread_id: &'a str,
    tracker: Option<&'a std::sync::Arc<dyn crate::tracker::Tracker>>,
}

fn dynamic_tool_result(success: bool, payload: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "success": success,
        "contentItems": [
            {
                "type": "inputText",
                "text": payload.to_string()
            }
        ]
    })
}

#[derive(Debug, PartialEq, Eq)]
enum TurnSignal {
    None,
    Completed(String),
}

fn parse_message(line: &str, events: &EventSink, issue_id: &str) -> Result<ServerMessage> {
    serde_json::from_str::<ServerMessage>(line).map_err(|error| {
        events(RuntimeEvent::Malformed {
            issue_id: issue_id.to_string(),
            line: line.to_string(),
            at: Utc::now(),
        });
        SymphonyError::agent("response_error", error.to_string())
    })
}

fn handle_notification(
    message: ServerMessage,
    events: &EventSink,
    issue_id: &str,
    thread_id: Option<&str>,
    outcome: Option<&mut AgentRunOutcome>,
) -> Result<TurnSignal> {
    let Some(method) = message.method.as_deref() else {
        return Ok(TurnSignal::None);
    };
    match method {
        "turn/completed" => {
            let turn_id = message
                .params
                .pointer("/turn/id")
                .and_then(serde_json::Value::as_str)
                .or_else(|| {
                    message
                        .params
                        .get("turnId")
                        .and_then(serde_json::Value::as_str)
                })
                .unwrap_or_default()
                .to_string();
            let input_tokens = message
                .params
                .pointer("/usage/input_tokens")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let output_tokens = message
                .params
                .pointer("/usage/output_tokens")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            if let Some(outcome) = outcome {
                outcome.input_tokens += input_tokens;
                outcome.output_tokens += output_tokens;
                outcome.total_tokens += input_tokens + output_tokens;
            }
            events(RuntimeEvent::TurnCompleted {
                issue_id: issue_id.to_string(),
                thread_id: thread_id.unwrap_or_default().to_string(),
                turn_id: turn_id.clone(),
                at: Utc::now(),
                input_tokens,
                output_tokens,
            });
            Ok(TurnSignal::Completed(turn_id))
        }
        "turn/failed" => Err(SymphonyError::agent(
            "turn_failed",
            message.params.to_string(),
        )),
        "turn/cancelled" => Err(SymphonyError::agent(
            "turn_cancelled",
            message.params.to_string(),
        )),
        "tool/requestUserInput" | "tool/userInputRequired" => {
            events(RuntimeEvent::TurnInputRequired {
                issue_id: issue_id.to_string(),
                at: Utc::now(),
            });
            Err(SymphonyError::agent(
                "turn_input_required",
                "agent requested user input",
            ))
        }
        other => {
            events(RuntimeEvent::OtherMessage {
                issue_id: issue_id.to_string(),
                payload: serde_json::json!({ "method": other, "params": message.params }),
                at: Utc::now(),
            });
            Ok(TurnSignal::None)
        }
    }
}

async fn ensure_codex_available() -> Result<()> {
    let status = Command::new("bash")
        .arg("-lc")
        .arg("command -v codex >/dev/null 2>&1")
        .status()
        .await?;
    if status.success() {
        Ok(())
    } else {
        Err(SymphonyError::agent(
            "codex_not_found",
            "codex binary not found in PATH",
        ))
    }
}
