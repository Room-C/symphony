pub mod labels;
pub mod projects_v2;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct GithubLabel {
    pub name: String,
}

pub fn parse_priority(labels: &[GithubLabel]) -> Option<u8> {
    labels
        .iter()
        .find_map(|label| label.name.strip_prefix("priority:")?.parse().ok())
}

pub fn visible_labels(labels: &[GithubLabel]) -> Vec<String> {
    labels
        .iter()
        .filter_map(|label| {
            let lowered = label.name.to_lowercase();
            (!lowered.starts_with("symphony:") && !lowered.starts_with("priority:"))
                .then_some(lowered)
        })
        .collect()
}

pub fn state_from_labels(labels: &[GithubLabel]) -> String {
    labels
        .iter()
        .find_map(|label| {
            label
                .name
                .strip_prefix("symphony:")
                .map(|state| state.replace('-', " "))
        })
        .unwrap_or_else(|| "Todo".to_string())
}

pub fn state_to_label(state: &str) -> String {
    format!("symphony:{}", state.to_lowercase().replace(' ', "-"))
}

pub fn parse_blocked_by(body: Option<&str>, owner: &str, repo: &str) -> Vec<String> {
    let Some(body) = body else {
        return Vec::new();
    };
    let re = regex::Regex::new(
        r"- \[ \] (?:(?P<owner>[A-Za-z0-9_.-]+)/(?P<repo>[A-Za-z0-9_.-]+))?#(?P<number>\d+)",
    )
    .unwrap();
    re.captures_iter(body)
        .filter_map(|caps| {
            let number = caps.name("number")?.as_str();
            let cap_owner = caps.name("owner").map(|m| m.as_str()).unwrap_or(owner);
            let cap_repo = caps.name("repo").map(|m| m.as_str()).unwrap_or(repo);
            Some(format!("{cap_owner}/{cap_repo}#{number}"))
        })
        .collect()
}
