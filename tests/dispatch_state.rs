use chrono::Utc;
use symphony::config::Config;
use symphony::events::{Issue, LiveSession, OrchestratorState};
use symphony::orchestrator::dispatch::eligible_issues;
use symphony::orchestrator::state::assert_state_invariants;

#[test]
fn dispatch_respects_global_and_per_state_quota() {
    let mut config = Config::default();
    config.agent.max_concurrent_agents = 2;
    config
        .agent
        .max_concurrent_agents_by_state
        .insert("todo".to_string(), 1);

    let mut state = OrchestratorState::default();
    let running = issue("1", "Todo", 1);
    state.claimed.insert(running.id.clone());
    state.running.insert(
        running.id.clone(),
        LiveSession {
            issue: running.clone(),
            identifier: running.identifier.clone(),
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

    let eligible = eligible_issues(
        &state,
        &config,
        vec![issue("2", "Todo", 2), issue("3", "In Progress", 1)],
    );

    assert_eq!(eligible.len(), 1);
    assert_eq!(eligible[0].id, "3");
    assert!(assert_state_invariants(&state));
}

fn issue(id: &str, state: &str, priority: u8) -> Issue {
    Issue {
        id: id.to_string(),
        identifier: format!("Room-C/symphony#{id}"),
        title: format!("Issue {id}"),
        state: state.to_string(),
        description: None,
        priority: Some(priority),
        branch_name: None,
        url: format!("https://github.com/Room-C/symphony/issues/{id}"),
        labels: vec![],
        blocked_by: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
