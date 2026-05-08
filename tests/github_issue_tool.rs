use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;
use symphony::Result;
use symphony::agent::tools::github_issue;
use symphony::events::Issue;
use symphony::tracker::Tracker;

#[tokio::test]
async fn github_issue_tool_routes_actions_to_tracker() {
    let tracker = Arc::new(FakeTracker::default());

    github_issue::execute(
        tracker.clone(),
        json!({
            "action": "comment",
            "issue_id": "I_1",
            "body": "working"
        }),
    )
    .await
    .unwrap();
    github_issue::execute(
        tracker.clone(),
        json!({
            "action": "set_state",
            "issue_id": "I_1",
            "state": "Human Review"
        }),
    )
    .await
    .unwrap();
    github_issue::execute(
        tracker.clone(),
        json!({
            "action": "link_pr",
            "issue_id": "I_1",
            "pr_number": 12
        }),
    )
    .await
    .unwrap();
    github_issue::execute(
        tracker.clone(),
        json!({
            "action": "close",
            "issue_id": "I_1"
        }),
    )
    .await
    .unwrap();

    assert_eq!(
        tracker.calls.lock().unwrap().as_slice(),
        [
            "comment:I_1:working",
            "set_state:I_1:Human Review",
            "link_pr:I_1:12",
            "close:I_1"
        ]
    );
}

#[derive(Default)]
struct FakeTracker {
    calls: Mutex<Vec<String>>,
}

#[async_trait]
impl Tracker for FakeTracker {
    async fn fetch_candidate_issues(&self) -> Result<Vec<Issue>> {
        Ok(vec![])
    }

    async fn fetch_issues_by_states(&self, _states: &[String]) -> Result<Vec<Issue>> {
        Ok(vec![])
    }

    async fn fetch_issue_states_by_ids(&self, _ids: &[String]) -> Result<HashMap<String, String>> {
        Ok(HashMap::new())
    }

    async fn comment(&self, issue_id: &str, body: &str) -> Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("comment:{issue_id}:{body}"));
        Ok(())
    }

    async fn set_state(&self, issue_id: &str, state: &str) -> Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("set_state:{issue_id}:{state}"));
        Ok(())
    }

    async fn close(&self, issue_id: &str) -> Result<()> {
        self.calls.lock().unwrap().push(format!("close:{issue_id}"));
        Ok(())
    }

    async fn link_pr(&self, issue_id: &str, pr_number: u64) -> Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("link_pr:{issue_id}:{pr_number}"));
        Ok(())
    }
}
