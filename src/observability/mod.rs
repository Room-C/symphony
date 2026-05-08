pub mod http;

use std::path::Path;

use tracing_subscriber::EnvFilter;

use crate::Result;

pub fn init(json_logs: bool, log_dir: Option<&Path>) -> Result<()> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("symphony=info,info"));
    if let Some(log_dir) = log_dir {
        std::fs::create_dir_all(log_dir)?;
        let file_appender = tracing_appender::rolling::daily(log_dir, "symphony.jsonl");
        if json_logs {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .json()
                .with_writer(file_appender)
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(file_appender)
                .init();
        }
    } else if json_logs {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
    Ok(())
}
