use std::path::PathBuf;

use thiserror::Error;

pub type Result<T, E = SymphonyError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum SymphonyError {
    #[error("workflow file not found: {path}")]
    WorkflowNotFound { path: PathBuf },

    #[error("workflow front matter is missing")]
    MissingFrontMatter,

    #[error("workflow front matter must decode to a map/object")]
    FrontMatterNotMap,

    #[error("workflow prompt body is empty")]
    EmptyPrompt,

    #[error("workflow yaml parse failed: {0}")]
    WorkflowYaml(#[from] serde_yaml::Error),

    #[error("config validation failed: {code}: {message}")]
    ConfigValidation { code: &'static str, message: String },

    #[error("environment variable {name} is missing or empty")]
    MissingEnv { name: String },

    #[error("prompt render failed: {code}: {message}")]
    PromptRender { code: &'static str, message: String },

    #[error("workspace safety violation: {message}")]
    WorkspaceSafety { message: String },

    #[error("workspace hook {hook} failed: {message}")]
    WorkspaceHook { hook: String, message: String },

    #[error("tracker error: {code}: {message}")]
    Tracker { code: &'static str, message: String },

    #[error("agent error: {code}: {message}")]
    Agent { code: &'static str, message: String },

    #[error("http server error: {0}")]
    Http(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl SymphonyError {
    pub fn config(code: &'static str, message: impl Into<String>) -> Self {
        Self::ConfigValidation {
            code,
            message: message.into(),
        }
    }

    pub fn prompt(code: &'static str, message: impl Into<String>) -> Self {
        Self::PromptRender {
            code,
            message: message.into(),
        }
    }

    pub fn tracker(code: &'static str, message: impl Into<String>) -> Self {
        Self::Tracker {
            code,
            message: message.into(),
        }
    }

    pub fn agent(code: &'static str, message: impl Into<String>) -> Self {
        Self::Agent {
            code,
            message: message.into(),
        }
    }
}
