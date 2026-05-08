pub mod github;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::Result;
use crate::config::{TrackerConfig, TrackerMode};
use crate::events::Issue;

#[async_trait]
pub trait Tracker: Send + Sync {
    async fn fetch_candidate_issues(&self) -> Result<Vec<Issue>>;
    async fn fetch_issues_by_states(&self, states: &[String]) -> Result<Vec<Issue>>;
    async fn fetch_issue_states_by_ids(&self, ids: &[String]) -> Result<HashMap<String, String>>;

    async fn comment(&self, issue_id: &str, body: &str) -> Result<()>;
    async fn set_state(&self, issue_id: &str, state: &str) -> Result<()>;
    async fn close(&self, issue_id: &str) -> Result<()>;
    async fn link_pr(&self, issue_id: &str, pr_number: u64) -> Result<()>;
}

pub fn normalize_state(state: &str) -> String {
    state.to_lowercase()
}

pub fn sort_for_dispatch(issues: &mut [Issue]) {
    issues.sort_by(|left, right| {
        left.priority
            .unwrap_or(u8::MAX)
            .cmp(&right.priority.unwrap_or(u8::MAX))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| left.identifier.cmp(&right.identifier))
    });
}

pub fn build_tracker(config: &TrackerConfig) -> Result<Arc<dyn Tracker>> {
    match config.mode {
        TrackerMode::Labels => Ok(Arc::new(github::labels::GithubLabelsTracker::new(config)?)),
        TrackerMode::ProjectsV2 => Ok(Arc::new(github::projects_v2::GithubProjectsV2Tracker::new(
            config,
        )?)),
    }
}
