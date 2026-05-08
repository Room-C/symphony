use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::config::TrackerConfig;
use crate::error::{Result, SymphonyError};
use crate::events::Issue;
use crate::tracker::Tracker;

#[derive(Debug, Clone)]
pub struct GithubProjectsV2Tracker {
    client: reqwest::Client,
    endpoint: String,
    token: String,
    org: String,
    project_number: u64,
    status_field: String,
    active_states: Vec<String>,
}

impl GithubProjectsV2Tracker {
    pub fn new(config: &TrackerConfig) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            endpoint: graphql_endpoint(&config.endpoint),
            token: config.api_key.clone().ok_or_else(|| {
                SymphonyError::tracker("missing_tracker_api_key", "tracker.api_key is required")
            })?,
            org: config.org.clone().ok_or_else(|| {
                SymphonyError::tracker("missing_tracker_org", "tracker.org is required")
            })?,
            project_number: config.project_number.ok_or_else(|| {
                SymphonyError::tracker(
                    "missing_tracker_project_number",
                    "tracker.project_number is required",
                )
            })?,
            status_field: config.status_field.clone(),
            active_states: config.active_states.clone(),
        })
    }

    async fn graphql(&self, query: &str, variables: Value) -> Result<Value> {
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "Room-C-Symphony")
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
            .map_err(|error| SymphonyError::tracker("github_api_request", error.to_string()))?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(|error| SymphonyError::tracker("github_api_request", error.to_string()))?;
        if !status.is_success() {
            return Err(SymphonyError::tracker(
                "github_api_status",
                format!("GitHub GraphQL returned {status}: {body}"),
            ));
        }
        if body.get("errors").is_some() {
            return Err(SymphonyError::tracker(
                "github_graphql_errors",
                body["errors"].to_string(),
            ));
        }
        Ok(body["data"].clone())
    }

    async fn fetch_all_project_issues(&self) -> Result<Vec<Issue>> {
        let query = r#"
query SymphonyProjectIssues($org: String!, $number: Int!) {
  organization(login: $org) {
    projectV2(number: $number) {
      items(first: 100) {
        nodes {
          id
          content {
            ... on Issue {
              id
              number
              title
              body
              url
              createdAt
              updatedAt
              repository { name owner { login } }
              labels(first: 50) { nodes { name } }
            }
          }
          fieldValues(first: 20) {
            nodes {
              ... on ProjectV2ItemFieldSingleSelectValue {
                name
                field { ... on ProjectV2SingleSelectField { name } }
              }
            }
          }
        }
      }
    }
  }
}"#;
        let data = self
            .graphql(
                query,
                json!({ "org": self.org, "number": self.project_number }),
            )
            .await?;
        let nodes = data
            .pointer("/organization/projectV2/items/nodes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        nodes
            .into_iter()
            .filter_map(|node| self.normalize_node(node))
            .collect::<Result<Vec<_>>>()
    }

    fn normalize_node(&self, node: Value) -> Option<Result<Issue>> {
        let content = node.get("content")?;
        let repo_owner = content.pointer("/repository/owner/login")?.as_str()?;
        let repo = content.pointer("/repository/name")?.as_str()?;
        let number = content.get("number")?.as_u64()?;
        let status = node
            .pointer("/fieldValues/nodes")
            .and_then(Value::as_array)
            .and_then(|values| {
                values.iter().find_map(|value| {
                    let field = value.pointer("/field/name")?.as_str()?;
                    if field == self.status_field {
                        value.get("name")?.as_str().map(ToString::to_string)
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| "Todo".to_string());
        Some(Ok(Issue {
            id: content.get("id")?.as_str()?.to_string(),
            identifier: format!("{repo_owner}/{repo}#{number}"),
            title: content.get("title")?.as_str()?.to_string(),
            state: status,
            description: content
                .get("body")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            priority: None,
            branch_name: None,
            url: content.get("url")?.as_str()?.to_string(),
            labels: content
                .pointer("/labels/nodes")
                .and_then(Value::as_array)
                .map(|labels| {
                    labels
                        .iter()
                        .filter_map(|label| {
                            label
                                .get("name")
                                .and_then(Value::as_str)
                                .map(str::to_lowercase)
                        })
                        .collect()
                })
                .unwrap_or_default(),
            blocked_by: Vec::new(),
            created_at: content.get("createdAt")?.as_str()?.parse().ok()?,
            updated_at: content.get("updatedAt")?.as_str()?.parse().ok()?,
        }))
    }
}

#[async_trait]
impl Tracker for GithubProjectsV2Tracker {
    async fn fetch_candidate_issues(&self) -> Result<Vec<Issue>> {
        self.fetch_issues_by_states(&self.active_states).await
    }

    async fn fetch_issues_by_states(&self, states: &[String]) -> Result<Vec<Issue>> {
        let normalized: Vec<_> = states.iter().map(|state| state.to_lowercase()).collect();
        Ok(self
            .fetch_all_project_issues()
            .await?
            .into_iter()
            .filter(|issue| normalized.contains(&issue.state.to_lowercase()))
            .collect())
    }

    async fn fetch_issue_states_by_ids(&self, ids: &[String]) -> Result<HashMap<String, String>> {
        Ok(self
            .fetch_all_project_issues()
            .await?
            .into_iter()
            .filter(|issue| ids.contains(&issue.id))
            .map(|issue| (issue.id, issue.state))
            .collect())
    }

    async fn comment(&self, _issue_id: &str, _body: &str) -> Result<()> {
        Err(SymphonyError::tracker(
            "unsupported_tracker_write",
            "projects_v2 comment writes require repository REST lookup and are not enabled in v0.1",
        ))
    }

    async fn set_state(&self, _issue_id: &str, _state: &str) -> Result<()> {
        Err(SymphonyError::tracker(
            "unsupported_tracker_write",
            "projects_v2 status writes require field option lookup and are not enabled in v0.1",
        ))
    }

    async fn close(&self, _issue_id: &str) -> Result<()> {
        Err(SymphonyError::tracker(
            "unsupported_tracker_write",
            "projects_v2 close writes require repository REST lookup and are not enabled in v0.1",
        ))
    }

    async fn link_pr(&self, _issue_id: &str, _pr_number: u64) -> Result<()> {
        Err(SymphonyError::tracker(
            "unsupported_tracker_write",
            "projects_v2 PR linking requires repository REST lookup and is not enabled in v0.1",
        ))
    }
}

fn graphql_endpoint(endpoint: &str) -> String {
    if endpoint.contains("api.github.com/graphql") {
        endpoint.to_string()
    } else {
        format!("{}/graphql", endpoint.trim_end_matches('/'))
    }
}
