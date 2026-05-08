use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::{Result, SymphonyError};
use crate::tracker::Tracker;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GithubIssueToolInput {
    pub action: String,
    pub issue_id: String,
    pub body: Option<String>,
    pub state: Option<String>,
    pub pr_number: Option<u64>,
}

pub async fn execute(tracker: Arc<dyn Tracker>, input: Value) -> Result<Value> {
    let input: GithubIssueToolInput = serde_json::from_value(input)?;
    match input.action.as_str() {
        "comment" => {
            let body = input.body.ok_or_else(|| {
                SymphonyError::agent("invalid_tool_input", "comment requires body")
            })?;
            tracker.comment(&input.issue_id, &body).await?;
        }
        "set_state" => {
            let state = input.state.ok_or_else(|| {
                SymphonyError::agent("invalid_tool_input", "set_state requires state")
            })?;
            tracker.set_state(&input.issue_id, &state).await?;
        }
        "close" => tracker.close(&input.issue_id).await?,
        "link_pr" => {
            let pr_number = input.pr_number.ok_or_else(|| {
                SymphonyError::agent("invalid_tool_input", "link_pr requires pr_number")
            })?;
            tracker.link_pr(&input.issue_id, pr_number).await?;
        }
        other => {
            return Err(SymphonyError::agent(
                "invalid_tool_input",
                format!("unsupported github_issue action {other:?}"),
            ));
        }
    }
    Ok(json!({ "success": true }))
}
