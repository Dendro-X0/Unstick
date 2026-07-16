//! Unstick background service.

mod ipc_server;
mod runtime;

use std::fs;

use anyhow::Result;
use guardian_core::{config_dir, VERSION};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn init_logging() -> Result<WorkerGuard> {
    fs::create_dir_all(config_dir())?;
    let file_appender =
        tracing_appender::rolling::daily(config_dir(), "guardian.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout_layer = fmt::layer().with_target(false).with_ansi(true);
    let file_layer = fmt::layer()
        .with_target(false)
        .with_ansi(false)
        .with_writer(non_blocking);

    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    Ok(guard)
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = init_logging()?;

    tracing::info!(version = VERSION, "guardian-service starting");
    runtime::run().await
}
