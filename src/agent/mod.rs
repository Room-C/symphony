pub mod codex;
pub mod codex_protocol;
pub mod tools;

use std::path::PathBuf;

use async_trait::async_trait;

use crate::Result;
use crate::config::CodexConfig;
use crate::events::{Issue, RuntimeEvent};

#[derive(Debug, Clone)]
pub struct AgentRunRequest {
    pub issue: Issue,
    pub workspace_path: PathBuf,
    pub prompt: String,
    pub continuation_prompt: String,
    pub attempt: Option<u32>,
    pub max_turns: u32,
    pub codex: CodexConfig,
}

#[derive(Debug, Clone, Default)]
pub struct AgentRunOutcome {
    pub normal: bool,
    pub turn_count: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub thread_id: Option<String>,
    pub last_turn_id: Option<String>,
}

#[async_trait]
pub trait AgentRunner: Send + Sync {
    async fn run(&self, request: AgentRunRequest, events: EventSink) -> Result<AgentRunOutcome>;
}

pub type EventSink = std::sync::Arc<dyn Fn(RuntimeEvent) + Send + Sync>;

pub fn noop_event_sink() -> EventSink {
    std::sync::Arc::new(|_| {})
}

#[derive(Debug, Default)]
pub struct NoopAgentRunner;

#[async_trait]
impl AgentRunner for NoopAgentRunner {
    async fn run(&self, _request: AgentRunRequest, _events: EventSink) -> Result<AgentRunOutcome> {
        Ok(AgentRunOutcome {
            normal: true,
            turn_count: 1,
            ..AgentRunOutcome::default()
        })
    }
}
