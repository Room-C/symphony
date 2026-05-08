pub mod dispatch;
pub mod reconcile;
pub mod state;

use std::sync::Arc;
use std::time::Duration;

use chrono::{TimeDelta, Utc};
use tokio::sync::RwLock;
use tokio::time;
use tracing::{debug, error, info, warn};

use crate::agent::{AgentRunRequest, AgentRunner, EventSink, noop_event_sink};
use crate::events::{CodexTotals, Issue, LiveSession, OrchestratorState, RetryEntry};
use crate::prompt::render_prompt;
use crate::retry::{continuation_delay, next_backoff};
use crate::status::StatusSnapshot;
use crate::tracker::{Tracker, sort_for_dispatch};
use crate::workflow_store::WorkflowStore;
use crate::workspace::WorkspaceManager;
use crate::{Result, SymphonyError};

#[derive(Clone)]
pub struct Orchestrator {
    workflow_store: WorkflowStore,
    tracker: Arc<dyn Tracker>,
    agent: Arc<dyn AgentRunner>,
    state: Arc<RwLock<OrchestratorState>>,
    events: EventSink,
}

impl Orchestrator {
    pub fn new(
        workflow_store: WorkflowStore,
        tracker: Arc<dyn Tracker>,
        agent: Arc<dyn AgentRunner>,
    ) -> Self {
        Self {
            workflow_store,
            tracker,
            agent,
            state: Arc::new(RwLock::new(OrchestratorState::default())),
            events: noop_event_sink(),
        }
    }

    pub fn with_events(mut self, events: EventSink) -> Self {
        self.events = events;
        self
    }

    pub fn state(&self) -> Arc<RwLock<OrchestratorState>> {
        self.state.clone()
    }

    pub async fn startup_cleanup(&self) -> Result<()> {
        let workflow = self.workflow_store.snapshot().await;
        let terminal = workflow.config.tracker.terminal_states.clone();
        match self.tracker.fetch_issues_by_states(&terminal).await {
            Ok(issues) => {
                let root = workflow.config.workspace.root.clone();
                let manager = WorkspaceManager::new(root.clone(), workflow.config.hooks);
                for issue in issues {
                    let key = crate::path_safety::sanitize_workspace_key(&issue.identifier);
                    let path = root.join(key);
                    manager.before_remove_best_effort(&path).await;
                    if let Err(error) = tokio::fs::remove_dir_all(&path).await
                        && error.kind() != std::io::ErrorKind::NotFound
                    {
                        warn!(%error, path = %path.display(), "terminal workspace cleanup failed");
                    }
                }
                Ok(())
            }
            Err(error) => {
                warn!(%error, "startup terminal cleanup failed; continuing");
                Ok(())
            }
        }
    }

    pub async fn run_until_shutdown(&self) -> Result<()> {
        self.workflow_store.snapshot().await.config.validate()?;
        self.startup_cleanup().await?;
        self.tick().await?;
        loop {
            let interval_ms = self
                .workflow_store
                .snapshot()
                .await
                .config
                .polling
                .interval_ms;
            tokio::select! {
                _ = time::sleep(Duration::from_millis(interval_ms)) => self.tick().await?,
                _ = tokio::signal::ctrl_c() => {
                    info!("shutdown signal received");
                    return Ok(());
                }
            }
        }
    }

    pub async fn tick(&self) -> Result<()> {
        self.workflow_store.reload().await.ok();
        let workflow = self.workflow_store.snapshot().await;
        if let Err(error) = workflow.config.validate() {
            error!(%error, "dispatch preflight validation failed");
            return Ok(());
        }
        reconcile::reconcile_running_issues(&self.state, self.tracker.clone(), &workflow.config)
            .await;
        self.dispatch_due_retries().await;
        let mut candidates = match self.tracker.fetch_candidate_issues().await {
            Ok(issues) => issues,
            Err(error) => {
                error!(%error, "candidate issue fetch failed; skipping tick");
                return Ok(());
            }
        };
        debug!(
            candidate_count = candidates.len(),
            "candidate issue fetch completed"
        );
        sort_for_dispatch(&mut candidates);
        let guard = self.state.read().await;
        let eligible = dispatch::eligible_issues(&guard, &workflow.config, candidates);
        drop(guard);
        for issue in eligible {
            self.dispatch_issue(issue, None).await?;
        }
        Ok(())
    }

