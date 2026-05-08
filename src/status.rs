use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::events::{CodexTotals, OrchestratorState, RetryEntry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusSnapshot {
    pub generated_at: DateTime<Utc>,
    pub counts: StatusCounts,
    pub running: Vec<RunningSnapshot>,
    pub retrying: Vec<RetryEntry>,
    pub codex_totals: CodexTotals,
    pub rate_limits: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusCounts {
    pub running: usize,
    pub retrying: usize,
    pub claimed: usize,
    pub completed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningSnapshot {
    pub issue_id: String,
    pub issue_identifier: String,
    pub state: String,
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_count: u32,
    pub last_event: Option<String>,
    pub started_at: DateTime<Utc>,
    pub last_event_at: Option<DateTime<Utc>>,
    pub tokens: TokenSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSnapshot {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

impl StatusSnapshot {
    pub fn from_state(state: &OrchestratorState) -> Self {
        let running = state
            .running
            .iter()
            .map(|(issue_id, live)| RunningSnapshot {
                issue_id: issue_id.clone(),
                issue_identifier: live.identifier.clone(),
                state: live.issue.state.clone(),
                session_id: live.session_id.clone(),
                thread_id: live.thread_id.clone(),
                turn_count: live.turn_count,
                last_event: live.last_codex_event.clone(),
                started_at: live.started_at,
                last_event_at: live.last_codex_timestamp,
                tokens: TokenSnapshot {
                    input_tokens: live.codex_input_tokens,
                    output_tokens: live.codex_output_tokens,
                    total_tokens: live.codex_total_tokens,
                },
            })
            .collect();
        Self {
            generated_at: Utc::now(),
            counts: StatusCounts {
                running: state.running.len(),
                retrying: state.retry_attempts.len(),
                claimed: state.claimed.len(),
                completed: state.completed.len(),
            },
            running,
            retrying: state.retry_attempts.values().cloned().collect(),
            codex_totals: state.codex_totals.clone(),
            rate_limits: state.codex_rate_limits.clone(),
        }
    }
}
