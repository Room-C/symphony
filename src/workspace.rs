use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::fs;
use tokio::process::Command;
use tokio::time;
use tracing::{error, info};

use crate::config::HooksConfig;
use crate::error::{Result, SymphonyError};
use crate::path_safety::{ensure_cwd, ensure_workspace_child, sanitize_workspace_key};

#[derive(Debug, Clone)]
pub struct Workspace {
    pub key: String,
    pub path: PathBuf,
    pub created_now: bool,
}

#[derive(Debug, Clone)]
pub struct WorkspaceManager {
    root: PathBuf,
    hooks: HooksConfig,
}

#[derive(Debug, Clone, Copy)]
pub enum HookName {
    AfterCreate,
    BeforeRun,
    AfterRun,
    BeforeRemove,
}

impl HookName {
    fn as_str(self) -> &'static str {
        match self {
            Self::AfterCreate => "after_create",
            Self::BeforeRun => "before_run",
            Self::AfterRun => "after_run",
            Self::BeforeRemove => "before_remove",
        }
    }
}

impl WorkspaceManager {
    pub fn new(root: PathBuf, hooks: HooksConfig) -> Self {
        Self { root, hooks }
    }

    pub async fn prepare(&self, issue_identifier: &str) -> Result<Workspace> {
        fs::create_dir_all(&self.root).await?;
        let key = sanitize_workspace_key(issue_identifier);
        let path = ensure_workspace_child(&self.root, &key)?;
        let created_now = fs::metadata(&path).await.is_err();
        fs::create_dir_all(&path).await?;
        if created_now {
            self.run_hook(HookName::AfterCreate, &path).await?;
        }
        Ok(Workspace {
            key,
            path,
            created_now,
        })
    }

    pub async fn before_run(&self, workspace: &Workspace) -> Result<()> {
        self.run_hook(HookName::BeforeRun, &workspace.path).await
    }

    pub async fn after_run_best_effort(&self, workspace: &Workspace) {
        if let Err(error) = self.run_hook(HookName::AfterRun, &workspace.path).await {
            error!(%error, path = %workspace.path.display(), "after_run hook failed; ignoring");
        }
    }

    pub async fn before_remove_best_effort(&self, workspace_path: &Path) {
        if let Err(error) = self.run_hook(HookName::BeforeRemove, workspace_path).await {
            error!(%error, path = %workspace_path.display(), "before_remove hook failed; ignoring");
        }
    }

    pub async fn validate_agent_cwd(&self, workspace: &Workspace, cwd: &Path) -> Result<()> {
        ensure_cwd(&workspace.path, cwd)
    }

    async fn run_hook(&self, hook: HookName, cwd: &Path) -> Result<()> {
        let script = match hook {
            HookName::AfterCreate => self.hooks.after_create.as_deref(),
            HookName::BeforeRun => self.hooks.before_run.as_deref(),
            HookName::AfterRun => self.hooks.after_run.as_deref(),
            HookName::BeforeRemove => self.hooks.before_remove.as_deref(),
        };
        let Some(script) = script else {
            return Ok(());
        };
        info!(hook = hook.as_str(), cwd = %cwd.display(), "running workspace hook");
        let mut child = Command::new("bash")
            .arg("-lc")
            .arg(script)
            .current_dir(cwd)
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| SymphonyError::WorkspaceHook {
                hook: hook.as_str().to_string(),
                message: error.to_string(),
            })?;
        let timeout = Duration::from_millis(self.hooks.timeout_ms());
        let status = time::timeout(timeout, child.wait()).await.map_err(|_| {
            SymphonyError::WorkspaceHook {
                hook: hook.as_str().to_string(),
                message: format!("timed out after {}ms", self.hooks.timeout_ms()),
            }
        })??;
        if !status.success() {
            return Err(SymphonyError::WorkspaceHook {
                hook: hook.as_str().to_string(),
                message: format!("exited with status {status}"),
            });
        }
        Ok(())
    }
}
