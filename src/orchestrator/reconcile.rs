use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::config::Config;
use crate::events::OrchestratorState;
use crate::tracker::Tracker;

pub async fn reconcile_running_issues(
    state: &Arc<RwLock<OrchestratorState>>,
    tracker: Arc<dyn Tracker>,
    config: &Config,
) {
    let running_ids: Vec<String> = state.read().await.running.keys().cloned().collect();
    if running_ids.is_empty() {
        return;
    }
    let refreshed = match tracker.fetch_issue_states_by_ids(&running_ids).await {
        Ok(states) => states,
        Err(error) => {
            debug!(%error, "running state refresh failed; keeping workers running");
            return;
        }
    };
    let active = config.active_state_set();
    let terminal = config.terminal_state_set();
    let mut guard = state.write().await;
    for issue_id in running_ids {
        let Some(state_value) = refreshed.get(&issue_id) else {
            continue;
        };
        let normalized = state_value.to_lowercase();
        if terminal.contains(&normalized) || !active.contains(&normalized) {
            warn!(issue_id, state = %state_value, "issue no longer active; releasing claim");
            guard.running.remove(&issue_id);
            guard.claimed.remove(&issue_id);
            guard.retry_attempts.remove(&issue_id);
        } else if let Some(live) = guard.running.get_mut(&issue_id) {
            live.issue.state = state_value.clone();
        }
    }
}
