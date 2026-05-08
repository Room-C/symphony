use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Issue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub state: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub priority: Option<u8>,
    #[serde(default)]
    pub branch_name: Option<String>,
    pub url: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Issue {
    pub fn normalized_state(&self) -> String {
        self.state.to_lowercase()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunAttempt {
    pub issue_id: String,
    pub issue_identifier: String,
    pub attempt: Option<u32>,
    pub workspace_path: PathBuf,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LiveSession {
    pub issue: Issue,
    pub identifier: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub thread_id: Option<String>,
    pub turn_count: u32,
    pub retry_attempt: u32,
    pub started_at: DateTime<Utc>,
    #[serde(default)]
    pub last_codex_event: Option<String>,
    #[serde(default)]
    pub last_codex_timestamp: Option<DateTime<Utc>>,
    #[serde(default)]
    pub codex_input_tokens: u64,
    #[serde(default)]
    pub codex_output_tokens: u64,
    #[serde(default)]
    pub codex_total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetryEntry {
    pub issue_id: String,
    pub identifier: String,
    pub attempt: u32,
    pub due_at: DateTime<Utc>,
    pub delay_ms: u64,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CodexTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub seconds_running: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrchestratorState {
    pub running: HashMap<String, LiveSession>,
    pub claimed: HashSet<String>,
    pub retry_attempts: HashMap<String, RetryEntry>,
    pub completed: HashSet<String>,
    pub codex_totals: CodexTotals,
    #[serde(default)]
    pub codex_rate_limits: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeEvent {
    SessionStarted {
        issue_id: String,
        thread_id: String,
        at: DateTime<Utc>,
    },
    TurnCompleted {
        issue_id: String,
        thread_id: String,
        turn_id: String,
        at: DateTime<Utc>,
        input_tokens: u64,
        output_tokens: u64,
    },
    TurnFailed {
        issue_id: String,
        code: String,
        message: String,
        at: DateTime<Utc>,
    },
    TurnCancelled {
        issue_id: String,
        at: DateTime<Utc>,
    },
    TurnInputRequired {
        issue_id: String,
        at: DateTime<Utc>,
    },
    ApprovalAutoApproved {
        issue_id: String,
        at: DateTime<Utc>,
    },
    UnsupportedToolCall {
        issue_id: String,
        tool_name: String,
        at: DateTime<Utc>,
    },
    Notification {
        issue_id: String,
        message: String,
        at: DateTime<Utc>,
    },
    OtherMessage {
        issue_id: String,
        payload: serde_json::Value,
        at: DateTime<Utc>,
    },
    Malformed {
        issue_id: String,
        line: String,
        at: DateTime<Utc>,
    },
}

pub fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}
