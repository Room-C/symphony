use std::collections::HashMap;

use crate::config::Config;
use crate::events::{Issue, OrchestratorState};

pub fn eligible_issues(
    state: &OrchestratorState,
    config: &Config,
    candidates: Vec<Issue>,
) -> Vec<Issue> {
    let mut counts_by_state = running_counts_by_state(state);
    let mut eligible = Vec::new();
    let mut available = config
        .agent
        .max_concurrent_agents
        .saturating_sub(state.running.len());
    for issue in candidates {
        if available == 0 {
            break;
        }
        if state.claimed.contains(&issue.id) || state.running.contains_key(&issue.id) {
            continue;
        }
        let normalized = issue.state.to_lowercase();
        if let Some(limit) = config.agent.max_concurrent_agents_by_state.get(&normalized) {
            let current = counts_by_state.get(&normalized).copied().unwrap_or(0);
            if current >= *limit {
                continue;
            }
        }
        *counts_by_state.entry(normalized).or_default() += 1;
        available -= 1;
        eligible.push(issue);
    }
    eligible
}

fn running_counts_by_state(state: &OrchestratorState) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for live in state.running.values() {
        *counts.entry(live.issue.state.to_lowercase()).or_default() += 1;
    }
    counts
}
