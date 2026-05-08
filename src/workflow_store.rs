use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{error, info};

use crate::error::Result;
use crate::workflow::Workflow;

#[derive(Clone)]
pub struct WorkflowStore {
    path: PathBuf,
    current: Arc<RwLock<Workflow>>,
}

impl WorkflowStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let workflow = Workflow::load(&path)?;
        Ok(Self {
            path,
            current: Arc::new(RwLock::new(workflow)),
        })
    }

    pub async fn snapshot(&self) -> Workflow {
        self.current.read().await.clone()
    }

    pub async fn reload(&self) -> Result<()> {
        match Workflow::load(&self.path) {
            Ok(workflow) => {
                *self.current.write().await = workflow;
                info!(path = %self.path.display(), "workflow reloaded");
                Ok(())
            }
            Err(error) => {
                error!(path = %self.path.display(), %error, "workflow reload failed; keeping last known good config");
                Err(error)
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
