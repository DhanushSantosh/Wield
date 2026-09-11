//! Process logging to the XDG state directory.

use std::path::PathBuf;
use tracing_subscriber::prelude::*;

/// Initialize tracing and return the guard that keeps file logging alive.
pub fn init() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let Some(log_dir) = log_dir() else {
        init_stderr();
        return None;
    };
    if std::fs::create_dir_all(&log_dir).is_err() {
        init_stderr();
        return None;
    }

    let file = tracing_appender::rolling::daily(log_dir, "wield.log");
    let (writer, guard) = tracing_appender::non_blocking(file);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(writer);
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    #[cfg(debug_assertions)]
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .try_init();
    #[cfg(not(debug_assertions))]
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .try_init();
    Some(guard)
}

fn init_stderr() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn log_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .map(|state| state.join("wield/logs"))
}
