use std::sync::Arc;

use tokio::sync::RwLock;

use crate::agent::EventSink;
use crate::events::{OrchestratorState, RuntimeEvent};

pub fn assert_state_invariants(state: &OrchestratorState) -> bool {
    state.running.keys().all(|id| state.claimed.contains(id))
        && state
            .retry_attempts
            .keys()
            .all(|id| state.claimed.contains(id))
}

pub fn live_event_sink(state: Arc<RwLock<OrchestratorState>>) -> EventSink {
    Arc::new(move |event| {
        let state = state.clone();
        tokio::spawn(async move {
            apply_runtime_event(&state, event).await;
        });
    })
}

async fn apply_runtime_event(state: &Arc<RwLock<OrchestratorState>>, event: RuntimeEvent) {
    let mut guard = state.write().await;
    match event {
        RuntimeEvent::SessionStarted {
            issue_id,
            thread_id,
            at,
        } => {
            if let Some(live) = guard.running.get_mut(&issue_id) {
                live.thread_id = Some(thread_id);
                live.last_codex_event = Some("session_started".to_string());
                live.last_codex_timestamp = Some(at);
            }
        }
        RuntimeEvent::TurnCompleted {
            issue_id,
            thread_id,
            turn_id,
            at,
            input_tokens,
            output_tokens,
        } => {
            if let Some(live) = guard.running.get_mut(&issue_id) {
                live.thread_id = Some(thread_id.clone());
                live.session_id = Some(format!("{thread_id}-{turn_id}"));
                live.turn_count = live.turn_count.saturating_add(1);
                live.codex_input_tokens = live.codex_input_tokens.saturating_add(input_tokens);
                live.codex_output_tokens = live.codex_output_tokens.saturating_add(output_tokens);
                live.codex_total_tokens = live
                    .codex_input_tokens
                    .saturating_add(live.codex_output_tokens);
                live.last_codex_event = Some("turn_completed".to_string());
                live.last_codex_timestamp = Some(at);
            }
        }
        RuntimeEvent::TurnFailed {
            issue_id,
            code,
            message,
            at,
        } => set_last_event(
            &mut guard,
            &issue_id,
            format!("turn_failed:{code}:{message}"),
            at,
        ),
        RuntimeEvent::TurnCancelled { issue_id, at } => {
            set_last_event(&mut guard, &issue_id, "turn_cancelled", at)
        }
        RuntimeEvent::TurnInputRequired { issue_id, at } => {
            set_last_event(&mut guard, &issue_id, "turn_input_required", at)
        }
        RuntimeEvent::ApprovalAutoApproved { issue_id, at } => {
            set_last_event(&mut guard, &issue_id, "approval_auto_approved", at)
        }
        RuntimeEvent::UnsupportedToolCall {
            issue_id,
            tool_name,
            at,
        } => set_last_event(
            &mut guard,
            &issue_id,
            format!("unsupported_tool_call:{tool_name}"),
            at,
        ),
        RuntimeEvent::Notification {
            issue_id,
            message,
            at,
        } => set_last_event(&mut guard, &issue_id, message, at),
        RuntimeEvent::OtherMessage { issue_id, at, .. } => {
            set_last_event(&mut guard, &issue_id, "other_message", at)
        }
        RuntimeEvent::Malformed { issue_id, at, .. } => {
            set_last_event(&mut guard, &issue_id, "malformed_message", at)
        }
    }
}

fn set_last_event(
    state: &mut OrchestratorState,
    issue_id: &str,
    message: impl Into<String>,
    at: chrono::DateTime<chrono::Utc>,
) {
    if let Some(live) = state.running.get_mut(issue_id) {
        live.last_codex_event = Some(message.into());
        live.last_codex_timestamp = Some(at);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::Utc;

    use super::*;
    use crate::events::{Issue, LiveSession};

    #[tokio::test]
    async fn live_event_sink_records_session_and_turn_updates() {
        let issue = issue("I_1");
        let state = Arc::new(RwLock::new(OrchestratorState::default()));
        {
            let mut guard = state.write().await;
            guard.running.insert(
                issue.id.clone(),
                LiveSession {
                    identifier: issue.identifier.clone(),
                    issue: issue.clone(),
                    session_id: None,
                    thread_id: None,
                    turn_count: 0,
                    retry_attempt: 0,
                    started_at: Utc::now(),
                    last_codex_event: None,
                    last_codex_timestamp: None,
                    codex_input_tokens: 0,
                    codex_output_tokens: 0,
                    codex_total_tokens: 0,
                },
            );
        }

        let sink = live_event_sink(state.clone());
        sink(RuntimeEvent::SessionStarted {
            issue_id: issue.id.clone(),
            thread_id: "thread-1".to_string(),
            at: Utc::now(),
        });
        wait_until(&state, |live| live.thread_id.as_deref() == Some("thread-1")).await;

        sink(RuntimeEvent::TurnCompleted {
            issue_id: issue.id.clone(),
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            at: Utc::now(),
            input_tokens: 11,
            output_tokens: 7,
        });
        wait_until(&state, |live| live.turn_count == 1).await;

        let guard = state.read().await;
        let live = guard.running.get(&issue.id).unwrap();
        assert_eq!(live.thread_id.as_deref(), Some("thread-1"));
        assert_eq!(live.session_id.as_deref(), Some("thread-1-turn-1"));
        assert_eq!(live.turn_count, 1);
        assert_eq!(live.codex_input_tokens, 11);
        assert_eq!(live.codex_output_tokens, 7);
        assert_eq!(live.codex_total_tokens, 18);
        assert_eq!(live.last_codex_event.as_deref(), Some("turn_completed"));
        assert!(live.last_codex_timestamp.is_some());
    }

    async fn wait_until(
        state: &Arc<RwLock<OrchestratorState>>,
        predicate: impl Fn(&LiveSession) -> bool,
    ) {
        for _ in 0..20 {
            {
                let guard = state.read().await;
                if guard.running.get("I_1").is_some_and(|live| predicate(live)) {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for live event sink");
    }

    fn issue(id: &str) -> Issue {
        Issue {
            id: id.to_string(),
            identifier: "Room-C/symphony#1".to_string(),
            title: "Test issue".to_string(),
            state: "In Progress".to_string(),
            description: None,
            priority: None,
            branch_name: None,
            url: "https://github.com/Room-C/symphony/issues/1".to_string(),
            labels: Vec::new(),
            blocked_by: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
