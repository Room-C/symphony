use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use symphony::agent::codex::CodexRunner;
use symphony::observability;
use symphony::orchestrator::Orchestrator;
use symphony::tracker::build_tracker;
use symphony::workflow_store::WorkflowStore;

#[derive(Debug, Parser)]
#[command(
    name = "symphony",
    version,
    about = "Run coding agents from GitHub Issues"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Run {
        #[arg(long, default_value = "WORKFLOW.md")]
        workflow: PathBuf,
        #[arg(long)]
        http_bind: Option<String>,
    },
    Check {
        #[arg(long, default_value = "WORKFLOW.md")]
        workflow: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Check { workflow } => {
            let workflow = symphony::Workflow::load(workflow)?;
            println!(
                "workflow ok: tracker={:?} mode={:?} workspace={}",
                workflow.config.tracker.kind,
                workflow.config.tracker.mode,
                workflow.config.workspace.root.display()
            );
            Ok(())
        }
        Commands::Run {
            workflow,
            http_bind,
        } => {
            let store = WorkflowStore::load(&workflow)?;
            let snapshot = store.snapshot().await;
            observability::init(
                snapshot.config.observability.json_logs,
                snapshot.config.observability.log_dir.as_deref(),
            )?;
            let tracker = build_tracker(&snapshot.config.tracker)?;
            let orchestrator = Orchestrator::new(store, tracker, Arc::new(CodexRunner));
            let bind = http_bind.unwrap_or(snapshot.config.observability.http_bind);
            let http_state = orchestrator.state();
            let orchestrator = orchestrator.with_events(
                symphony::orchestrator::state::live_event_sink(http_state.clone()),
            );
            tokio::spawn(async move {
                if let Err(error) = observability::http::serve(&bind, http_state).await {
                    tracing::error!(%error, "http status server failed");
                }
            });
            orchestrator.run_until_shutdown().await?;
            Ok(())
        }
    }
}