    async fn dispatch_due_retries(&self) {
        let due: Vec<RetryEntry> = {
            let mut state = self.state.write().await;
            let now = Utc::now();
            let ids: Vec<_> = state
                .retry_attempts
                .iter()
                .filter_map(|(id, retry)| (retry.due_at <= now).then_some(id.clone()))
                .collect();
            ids.into_iter()
                .filter_map(|id| state.retry_attempts.remove(&id))
                .collect()
        };
        if due.is_empty() {
            return;
        }
        let candidates = match self.tracker.fetch_candidate_issues().await {
            Ok(issues) => issues,
            Err(error) => {
                let workflow = self.workflow_store.snapshot().await;
                let mut state = self.state.write().await;
                for retry in due {
                    let delay = next_backoff(
                        retry.attempt + 1,
                        workflow.config.agent.max_retry_backoff_ms,
                    );
                    state.retry_attempts.insert(
                        retry.issue_id.clone(),
                        make_retry_entry(
                            retry.issue_id,
                            retry.identifier,
                            retry.attempt + 1,
                            delay,
                            Some(format!("retry poll failed: {error}")),
                        ),
                    );
                }
                return;
            }
        };
        for retry in due {
            if let Some(issue) = candidates
                .iter()
                .find(|issue| issue.id == retry.issue_id)
                .cloned()
            {
                if self
                    .dispatch_issue(issue, Some(retry.attempt))
                    .await
                    .is_err()
                {
                    warn!(issue_id = retry.issue_id, "retry dispatch failed");
                }
            } else {
                self.state.write().await.claimed.remove(&retry.issue_id);
            }
        }
    }

    pub async fn dispatch_issue(&self, issue: Issue, attempt: Option<u32>) -> Result<()> {
        let workflow = self.workflow_store.snapshot().await;
        {
            let mut state = self.state.write().await;
            if state.claimed.contains(&issue.id) || state.running.contains_key(&issue.id) {
                return Ok(());
            }
            if state.running.len() >= workflow.config.agent.max_concurrent_agents {
                return Err(SymphonyError::config(
                    "no_available_orchestrator_slots",
                    "max_concurrent_agents reached",
                ));
            }
            state.claimed.insert(issue.id.clone());
            state.retry_attempts.remove(&issue.id);
            state.running.insert(
                issue.id.clone(),
                LiveSession {
                    identifier: issue.identifier.clone(),
                    issue: issue.clone(),
                    session_id: None,
                    thread_id: None,
                    turn_count: 0,
                    retry_attempt: attempt.unwrap_or(0),
                    started_at: Utc::now(),
                    last_codex_event: None,
                    last_codex_timestamp: None,
                    codex_input_tokens: 0,
                    codex_output_tokens: 0,
                    codex_total_tokens: 0,
                },
            );
        }
        info!(issue_id = %issue.id, issue = %issue.identifier, "dispatching issue");

        let state = self.state.clone();
        let tracker = self.tracker.clone();
        let agent = self.agent.clone();
        let events = self.events.clone();
        tokio::spawn(async move {
            let result = run_worker_attempt(
                workflow,
                issue.clone(),
                attempt,
                tracker,
                agent,
                state.clone(),
                events.clone(),
            )
            .await;
            on_worker_exit(state, issue.id.clone(), result).await;
        });
        Ok(())
    }

    pub async fn snapshot(&self) -> StatusSnapshot {
        let guard = self.state.read().await;
        StatusSnapshot::from_state(&guard)
    }
}

