//! Mirrors Python `lib/cli/services/daemon_runtime/lifecycle_start.py`.

use crate::services::daemon_runtime::models::{CcbdServiceError, DaemonHandle};
use serde_json::Value;

/// State tracking for daemon startup process.
///
/// Mirrors Python `DaemonStartState` dataclass.
#[derive(Debug, Clone, Default)]
pub struct DaemonStartState {
    pub keeper_started: bool,
    pub started: bool,
    pub incompatible_restart_requested: bool,
    pub unreachable_restart_requested: bool,
}

/// Poll one iteration of daemon startup.
///
/// Mirrors Python `poll_daemon_start_iteration(...)`.
/// Uses closure injection for all daemon operations.
pub fn poll_daemon_start_iteration<I, C, S, R, K>(
    state: &mut DaemonStartState,
    inspect_daemon_fn: I,
    connect_compatible_daemon_fn: C,
    should_restart_unreachable_daemon_fn: S,
    restart_unreachable_daemon_fn: R,
    ensure_keeper_started_fn: K,
) -> Option<DaemonHandle>
where
    I: Fn() -> (Value, Value, Value),
    C: Fn(&Value, &Value, bool) -> Option<DaemonHandle>,
    S: Fn(&Value) -> bool,
    R: Fn(&Value),
    K: Fn() -> bool,
{
    let (_manager, _guard, inspection) = inspect_daemon_fn();

    // Try to connect
    let handle = maybe_connect_daemon(&inspection, state, &connect_compatible_daemon_fn);
    if handle.is_some() {
        return handle;
    }

    // Try to restart unreachable daemon
    if maybe_restart_unreachable_daemon(
        &inspection,
        state,
        &should_restart_unreachable_daemon_fn,
        &restart_unreachable_daemon_fn,
    ) {
        return None;
    }

    // Request spawn if needed
    maybe_request_spawn(&inspection, state, ensure_keeper_started_fn);

    None
}

/// Try to connect to daemon.
///
/// Mirrors Python `maybe_connect_daemon(...)`.
fn maybe_connect_daemon<C>(
    inspection: &Value,
    state: &mut DaemonStartState,
    connect_compatible_daemon_fn: C,
) -> Option<DaemonHandle>
where
    C: Fn(&Value, &Value, bool) -> Option<DaemonHandle>,
{
    let phase = _phase(inspection);
    if phase != "mounted" {
        return None;
    }

    let socket_connectable = inspection
        .get("socket_connectable")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !socket_connectable {
        return None;
    }

    let handle = connect_compatible_daemon_fn(
        inspection,
        inspection,
        !state.incompatible_restart_requested,
    );

    if let Some(h) = handle {
        return Some(DaemonHandle {
            client: h.client,
            inspection: inspection.clone(),
            started: state.started,
        });
    }

    if !state.incompatible_restart_requested {
        state.started = true;
        state.incompatible_restart_requested = true;
    }

    None
}

/// Try to restart unreachable daemon.
///
/// Mirrors Python `maybe_restart_unreachable_daemon(...)`.
fn maybe_restart_unreachable_daemon<S, R>(
    inspection: &Value,
    state: &mut DaemonStartState,
    should_restart_unreachable_daemon_fn: S,
    restart_unreachable_daemon_fn: R,
) -> bool
where
    S: Fn(&Value) -> bool,
    R: Fn(&Value),
{
    if state.unreachable_restart_requested {
        return false;
    }

    let phase = _phase(inspection);
    if phase != "mounted" && phase != "failed" {
        return false;
    }

    if !should_restart_unreachable_daemon_fn(inspection) {
        return false;
    }

    restart_unreachable_daemon_fn(inspection);
    state.started = true;
    state.unreachable_restart_requested = true;
    true
}

