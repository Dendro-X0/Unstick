//! Unstick background service.
//!
//! Windows GUI subsystem — no console window at runtime. Logs go to AppData.
//! Set `UNSTICK_CONSOLE=1` to attach a console for interactive debugging.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod ipc_server;
mod runtime;

use std::fs;

use anyhow::Result;
use guardian_core::{config_dir, VERSION};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn want_console() -> bool {
    std::env::var_os("UNSTICK_CONSOLE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(windows)]
fn attach_debug_console() {
    use windows::Win32::System::Console::{AllocConsole, AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            let _ = AllocConsole();
        }
    }
}

fn init_logging() -> Result<WorkerGuard> {
    fs::create_dir_all(config_dir())?;
    let file_appender = tracing_appender::rolling::daily(config_dir(), "guardian.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    if want_console() {
        #[cfg(windows)]
        attach_debug_console();
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(false).with_ansi(true))
            .with(
                fmt::layer()
                    .with_target(false)
                    .with_ansi(false)
                    .with_writer(non_blocking),
            )
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .with_target(false)
                    .with_ansi(false)
                    .with_writer(non_blocking),
            )
            .init();
    }

    Ok(guard)
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = init_logging()?;

    tracing::info!(version = VERSION, "guardian-service starting");
    runtime::run().await
}