async fn run_worker_attempt(
    workflow: crate::workflow::Workflow,
    issue: Issue,
    attempt: Option<u32>,
    tracker: Arc<dyn Tracker>,
    agent: Arc<dyn AgentRunner>,
    state: Arc<RwLock<OrchestratorState>>,
    events: EventSink,
) -> Result<crate::agent::AgentRunOutcome> {
    let manager = WorkspaceManager::new(
        workflow.config.workspace.root.clone(),
        workflow.config.hooks.clone(),
    );
    let workspace = manager.prepare(&issue.identifier).await?;
    manager.before_run(&workspace).await?;
    let prompt = render_prompt(&workflow.prompt_template, &issue, attempt)?;
    let continuation_prompt = format!(
        "Continue work on {}. Use the current workspace state; do not restart from scratch.",
        issue.identifier
    );
    let request = AgentRunRequest {
        issue: issue.clone(),
        workspace_path: workspace.path.clone(),
        prompt,
        continuation_prompt,
        attempt,
        max_turns: workflow.config.agent.max_turns,
        codex: workflow.config.codex.clone(),
        tracker: Some(tracker.clone()),
    };
    let outcome = agent.run(request, events).await;
    manager.after_run_best_effort(&workspace).await;
    if let Ok(outcome) = &outcome
        && let Some(thread_id) = &outcome.thread_id
    {
        let mut guard = state.write().await;
        if let Some(live) = guard.running.get_mut(&issue.id) {
            live.thread_id = Some(thread_id.clone());
            live.session_id = outcome
                .last_turn_id
                .as_ref()
                .map(|turn_id| format!("{thread_id}-{turn_id}"));
            live.turn_count = outcome.turn_count;
            live.codex_input_tokens = outcome.input_tokens;
            live.codex_output_tokens = outcome.output_tokens;
            live.codex_total_tokens = outcome.total_tokens;
        }
    }
    let refreshed = tracker
        .fetch_issue_states_by_ids(std::slice::from_ref(&issue.id))
        .await?;
    if let Some(state_value) = refreshed.get(&issue.id) {
        debug!(issue_id = issue.id, state = %state_value, "worker exit state refresh");
    }
    outcome
}

async fn on_worker_exit(
    state: Arc<RwLock<OrchestratorState>>,
    issue_id: String,
    result: Result<crate::agent::AgentRunOutcome>,
) {
    let mut guard = state.write().await;
    let Some(live) = guard.running.remove(&issue_id) else {
        return;
    };
    let elapsed = (Utc::now() - live.started_at)
        .to_std()
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    guard.codex_totals = CodexTotals {
        input_tokens: guard.codex_totals.input_tokens + live.codex_input_tokens,
        output_tokens: guard.codex_totals.output_tokens + live.codex_output_tokens,
        total_tokens: guard.codex_totals.total_tokens + live.codex_total_tokens,
        seconds_running: guard.codex_totals.seconds_running + elapsed,
    };
    match result {
        Ok(_) => {
            guard.completed.insert(issue_id.clone());
            guard.retry_attempts.insert(
                issue_id.clone(),
                make_retry_entry(
                    issue_id.clone(),
                    live.identifier,
                    1,
                    continuation_delay(),
                    None,
                ),
            );
        }
        Err(error) => {
            let next_attempt = live.retry_attempt.saturating_add(1).max(1);
            let delay = next_backoff(next_attempt, 300_000);
            guard.retry_attempts.insert(
                issue_id.clone(),
                make_retry_entry(
                    issue_id.clone(),
                    live.identifier,
                    next_attempt,
                    delay,
                    Some(error.to_string()),
                ),
            );
        }
    }
}

fn make_retry_entry(
    issue_id: String,
    identifier: String,
    attempt: u32,
    delay: Duration,
    error: Option<String>,
) -> RetryEntry {
    let delay_ms = delay.as_millis().min(u128::from(u64::MAX)) as u64;
    RetryEntry {
        issue_id,
        identifier,
        attempt,
        due_at: Utc::now() + TimeDelta::milliseconds(delay_ms.min(i64::MAX as u64) as i64),
        delay_ms,
        error,
    }
}
