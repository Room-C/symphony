use crate::events::OrchestratorState;

pub fn assert_state_invariants(state: &OrchestratorState) -> bool {
    state.running.keys().all(|id| state.claimed.contains(id))
        && state
            .retry_attempts
            .keys()
            .all(|id| state.claimed.contains(id))
}
