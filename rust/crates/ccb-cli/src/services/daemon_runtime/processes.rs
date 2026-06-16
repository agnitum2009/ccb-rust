//! Mirrors Python `lib/cli/services/daemon_runtime/processes.py`.

use std::thread;
use std::time::{Duration, Instant};

use crate::services::daemon_runtime::lease::mark_inspected_lease_unmounted;
use crate::services::daemon_runtime::models::CcbdServiceError;
use serde_json::Value;

/// Check if daemon should be restarted due to unreachable state.
///
/// Mirrors Python `should_restart_unreachable_daemon(inspection)`.
pub fn should_restart_unreachable_daemon(inspection: &Value) -> bool {
    let health = inspection.get("health").and_then(|v| v.as_str());
    let pid_alive = inspection
        .get("pid_alive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let socket_connectable = inspection
        .get("socket_connectable")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    health == Some("stale") && pid_alive && !socket_connectable
}

/// Restart unreachable daemon by killing it and waiting for release.
///
/// Mirrors Python `restart_unreachable_daemon(...)`.
/// Uses closure injection for all ccbd operations.
pub fn restart_unreachable_daemon<I, M, K>(
    inspection: &Value,
    shutdown_timeout_s: f64,
    inspect_daemon_fn: I,
    manager_factory: M,
    kill_pid_fn: K,
) -> Result<(), CcbdServiceError>
where
    I: Fn() -> (Value, Value, Value),
    M: Fn(Value) -> Value,
    K: Fn(i64, bool) -> bool,
{
    let lease = inspection.get("lease");
    if lease.is_none() || lease.unwrap().is_null() {
        return Ok(());
    }

    let pid = lease_pid(lease.unwrap());
    if pid <= 0 {
        return Ok(());
    }

    let pid_alive = inspection
        .get("pid_alive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let _manager = manager_factory(
        inspect_daemon_fn().0, // Pass paths context from inspect result
    );

    if pid_alive {
        // Try graceful shutdown first
        kill_pid_fn(pid, false);
        if wait_for_daemon_release(shutdown_timeout_s, &inspect_daemon_fn) {
            mark_inspected_lease_unmounted(inspection);
            return Ok(());
        }

        // Force kill if graceful shutdown failed
        kill_pid_fn(pid, true);
        if wait_for_daemon_release(shutdown_timeout_s, &inspect_daemon_fn) {
            mark_inspected_lease_unmounted(inspection);
            return Ok(());
        }

        let reason = inspection
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        return Err(CcbdServiceError(format!(
            "ccbd is unavailable: {}; pid {} did not exit",
            reason, pid
        )));
    }

    Ok(())
}

/// Extract PID from lease.
///
/// Mirrors Python `lease_pid(lease)`.
pub fn lease_pid(lease: &Value) -> i64 {
    lease.get("ccbd_pid").and_then(|v| v.as_i64()).unwrap_or(0)
}

/// Wait for daemon to be released (not running).
///
/// Mirrors Python `wait_for_daemon_release(...)`.
pub fn wait_for_daemon_release<F>(timeout_s: f64, inspect_daemon_fn: &F) -> bool
where
    F: Fn() -> (Value, Value, Value),
{
    let deadline = Instant::now() + Duration::from_secs_f64(timeout_s);
    while Instant::now() < deadline {
        let (_, _, inspection) = inspect_daemon_fn();
        let pid_alive = inspection
            .get("pid_alive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let health = inspection.get("health").and_then(|v| v.as_str());

        if !pid_alive {
            return true;
        }

        if matches!(health, Some("missing") | Some("unmounted") | Some("stale")) {
            return true;
        }

        thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Wait for PID to exit.
///
/// Mirrors Python `wait_for_pid_exit(pid, timeout_s)`.
pub fn wait_for_pid_exit<A>(pid: i64, timeout_s: f64, is_pid_alive_fn: A) -> bool
where
    A: Fn(i64) -> bool,
{
    let timeout = timeout_s.max(0.0);
    let deadline = Instant::now() + Duration::from_secs_f64(timeout);
    while Instant::now() < deadline {
        if !is_pid_alive_fn(pid) {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    !is_pid_alive_fn(pid)
}

/// Spawn ccbd process.
///
/// Mirrors Python `spawn_ccbd(context, start_timeout_s)`.
/// Uses closure injection for ccbd spawn operation.
pub fn spawn_ccbd<S>(spawn_fn: S, start_timeout_s: f64) -> Result<(), CcbdServiceError>
where
    S: Fn(f64) -> Result<(), String>,
{
    spawn_fn(start_timeout_s).map_err(CcbdServiceError)
}
