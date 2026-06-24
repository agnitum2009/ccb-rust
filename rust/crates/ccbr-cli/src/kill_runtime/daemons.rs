//! Mirrors Python `lib/cli/kill_runtime/daemons.py`.

use std::path::{Path, PathBuf};

pub const CCBRD_RUNTIME_NAME: &str = "ccbrd";
pub const CCBRD_RPC_PREFIX: &str = "ask";
pub const CCBRD_STATE_FILE_NAME: &str = "ccbrd.json";

/// Terminate the ccbrd daemon for a provider, requesting graceful shutdown
/// first and force-killing the recorded PID as a last resort.
///
/// Mirrors Python `terminate_provider_daemon(provider, ...)`. The callable
/// dependencies are passed in to keep this module free of daemon runtime
/// coupling. Missing state (`None`) short-circuits cleanly, matching the
/// Python guard `state and state.get("pid")`.
pub fn terminate_provider_daemon<F, S, R, K>(
    _provider: &str,
    _specs_by_provider: &std::collections::HashMap<String, serde_json::Value>,
    state_file_path_fn: F,
    shutdown_daemon_fn: S,
    read_state_fn: R,
    kill_pid_fn: K,
) where
    F: Fn(&str) -> PathBuf,
    S: Fn(&str, f64, &Path) -> bool,
    R: Fn(&Path) -> Option<serde_json::Value>,
    K: Fn(i64, bool) -> bool,
{
    let state_file = state_file_path_fn(CCBRD_STATE_FILE_NAME);
    if shutdown_daemon_fn(CCBRD_RPC_PREFIX, 1.0, &state_file) {
        println!("✅ {} runtime shutdown requested", CCBRD_RUNTIME_NAME);
        return;
    }
    if let Some(state) = read_state_fn(&state_file) {
        if let Some(pid) = state.get("pid").and_then(|v| v.as_i64()) {
            if kill_pid_fn(pid, true) {
                println!(
                    "✅ {} runtime force killed (pid={})",
                    CCBRD_RUNTIME_NAME, pid
                );
            } else {
                println!(
                    "⚠️ {} runtime could not be killed (pid={})",
                    CCBRD_RUNTIME_NAME, pid
                );
            }
        }
    }
}
