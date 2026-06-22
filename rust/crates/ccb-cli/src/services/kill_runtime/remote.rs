//! Mirrors Python `lib/cli/services/kill_runtime/remote.py`.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::context::CliContext;
use crate::services::daemon::DaemonHandle;
use crate::services::daemon_runtime::models::KillSummary;
use crate::services::kill_runtime::reporting::summary_from_stop_all_payload;
use crate::services::DaemonClient;

const DEFAULT_SHUTDOWN_TIMEOUT_S: f64 = 1.0;
const POLL_INTERVAL_MS: u64 = 50;

/// Trait for a client that can call the daemon `stop_all` method.
pub trait StopAllClient {
    fn stop_all(&self, force: bool) -> anyhow::Result<serde_json::Value>;
}

impl StopAllClient for crate::services::UnixDaemonClient {
    fn stop_all(&self, force: bool) -> anyhow::Result<serde_json::Value> {
        self.call("stop_all", json!({"force": force}))
            .map_err(|e| anyhow::anyhow!("stop_all failed: {e}"))
    }
}

/// Request the daemon to stop all agents remotely.
///
/// Mirrors Python `request_remote_stop`.
#[allow(clippy::too_many_arguments)]
pub fn request_remote_stop<F, G, C>(
    context: &CliContext,
    force: bool,
    connect_mounted_daemon_fn: F,
    record_shutdown_intent_fn: G,
    ccbd_client_factory_fn: C,
    stop_all_timeout_s: f64,
) -> anyhow::Result<Option<KillSummary>>
where
    F: FnOnce(&CliContext, bool) -> Result<DaemonHandle, anyhow::Error>,
    G: FnOnce(&CliContext, &str),
    C: FnOnce(&Path, f64) -> Result<Box<dyn StopAllClient>, anyhow::Error>,
{
    let _handle = match connect_mounted_daemon_fn(context, false) {
        Ok(handle) => handle,
        Err(_) => return Ok(None),
    };
    record_shutdown_intent_fn(context, "kill");
    let socket_path = context.paths.ccbd_socket_path();
    let client = match ccbd_client_factory_fn(socket_path.as_std_path(), stop_all_timeout_s) {
        Ok(client) => client,
        Err(_) if force => return Ok(None),
        Err(e) => return Err(e),
    };
    let payload = match client.stop_all(force) {
        Ok(payload) => payload,
        Err(_) if force => return Ok(None),
        Err(e) => return Err(e),
    };
    Ok(Some(summary_from_stop_all_payload(&payload)))
}

/// Resolve the shutdown summary: prefer remote shutdown, fall back to local.
///
/// Mirrors Python `resolve_shutdown_summary`.
pub fn resolve_shutdown_summary<F, G>(
    context: &CliContext,
    remote_summary: Option<&KillSummary>,
    force: bool,
    shutdown_daemon_fn: F,
    await_remote_shutdown_fn: G,
) -> anyhow::Result<KillSummary>
where
    F: FnOnce(&CliContext, bool) -> anyhow::Result<KillSummary>,
    G: FnOnce(&CliContext, bool) -> anyhow::Result<KillSummary>,
{
    if remote_summary.is_some() {
        return await_remote_shutdown_fn(context, force);
    }
    match shutdown_daemon_fn(context, force) {
        Ok(summary) => Ok(summary),
        Err(_e) if force => Ok(KillSummary {
            project_id: context.project.project_id.clone(),
            state: "unmounted".into(),
            socket_path: context.paths.ccbd_socket_path().to_string(),
            forced: force,
            cleanup_summaries: Vec::new(),
            worktree_warnings: Vec::new(),
        }),
        Err(e) => Err(e),
    }
}

/// Wait for the remote daemon to shut down, terminating lingering PIDs.
///
/// Mirrors Python `await_remote_shutdown`. The Rust version uses a simplified
/// polling model over injected closures.
#[allow(clippy::too_many_arguments)]
pub fn await_remote_shutdown<P, A, T, W, K>(
    context: &CliContext,
    force: bool,
    timeout_s: f64,
    expected_pids: &[u32],
    inspect_daemon_fn: P,
    is_pid_alive_fn: A,
    terminate_pid_tree_fn: T,
    wait_for_pid_exit_fn: W,
    wait_for_keeper_exit_fn: K,
) -> anyhow::Result<KillSummary>
where
    P: Fn(&CliContext) -> String,
    A: Fn(u32) -> bool + Copy,
    T: Fn(u32, f64, A) -> bool,
    W: Fn(u32, f64) -> bool,
    K: Fn(f64) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs_f64(timeout_s.max(0.1));
    let mut last_phase = "unmounted".to_string();
    while Instant::now() < deadline {
        last_phase = inspect_daemon_fn(context);
        if last_phase == "unmounted" {
            break;
        }
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }

    for pid in expected_pids {
        if !is_pid_alive_fn(*pid) {
            continue;
        }
        if wait_for_pid_exit_fn(*pid, DEFAULT_SHUTDOWN_TIMEOUT_S) {
            continue;
        }
        terminate_pid_tree_fn(*pid, DEFAULT_SHUTDOWN_TIMEOUT_S, is_pid_alive_fn);
    }

    let _ = wait_for_keeper_exit_fn(DEFAULT_SHUTDOWN_TIMEOUT_S);

    Ok(KillSummary {
        project_id: context.project.project_id.clone(),
        state: last_phase,
        socket_path: context.paths.ccbd_socket_path().to_string(),
        forced: force,
        cleanup_summaries: Vec::new(),
        worktree_warnings: Vec::new(),
    })
}
