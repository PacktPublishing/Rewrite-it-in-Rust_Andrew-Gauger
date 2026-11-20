//! One log for the whole app: a daily rolling file, written off the
//! calling thread, so a slow disk never stalls a frame.

use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// Installs the global subscriber. Hold the returned guard until
/// `main` ends: dropped, it stops the writer thread and the last
/// lines are lost. `RUST_LOG` overrides the default level, `info`.
pub fn init(dir: &Path) -> WorkerGuard {
    let file = tracing_appender::rolling::daily(dir, "cairn.log");
    let (writer, guard) = tracing_appender::non_blocking(file);
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(filter)
        .init();
    guard
}