/// Request spawn if daemon should start.
///
/// Mirrors Python `maybe_request_spawn(...)`.
fn maybe_request_spawn<K>(
    inspection: &Value,
    state: &mut DaemonStartState,
    ensure_keeper_started_fn: K,
) where
    K: Fn() -> bool,
{
    if _desired_state(inspection) != "running" {
        return;
    }

    let phase = _phase(inspection);
    if phase != "unmounted" && phase != "failed" {
        return;
    }

    let health = inspection.get("health").and_then(|v| v.as_str());
    let should_spawn = matches!(health, Some("missing") | Some("unmounted") | Some("stale"));
    if !should_spawn {
        return;
    }

    state.started = true;
    if state.keeper_started {
        return;
    }
    state.keeper_started = ensure_keeper_started_fn();
}

/// Finalize daemon start with error handling.
///
/// Mirrors Python `finalize_daemon_start(...)`.
/// Uses closure injection for inspect and connect operations.
pub fn finalize_daemon_start<I, C, E>(
    started: bool,
    inspect_daemon_fn: I,
    connect_compatible_daemon_fn: C,
    incompatible_daemon_error_fn: E,
) -> Result<DaemonHandle, CcbdServiceError>
where
    I: Fn() -> (Value, Value, Value),
    C: Fn(&Value, &Value, bool) -> Option<DaemonHandle>,
    E: FnOnce() -> String,
{
    let (_manager, _guard, inspection) = inspect_daemon_fn();
    let phase = _phase(&inspection);

    if phase == "mounted" {
        let handle = connect_compatible_daemon_fn(&inspection, &inspection, false);
        if let Some(h) = handle {
            return Ok(DaemonHandle {
                client: h.client,
                inspection: inspection.clone(),
                started,
            });
        }

        let socket_connectable = inspection
            .get("socket_connectable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if socket_connectable {
            return Err(CcbdServiceError(incompatible_daemon_error_fn()));
        }
    }

    if phase == "starting" {
        let stage = inspection.get("startup_stage").and_then(|v| v.as_str());
        if let Some(s) = stage {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Err(CcbdServiceError(format!(
                    "ccbrd is unavailable: lifecycle_starting(stage={})",
                    trimmed
                )));
            }
        }
        return Err(CcbdServiceError(
            "ccbrd is unavailable: lifecycle_starting".to_string(),
        ));
    }

    if phase == "stopping" {
        return Err(CcbdServiceError(
            "ccbrd is unavailable: lifecycle_stopping".to_string(),
        ));
    }

    let failure_reason = inspection
        .get("last_failure_reason")
        .and_then(|v| v.as_str());
    if phase == "failed" {
        if let Some(reason) = failure_reason {
            let trimmed = reason.trim();
            if !trimmed.is_empty() {
                let inspection_reason = inspection
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                return Err(CcbdServiceError(format!(
                    "ccbrd is unavailable: {}; lifecycle_failure: {}",
                    inspection_reason, trimmed
                )));
            }
        }
    }

    let inspection_reason = inspection
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    Err(CcbdServiceError(format!(
        "ccbrd is unavailable: {}",
        inspection_reason
    )))
}

/// Extract phase from inspection.
fn _phase(inspection: &Value) -> &'static str {
    let phase = inspection.get("phase").and_then(|v| v.as_str());
    if let Some(p) = phase {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            // Convert to owned string to avoid lifetime issues
            return Box::leak(trimmed.to_string().into_boxed_str());
        }
    }

    let health = inspection.get("health").and_then(|v| v.as_str());
    match health {
        Some("missing") | Some("unmounted") => "unmounted",
        Some("healthy") => "mounted",
        _ => "failed",
    }
}

/// Extract desired state from inspection.
fn _desired_state(inspection: &Value) -> &'static str {
    let desired = inspection.get("desired_state").and_then(|v| v.as_str());
    if let Some(d) = desired {
        let trimmed = d.trim();
        if !trimmed.is_empty() {
            // Convert to owned string to avoid lifetime issues
            return Box::leak(trimmed.to_string().into_boxed_str());
        }
    }
    "running"
}
