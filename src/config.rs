use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Result, SymphonyError};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Config {
    pub tracker: TrackerConfig,
    pub polling: PollingConfig,
    pub workspace: WorkspaceConfig,
    pub hooks: HooksConfig,
    pub agent: AgentConfig,
    pub codex: CodexConfig,
    pub observability: ObservabilityConfig,
}

impl Config {
    pub fn resolve(mut self, workflow_dir: &Path) -> Result<Self> {
        self.tracker.api_key = resolve_env_ref(self.tracker.api_key.as_deref())?;
        self.workspace.root = normalize_workspace_root(&self.workspace.root, workflow_dir)?;
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<()> {
        if self.polling.interval_ms == 0 {
            return Err(SymphonyError::config(
                "invalid_poll_interval",
                "polling.interval_ms must be greater than zero",
            ));
        }
        if self.agent.max_concurrent_agents == 0 {
            return Err(SymphonyError::config(
                "invalid_max_concurrent_agents",
                "agent.max_concurrent_agents must be greater than zero",
            ));
        }
        if self.agent.max_turns == 0 {
            return Err(SymphonyError::config(
                "invalid_max_turns",
                "agent.max_turns must be greater than zero",
            ));
        }
        if self.tracker.kind != TrackerKind::Github {
            return Err(SymphonyError::config(
                "unsupported_tracker_kind",
                format!("unsupported tracker kind {:?}", self.tracker.kind),
            ));
        }
        if self
            .tracker
            .api_key
            .as_deref()
            .unwrap_or_default()
            .is_empty()
        {
            return Err(SymphonyError::config(
                "missing_tracker_api_key",
                "tracker.api_key is required and must resolve to a non-empty value",
            ));
        }
        match self.tracker.mode {
            TrackerMode::Labels => {
                require_field("tracker.owner", self.tracker.owner.as_deref())?;
                require_field("tracker.repo", self.tracker.repo.as_deref())?;
            }
            TrackerMode::ProjectsV2 => {
                require_field("tracker.org", self.tracker.org.as_deref())?;
                if self.tracker.project_number.is_none() {
                    return Err(SymphonyError::config(
                        "missing_tracker_project_number",
                        "tracker.project_number is required for projects_v2 mode",
                    ));
                }
            }
        }
        if self.tracker.active_states.is_empty() {
            return Err(SymphonyError::config(
                "missing_active_states",
                "tracker.active_states must not be empty",
            ));
        }
        Ok(())
    }

    pub fn active_state_set(&self) -> Vec<String> {
        self.tracker
            .active_states
            .iter()
            .map(|state| state.to_lowercase())
            .collect()
    }

    pub fn terminal_state_set(&self) -> Vec<String> {
        self.tracker
            .terminal_states
            .iter()
            .map(|state| state.to_lowercase())
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrackerKind {
    Github,
    Linear,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrackerMode {
    Labels,
    ProjectsV2,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct TrackerConfig {
    pub kind: TrackerKind,
    pub mode: TrackerMode,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub org: Option<String>,
    pub project_number: Option<u64>,
    pub status_field: String,
    pub api_key: Option<String>,
    pub endpoint: String,
    pub active_states: Vec<String>,
    pub terminal_states: Vec<String>,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            kind: TrackerKind::Github,
            mode: TrackerMode::Labels,
            owner: None,
            repo: None,
            org: None,
            project_number: None,
            status_field: "Status".to_string(),
            api_key: None,
            endpoint: "https://api.github.com".to_string(),
            active_states: vec![
                "Todo".to_string(),
                "In Progress".to_string(),
                "Rework".to_string(),
            ],
            terminal_states: vec![
                "Done".to_string(),
                "Closed".to_string(),
                "Cancelled".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PollingConfig {
    pub interval_ms: u64,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            interval_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct WorkspaceConfig {
    pub root: PathBuf,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from(".symphony/workspaces"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct HooksConfig {
    pub after_create: Option<String>,
    pub before_run: Option<String>,
    pub after_run: Option<String>,
    pub before_remove: Option<String>,
    pub timeout_ms: Option<u64>,
}

impl HooksConfig {
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms.unwrap_or(60_000)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AgentConfig {
    pub max_concurrent_agents: usize,
    pub max_turns: u32,
    pub max_retry_backoff_ms: u64,
    pub max_concurrent_agents_by_state: HashMap<String, usize>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_concurrent_agents: 1,
            max_turns: 1,
            max_retry_backoff_ms: 300_000,
            max_concurrent_agents_by_state: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct CodexConfig {
    pub command: String,
    pub approval_policy: String,
    pub thread_sandbox: String,
    pub turn_sandbox_policy: String,
    pub read_timeout_ms: u64,
    pub turn_timeout_ms: u64,
    pub stall_timeout_ms: i64,
}

impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            command: "codex app-server".to_string(),
            approval_policy: "never".to_string(),
            thread_sandbox: "workspace-write".to_string(),
            turn_sandbox_policy: "workspace-write".to_string(),
            read_timeout_ms: 5_000,
            turn_timeout_ms: 3_600_000,
            stall_timeout_ms: 300_000,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ObservabilityConfig {
    pub http_bind: String,
    pub log_dir: Option<PathBuf>,
    pub json_logs: bool,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            http_bind: "127.0.0.1:8723".to_string(),
            log_dir: None,
            json_logs: true,
        }
    }
}

fn require_field(name: &'static str, value: Option<&str>) -> Result<()> {
    if value.unwrap_or_default().trim().is_empty() {
        Err(SymphonyError::config(
            "missing_required_field",
            format!("{name} is required"),
        ))
    } else {
        Ok(())
    }
}

fn resolve_env_ref(value: Option<&str>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if let Some(name) = value.strip_prefix('$') {
        if name.is_empty()
            || !name
                .chars()
                .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        {
            return Err(SymphonyError::config(
                "invalid_env_reference",
                format!("invalid env reference {value:?}"),
            ));
        }
        let resolved = env::var(name).map_err(|_| SymphonyError::MissingEnv {
            name: name.to_string(),
        })?;
        if resolved.is_empty() {
            return Err(SymphonyError::MissingEnv {
                name: name.to_string(),
            });
        }
        Ok(Some(resolved))
    } else {
        Ok(Some(value.to_string()))
    }
}

fn normalize_workspace_root(root: &Path, workflow_dir: &Path) -> Result<PathBuf> {
    let expanded = if let Some(rest) = root.to_string_lossy().strip_prefix("~/") {
        dirs::home_dir()
            .ok_or_else(|| SymphonyError::config("home_dir_unavailable", "could not resolve ~"))?
            .join(rest)
    } else if root == Path::new("~") {
        dirs::home_dir()
            .ok_or_else(|| SymphonyError::config("home_dir_unavailable", "could not resolve ~"))?
    } else if root.is_absolute() {
        root.to_path_buf()
    } else {
        workflow_dir.join(root)
    };
    Ok(expanded)
}
