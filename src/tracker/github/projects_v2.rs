use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::StatusCode;
use serde_json::{Value, json};

use crate::config::TrackerConfig;
use crate::error::{Result, SymphonyError};
use crate::events::Issue;
use crate::tracker::Tracker;

use super::format_request_error;

#[derive(Debug, Clone)]
pub struct GithubProjectsV2Tracker {
    client: reqwest::Client,
    endpoint: String,
    rest_endpoint: String,
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
            rest_endpoint: rest_endpoint(&config.endpoint),
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
            .map_err(|error| {
                SymphonyError::tracker("github_api_request", format_request_error(&error))
            })?;
        let status = response.status();
        let body: Value = response.json().await.map_err(|error| {
            SymphonyError::tracker("github_api_request", format_request_error(&error))
        })?;
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

    async fn rest_empty(&self, request: reqwest::RequestBuilder, ok: &[StatusCode]) -> Result<()> {
        let response = request
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "Room-C-Symphony")
            .send()
            .await
            .map_err(|error| {
                SymphonyError::tracker("github_api_request", format_request_error(&error))
            })?;
        let status = response.status();
        if !ok.contains(&status) {
            let body = response.text().await.unwrap_or_default();
            return Err(SymphonyError::tracker(
                "github_api_status",
                format!("GitHub returned {status}: {body}"),
            ));
        }
        Ok(())
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

    async fn find_issue_ref(&self, issue_id: &str) -> Result<IssueRef> {
        self.fetch_all_project_issues()
            .await?
            .into_iter()
            .find(|issue| issue.id == issue_id)
            .and_then(|issue| IssueRef::from_identifier(&issue.identifier))
            .ok_or_else(|| {
                SymphonyError::tracker(
                    "github_issue_not_found",
                    format!("issue id {issue_id} not found in project"),
                )
            })
    }

    async fn status_update_context(
        &self,
        issue_id: &str,
        state: &str,
    ) -> Result<StatusUpdateContext> {
        let query = r#"
query SymphonyProjectWriteContext($org: String!, $number: Int!) {
  organization(login: $org) {
    projectV2(number: $number) {
      id
      fields(first: 100) {
        nodes {
          ... on ProjectV2SingleSelectField {
            id
            name
            options { id name }
          }
        }
      }
      items(first: 100) {
        nodes {
          id
          content {
            ... on Issue { id }
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
        let project = data
            .pointer("/organization/projectV2")
            .ok_or_else(|| SymphonyError::tracker("github_graphql_errors", "project not found"))?;
        let project_id = project
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| SymphonyError::tracker("github_graphql_errors", "project id missing"))?
            .to_string();
        let field = project
            .pointer("/fields/nodes")
            .and_then(Value::as_array)
            .and_then(|fields| {
                fields.iter().find(|field| {
                    field.get("name").and_then(Value::as_str) == Some(self.status_field.as_str())
                })
            })
            .ok_or_else(|| {
                SymphonyError::tracker(
                    "github_graphql_errors",
                    format!("status field {:?} not found", self.status_field),
                )
            })?;
        let field_id = field
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                SymphonyError::tracker("github_graphql_errors", "status field id missing")
            })?
            .to_string();
        let option_id = field
            .get("options")
            .and_then(Value::as_array)
            .and_then(|options| {
                options.iter().find_map(|option| {
                    let name = option.get("name")?.as_str()?;
                    name.eq_ignore_ascii_case(state)
                        .then(|| option.get("id")?.as_str().map(ToString::to_string))
                        .flatten()
                })
            })
            .ok_or_else(|| {
                SymphonyError::tracker(
                    "github_graphql_errors",
                    format!("status option {state:?} not found"),
                )
            })?;
        let item_id = project
            .pointer("/items/nodes")
            .and_then(Value::as_array)
            .and_then(|items| {
                items.iter().find_map(|item| {
                    (item.pointer("/content/id").and_then(Value::as_str) == Some(issue_id))
                        .then(|| item.get("id")?.as_str().map(ToString::to_string))
                        .flatten()
                })
            })
            .ok_or_else(|| {
                SymphonyError::tracker(
                    "github_issue_not_found",
                    format!("issue id {issue_id} not found in project items"),
                )
            })?;
        Ok(StatusUpdateContext {
            project_id,
            item_id,
            field_id,
            option_id,
        })
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

    async fn comment(&self, issue_id: &str, body: &str) -> Result<()> {
        let issue = self.find_issue_ref(issue_id).await?;
        let path = format!(
            "{}/repos/{}/{}/issues/{}/comments",
            self.rest_endpoint, issue.owner, issue.repo, issue.number
        );
        self.rest_empty(
            self.client.post(path).json(&json!({ "body": body })),
            &[StatusCode::CREATED],
        )
        .await
    }

    async fn set_state(&self, issue_id: &str, state: &str) -> Result<()> {
        let context = self.status_update_context(issue_id, state).await?;
        let mutation = r#"
mutation SymphonyUpdateProjectStatus($projectId: ID!, $itemId: ID!, $fieldId: ID!, $optionId: String!) {
  updateProjectV2ItemFieldValue(input: {
    projectId: $projectId,
    itemId: $itemId,
    fieldId: $fieldId,
    value: { singleSelectOptionId: $optionId }
  }) {
    projectV2Item { id }
  }
}"#;
        self.graphql(
            mutation,
            json!({
                "projectId": context.project_id,
                "itemId": context.item_id,
                "fieldId": context.field_id,
                "optionId": context.option_id,
            }),
        )
        .await?;
        Ok(())
    }

    async fn close(&self, issue_id: &str) -> Result<()> {
        let issue = self.find_issue_ref(issue_id).await?;
        let path = format!(
            "{}/repos/{}/{}/issues/{}",
            self.rest_endpoint, issue.owner, issue.repo, issue.number
        );
        self.rest_empty(
            self.client.patch(path).json(&json!({ "state": "closed" })),
            &[StatusCode::OK],
        )
        .await
    }

    async fn link_pr(&self, issue_id: &str, pr_number: u64) -> Result<()> {
        self.comment(issue_id, &format!("Linked pull request: #{pr_number}"))
            .await
    }
}

fn graphql_endpoint(endpoint: &str) -> String {
    if endpoint.contains("api.github.com/graphql") {
        endpoint.to_string()
    } else {
        format!("{}/graphql", endpoint.trim_end_matches('/'))
    }
}

fn rest_endpoint(endpoint: &str) -> String {
    endpoint
        .trim_end_matches('/')
        .strip_suffix("/graphql")
        .unwrap_or_else(|| endpoint.trim_end_matches('/'))
        .to_string()
}

#[derive(Debug, Clone)]
struct IssueRef {
    owner: String,
    repo: String,
    number: u64,
}

impl IssueRef {
    fn from_identifier(identifier: &str) -> Option<Self> {
        let (owner_repo, number) = identifier.rsplit_once('#')?;
        let (owner, repo) = owner_repo.split_once('/')?;
        Some(Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
            number: number.parse().ok()?,
        })
    }
}

#[derive(Debug, Clone)]
struct StatusUpdateContext {
    project_id: String,
    item_id: String,
    field_id: String,
    option_id: String,
}
