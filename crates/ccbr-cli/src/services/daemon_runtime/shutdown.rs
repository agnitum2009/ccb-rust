//! Mirrors Python `lib/cli/services/daemon_runtime/shutdown.py`.

use std::fs;
use std::path::Path;

use crate::services::daemon_runtime::lease::mark_inspected_lease_unmounted;
use crate::services::daemon_runtime::models::{CcbdServiceError, KillSummary};
use serde_json::Value;

/// Request shutdown via client or mark unmounted.
///
/// Mirrors Python `_request_shutdown_or_mark_unmounted(...)`.
/// Uses closure injection for client factory.
fn _request_shutdown_or_mark_unmounted<C>(
    inspection: &Value,
    _manager: Value,
    _force: bool,
    client_factory: C,
) -> Result<(), CcbdServiceError>
where
    C: FnOnce() -> Value,
{
    let socket_connectable = inspection
        .get("socket_connectable")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if socket_connectable {
        let _client = client_factory();
        // client.shutdown() call would go here
        // For now, placeholder - actual ccbrd client call injected via closure
        Ok(())
    } else {
        mark_inspected_lease_unmounted(inspection);
        Ok(())
    }
}

/// Wait for daemon to shutdown.
///
/// Mirrors Python `_wait_for_daemon_shutdown(...)`.
/// Uses closure injection for process operations.
fn _wait_for_daemon_shutdown<W, T, A>(
    daemon_pid: i64,
    inspection: &Value,
    _manager: Value,
    shutdown_timeout_s: f64,
    wait_for_pid_exit_fn: W,
    terminate_pid_tree_fn: T,
    is_pid_alive_fn: A,
) where
    W: Fn(i64, f64) -> bool,
    T: Fn(i64, f64, A) -> bool,
    A: Fn(i64) -> bool + Copy,
{
    if daemon_pid <= 0 {
        return;
    }
    let pid_alive = inspection
        .get("pid_alive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !pid_alive {
        return;
    }

    if !wait_for_pid_exit_fn(daemon_pid, shutdown_timeout_s) {
        terminate_pid_tree_fn(daemon_pid, shutdown_timeout_s, is_pid_alive_fn);
    }

    if !is_pid_alive_fn(daemon_pid) {
        mark_inspected_lease_unmounted(inspection);
    }
}

/// Wait for keeper to shutdown.
///
/// Mirrors Python `_wait_for_keeper_shutdown(...)`.
/// Uses closure injection for process operations.
fn _wait_for_keeper_shutdown<W, T, A>(
    keeper_pid: i64,
    shutdown_timeout_s: f64,
    wait_for_keeper_exit_fn: W,
    terminate_pid_tree_fn: T,
    is_pid_alive_fn: A,
) where
    W: Fn(f64) -> bool,
    T: Fn(i64, f64, A) -> bool,
    A: Fn(i64) -> bool + Copy,
{
    if keeper_pid <= 0 {
        return;
    }

    if !wait_for_keeper_exit_fn(shutdown_timeout_s) {
        terminate_pid_tree_fn(keeper_pid, shutdown_timeout_s, is_pid_alive_fn);
    }
}

/// Unlink socket if forced shutdown.
///
/// Mirrors Python `_unlink_socket_if_forced(...)`.
fn _unlink_socket_if_forced(socket_path: &Path, force: bool) {
    if !force {
        return;
    }
    let _ = fs::remove_file(socket_path);
}

/// Shutdown daemon with cleanup.
///
/// Mirrors Python `shutdown_daemon(...)`.
/// Uses closure injection for all ccbrd and process operations.
#[allow(clippy::too_many_arguments)]
pub fn shutdown_daemon<R, F, I, C, L, K, W, E, T, A>(
    record_shutdown_intent_fn: R,
    finalize_shutdown_lifecycle_fn: F,
    inspect_daemon_fn: I,
    client_factory: C,
    lease_pid_fn: L,
    keeper_pid_fn: K,
    wait_for_pid_exit_fn: W,
    wait_for_keeper_exit_fn: E,
    is_pid_alive_fn: A,
    terminate_pid_tree_fn: T,
    shutdown_timeout_s: f64,
    force: bool,
    socket_path: &Path,
    project_id: &str,
) -> Result<KillSummary, CcbdServiceError>
where
    R: FnOnce(&str, &str, u32),
    F: FnOnce(&Path),
    I: Fn() -> (Value, Value, Value),
    C: FnOnce() -> Value,
    L: Fn(&Value) -> i64,
    K: Fn(&Value) -> i64,
    W: Fn(i64, f64) -> bool,
    E: Fn(f64) -> bool,
    A: Fn(i64) -> bool + Copy,
    T: Fn(i64, f64, A) -> bool,
{
    record_shutdown_intent_fn(project_id, "kill", std::process::id());

    let (manager, _guard, inspection) = inspect_daemon_fn();
    let lease = inspection.get("lease").unwrap_or(&Value::Null);
    let daemon_pid = lease_pid_fn(lease);
    let keeper_pid = keeper_pid_fn(lease);

    _request_shutdown_or_mark_unmounted(&inspection, manager.clone(), force, client_factory)?;

    _wait_for_daemon_shutdown(
        daemon_pid,
        &inspection,
        manager,
        shutdown_timeout_s,
        &wait_for_pid_exit_fn,
        &terminate_pid_tree_fn,
        is_pid_alive_fn,
    );

    _wait_for_keeper_shutdown(
        keeper_pid,
        shutdown_timeout_s,
        wait_for_keeper_exit_fn,
        &terminate_pid_tree_fn,
        is_pid_alive_fn,
    );

    _unlink_socket_if_forced(socket_path, force);
    finalize_shutdown_lifecycle_fn(socket_path);

    let (_manager2, _guard2, final_inspection) = inspect_daemon_fn();
    let state = _inspection_phase(&final_inspection);

    Ok(KillSummary {
        project_id: project_id.to_string(),
        state,
        socket_path: socket_path.to_string_lossy().to_string(),
        forced: force,
        cleanup_summaries: vec![],
        worktree_warnings: vec![],
    })
}

/// Extract phase from inspection.
fn _inspection_phase(inspection: &Value) -> String {
    let phase = inspection.get("phase").and_then(|v| v.as_str());
    if let Some(p) = phase {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    // Fallback to lease mount_state
    if let Some(lease) = inspection.get("lease") {
        if let Some(mount_state) = lease.get("mount_state") {
            if let Some(value) = mount_state.get("value") {
                if let Some(state) = value.as_str() {
                    let trimmed = state.trim();
                    if !trimmed.is_empty() {
                        return trimmed.to_string();
                    }
                }
            }
        }
    }

    "unmounted".to_string()
}
