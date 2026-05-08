use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;

use crate::config::TrackerConfig;
use crate::error::{Result, SymphonyError};
use crate::events::Issue;
use crate::tracker::Tracker;

use super::{
    GithubLabel, format_request_error, parse_blocked_by, parse_priority, state_from_labels,
    state_to_label, visible_labels,
};

#[derive(Debug, Clone)]
pub struct GithubLabelsTracker {
    client: reqwest::Client,
    endpoint: String,
    token: String,
    owner: String,
    repo: String,
    active_states: Vec<String>,
}

impl GithubLabelsTracker {
    pub fn new(config: &TrackerConfig) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            endpoint: config.endpoint.trim_end_matches('/').to_string(),
            token: config.api_key.clone().ok_or_else(|| {
                SymphonyError::tracker("missing_tracker_api_key", "tracker.api_key is required")
            })?,
            owner: config.owner.clone().ok_or_else(|| {
                SymphonyError::tracker("missing_tracker_owner", "tracker.owner is required")
            })?,
            repo: config.repo.clone().ok_or_else(|| {
                SymphonyError::tracker("missing_tracker_repo", "tracker.repo is required")
            })?,
            active_states: config.active_states.clone(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.endpoint, path)
    }

    async fn request_json<T: for<'de> Deserialize<'de>>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T> {
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
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(SymphonyError::tracker(
                "github_api_status",
                format!("GitHub returned {status}: {body}"),
            ));
        }
        response.json::<T>().await.map_err(|error| {
            SymphonyError::tracker("github_api_request", format_request_error(&error))
        })
    }

    async fn request_empty(
        &self,
        request: reqwest::RequestBuilder,
        ok: &[StatusCode],
    ) -> Result<()> {
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

    async fn fetch_by_label_state(&self, state: &str) -> Result<Vec<Issue>> {
        let label = state_to_label(state);
        let encoded = urlencoding::encode(&label);
        let path = format!(
            "/repos/{}/{}/issues?state=open&labels={encoded}&per_page=50",
            self.owner, self.repo
        );
        let mut raw: Vec<GithubIssue> = self.request_json(self.client.get(self.url(&path))).await?;
        raw.retain(|issue| issue.pull_request.is_none());
        Ok(raw
            .into_iter()
            .map(|issue| self.normalize_issue(issue))
            .collect())
    }

    fn normalize_issue(&self, issue: GithubIssue) -> Issue {
        let state = state_from_labels(&issue.labels);
        Issue {
            id: issue.node_id,
            identifier: format!("{}/{}#{}", self.owner, self.repo, issue.number),
            title: issue.title,
            state,
            description: issue.body.clone(),
            priority: parse_priority(&issue.labels),
            branch_name: None,
            url: issue.html_url,
            labels: visible_labels(&issue.labels),
            blocked_by: parse_blocked_by(issue.body.as_deref(), &self.owner, &self.repo),
            created_at: issue.created_at,
            updated_at: issue.updated_at,
        }
    }

    async fn find_issue_by_id(&self, issue_id: &str) -> Result<GithubIssue> {
        for issue in self.fetch_candidate_issues().await? {
            if issue.id == issue_id {
                let number = issue
                    .identifier
                    .rsplit('#')
                    .next()
                    .and_then(|raw| raw.parse::<u64>().ok())
                    .ok_or_else(|| {
                        SymphonyError::tracker(
                            "github_issue_identifier",
                            "invalid issue identifier",
                        )
                    })?;
                let path = format!("/repos/{}/{}/issues/{number}", self.owner, self.repo);
                return self.request_json(self.client.get(self.url(&path))).await;
            }
        }
        Err(SymphonyError::tracker(
            "github_issue_not_found",
            format!("issue id {issue_id} not found in active candidates"),
        ))
    }
}

#[async_trait]
impl Tracker for GithubLabelsTracker {
    async fn fetch_candidate_issues(&self) -> Result<Vec<Issue>> {
        self.fetch_issues_by_states(&self.active_states).await
    }

    async fn fetch_issues_by_states(&self, states: &[String]) -> Result<Vec<Issue>> {
        let mut all = Vec::new();
        for state in states {
            all.extend(self.fetch_by_label_state(state).await?);
        }
        all.sort_by(|left, right| left.id.cmp(&right.id));
        all.dedup_by(|left, right| left.id == right.id);
        Ok(all)
    }

    async fn fetch_issue_states_by_ids(&self, ids: &[String]) -> Result<HashMap<String, String>> {
        let issues = self.fetch_candidate_issues().await?;
        let mut states = HashMap::new();
        for issue in issues {
            if ids.contains(&issue.id) {
                states.insert(issue.id, issue.state);
            }
        }
        Ok(states)
    }

    async fn comment(&self, issue_id: &str, body: &str) -> Result<()> {
        let issue = self.find_issue_by_id(issue_id).await?;
        let path = format!(
            "/repos/{}/{}/issues/{}/comments",
            self.owner, self.repo, issue.number
        );
        self.request_empty(
            self.client
                .post(self.url(&path))
                .json(&json!({ "body": body })),
            &[StatusCode::CREATED],
        )
        .await
    }

    async fn set_state(&self, issue_id: &str, state: &str) -> Result<()> {
        let issue = self.find_issue_by_id(issue_id).await?;
        let old_state_labels: Vec<_> = issue
            .labels
            .iter()
            .filter(|label| label.name.to_lowercase().starts_with("symphony:"))
            .map(|label| label.name.clone())
            .collect();
        for label in old_state_labels {
            let path = format!(
                "/repos/{}/{}/issues/{}/labels/{}",
                self.owner,
                self.repo,
                issue.number,
                urlencoding::encode(&label)
            );
            self.request_empty(
                self.client.delete(self.url(&path)),
                &[StatusCode::OK, StatusCode::NO_CONTENT],
            )
            .await?;
        }
        let label = state_to_label(state);
        let path = format!(
            "/repos/{}/{}/issues/{}/labels",
            self.owner, self.repo, issue.number
        );
        self.request_empty(
            self.client
                .post(self.url(&path))
                .json(&json!({ "labels": [label] })),
            &[StatusCode::OK],
        )
        .await
    }

    async fn close(&self, issue_id: &str) -> Result<()> {
        let issue = self.find_issue_by_id(issue_id).await?;
        let path = format!(
            "/repos/{}/{}/issues/{}",
            self.owner, self.repo, issue.number
        );
        self.request_empty(
            self.client
                .patch(self.url(&path))
                .json(&json!({ "state": "closed" })),
            &[StatusCode::OK],
        )
        .await
    }

    async fn link_pr(&self, issue_id: &str, pr_number: u64) -> Result<()> {
        let body = format!("Linked pull request: #{}", pr_number);
        self.comment(issue_id, &body).await
    }
}

#[derive(Debug, Clone, Deserialize)]
struct GithubIssue {
    node_id: String,
    number: u64,
    title: String,
    body: Option<String>,
    html_url: String,
    labels: Vec<GithubLabel>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    #[serde(default)]
    pull_request: Option<serde_json::Value>,
}
