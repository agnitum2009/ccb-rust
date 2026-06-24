//! Mirrors Python `lib/cli/services/daemon_runtime/lifecycle.py`.

use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::services::daemon_runtime::models::{CcbdServiceError, DaemonHandle};
use serde_json::Value;

use super::lifecycle_start::{
    finalize_daemon_start, poll_daemon_start_iteration, DaemonStartState,
};

/// Ensure daemon is started, polling until ready or timeout.
///
/// Mirrors Python `ensure_daemon_started(...)`.
/// Uses closure injection for all daemon operations.
#[allow(clippy::too_many_arguments)]
pub fn ensure_daemon_started<C, R, I, K, S, E, RUR, ER>(
    clear_shutdown_intent_fn: C,
    record_running_intent_fn: R,
    ensure_keeper_started_fn: I,
    inspect_daemon_fn: K,
    connect_compatible_daemon_fn: S,
    should_restart_unreachable_daemon_fn: E,
    restart_unreachable_daemon_fn: RUR,
    incompatible_daemon_error_fn: ER,
    start_timeout_s: f64,
    progress_stall_timeout_s: f64,
) -> Result<DaemonHandle, CcbdServiceError>
where
    C: FnOnce(),
    R: FnOnce() -> bool,
    I: Fn() -> bool,
    K: Fn() -> (Value, Value, Value),
    S: Fn(&Value, &Value, bool) -> Option<DaemonHandle>,
    E: Fn(&Value) -> bool,
    RUR: Fn(&Value),
    ER: FnOnce() -> String,
{
    clear_shutdown_intent_fn();
    let startup_requested = record_running_intent_fn();

    let mut state = DaemonStartState {
        keeper_started: ensure_keeper_started_fn(),
        started: startup_requested,
        incompatible_restart_requested: false,
        unreachable_restart_requested: false,
    };

    let local_deadline = Instant::now() + Duration::from_secs_f64(start_timeout_s.max(0.0));

    loop {
        let handle = poll_daemon_start_iteration(
            &mut state,
            &inspect_daemon_fn,
            &connect_compatible_daemon_fn,
            &should_restart_unreachable_daemon_fn,
            &restart_unreachable_daemon_fn,
            &ensure_keeper_started_fn,
        );

        if let Some(h) = handle {
            return Ok(h);
        }

        let (_, _, inspection) = inspect_daemon_fn();
        if _startup_wait_exhausted(&inspection, local_deadline, progress_stall_timeout_s) {
            break;
        }

        thread::sleep(Duration::from_millis(50));
    }

    finalize_daemon_start(
        state.started,
        &inspect_daemon_fn,
        &connect_compatible_daemon_fn,
        incompatible_daemon_error_fn,
    )
}

/// Connect to mounted daemon, optionally restarting stale/unreachable instances.
///
/// Mirrors Python `connect_mounted_daemon(...)`.
/// Uses closure injection for all daemon operations.
pub fn connect_mounted_daemon<I, C, S, E, ER>(
    allow_restart_stale: bool,
    inspect_daemon_fn: I,
    connect_compatible_daemon_fn: C,
    should_restart_unreachable_daemon_fn: E,
    ensure_daemon_started_fn: S,
    incompatible_daemon_error_fn: ER,
) -> Result<DaemonHandle, CcbdServiceError>
where
    I: Fn() -> (Value, Value, Value),
    C: Fn(&Value, &Value, bool) -> Option<DaemonHandle>,
    S: Fn() -> Result<DaemonHandle, CcbdServiceError>,
    E: Fn(&Value) -> bool,
    ER: FnOnce() -> String,
{
    let (_, _, inspection) = inspect_daemon_fn();
    let phase = _phase(&inspection);

    if phase == "mounted" {
        let handle = connect_compatible_daemon_fn(&inspection, &inspection, allow_restart_stale);
        if handle.is_some() {
            return handle.ok_or_else(|| CcbdServiceError("Failed to connect".to_string()));
        }
    }

    let (_, _, inspection) = inspect_daemon_fn();
    let phase = _phase(&inspection);

    if allow_restart_stale
        && _should_wait_or_recover(&inspection, &should_restart_unreachable_daemon_fn)
    {
        return ensure_daemon_started_fn();
    }

    if phase == "unmounted" {
        return Err(CcbdServiceError(
            "project ccbrd is unmounted; run `ccbr` first".to_string(),
        ));
    }
    if phase == "starting" {
        return Err(CcbdServiceError(
            "project ccbrd is starting; wait for keeper to finish startup".to_string(),
        ));
    }
    if phase == "stopping" {
        return Err(CcbdServiceError(
            "project ccbrd is stopping; wait for shutdown to finish".to_string(),
        ));
    }
    if phase == "mounted"
        && inspection
            .get("socket_connectable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        let handle = connect_compatible_daemon_fn(&inspection, &inspection, false);
        if handle.is_some() {
            return handle.ok_or_else(|| CcbdServiceError("Failed to connect".to_string()));
        }
        return Err(CcbdServiceError(incompatible_daemon_error_fn()));
    }

    let failure_reason = inspection
        .get("last_failure_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if phase == "failed" && !failure_reason.is_empty() {
        let reason = inspection
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        return Err(CcbdServiceError(format!(
            "ccbrd is unavailable: {}; lifecycle_failure: {}",
            reason, failure_reason
        )));
    }

    let reason = inspection
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    Err(CcbdServiceError(format!(
        "ccbrd is unavailable: {}",
        reason
    )))
}

/// Check if should wait for or recover daemon state.
fn _should_wait_or_recover<E>(inspection: &Value, should_restart_unreachable_daemon_fn: &E) -> bool
where
    E: Fn(&Value) -> bool,
{
    let phase = _phase(inspection);
    if _desired_state(inspection) != "running" {
        return false;
    }
    if matches!(phase, "unmounted" | "starting" | "failed") {
        return true;
    }
    if phase == "mounted" {
        let health = inspection.get("health").and_then(|v| v.as_str());
        let unhealthy = matches!(health, Some("missing") | Some("unmounted") | Some("stale"));
        if unhealthy || should_restart_unreachable_daemon_fn(inspection) {
            return true;
        }
    }
    false
}

/// Extract phase from inspection.
fn _phase(inspection: &Value) -> &'static str {
    let phase = inspection.get("phase").and_then(|v| v.as_str());
    if let Some(p) = phase {
        if !p.is_empty() {
            // Convert to owned string to avoid lifetime issues
            return Box::leak(p.to_string().into_boxed_str());
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
        if !d.is_empty() {
            // Convert to owned string to avoid lifetime issues
            return Box::leak(d.to_string().into_boxed_str());
        }
    }
    "running"
}

/// Check if startup wait time is exhausted.
fn _startup_wait_exhausted(
    inspection: &Value,
    local_deadline: Instant,
    progress_stall_timeout_s: f64,
) -> bool {
    let now = Instant::now();
    if now >= local_deadline {
        return true;
    }

    let phase = _phase(inspection);
    if phase != "starting" {
        return false;
    }

    // Check transaction deadline
    if let Some(deadline_str) = inspection
        .get("startup_deadline_at")
        .and_then(|v| v.as_str())
    {
        if let Some(deadline_ts) = _timestamp_seconds(deadline_str) {
            let deadline_secs = deadline_ts.as_secs_f64();
            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64();
            if now_secs >= deadline_secs {
                return true;
            }
        }
    }

    // Check progress stall
    if progress_stall_timeout_s <= 0.0 {
        return false;
    }
    if let Some(progress_str) = inspection.get("last_progress_at").and_then(|v| v.as_str()) {
        if let Some(progress_ts) = _timestamp_seconds(progress_str) {
            let progress_secs = progress_ts.as_secs_f64();
            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64();
            if now_secs >= progress_secs + progress_stall_timeout_s {
                return true;
            }
        }
    }

    false
}

/// Parse timestamp string to seconds since epoch.
/// Handles Unix timestamp integers and ISO 8601 / RFC 3339 strings (including `Z`).
fn _timestamp_seconds(text: &str) -> Option<Duration> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Try parsing as Unix timestamp (seconds or milliseconds)
    if let Ok(secs) = trimmed.parse::<f64>() {
        let secs = if secs > 1_000_000_000_000.0 {
            // Milliseconds since epoch
            secs / 1000.0
        } else {
            secs
        };
        return Some(Duration::from_secs_f64(secs.max(0.0)));
    }

    // Try ISO 8601 / RFC 3339 parsing (accepts trailing `Z`)
    let normalized = if let Some(stripped) = trimmed.strip_suffix('Z') {
        format!("{}+00:00", stripped)
    } else {
        trimmed.to_string()
    };
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&normalized) {
        return Some(Duration::from_secs_f64(dt.timestamp() as f64));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::daemon_runtime::models::DaemonHandle;
    use serde_json::json;

    fn make_inspection(
        phase: &str,
        socket_connectable: bool,
        startup_stage: Option<&str>,
        startup_deadline_at: Option<&str>,
    ) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("phase".into(), Value::String(phase.into()));
        obj.insert("desired_state".into(), Value::String("running".into()));
        obj.insert("socket_connectable".into(), Value::Bool(socket_connectable));
        obj.insert("health".into(), Value::String("healthy".into()));
        if let Some(stage) = startup_stage {
            obj.insert("startup_stage".into(), Value::String(stage.into()));
        }
        if let Some(deadline) = startup_deadline_at {
            obj.insert("startup_deadline_at".into(), Value::String(deadline.into()));
        }
        Value::Object(obj)
    }

    #[test]
    fn ensure_daemon_started_waits_until_mounted() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = AtomicUsize::new(0);
        let inspect = || {
            let n = calls.fetch_add(1, Ordering::SeqCst) + 1;
            if n < 3 {
                (
                    Value::Null,
                    Value::Null,
                    make_inspection("starting", false, Some("spawn_requested"), None),
                )
            } else {
                (
                    Value::Null,
                    Value::Null,
                    make_inspection("mounted", true, None, None),
                )
            }
        };
        let connect = |_current: &Value, _inspection: &Value, _restart: bool| {
            Some(DaemonHandle {
                client: Some(json!({"client": "ccbrd"})),
                inspection: Value::Null,
                started: false,
            })
        };

        let handle = ensure_daemon_started(
            || {},
            || true,
            || true,
            inspect,
            connect,
            |_inspection| false,
            |_inspection| {},
            || "incompatible".into(),
            2.0,
            0.0,
        )
        .unwrap();

        assert!(handle.client.is_some());
        assert!(handle.started);
    }

    #[test]
    fn ensure_daemon_started_uses_shared_startup_deadline() {
        let inspection = make_inspection(
            "starting",
            false,
            Some("spawn_requested"),
            Some("1970-01-01T00:00:08Z"),
        );
        let err = ensure_daemon_started(
            || {},
            || true,
            || true,
            || (Value::Null, Value::Null, inspection.clone()),
            |_current, _inspection, _restart| None,
            |_inspection| false,
            |_inspection| {},
            || "incompatible".into(),
            20.0,
            0.0,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("lifecycle_starting(stage=spawn_requested)"));
    }

    #[test]
    fn timestamp_seconds_parses_iso_with_z() {
        let ts = _timestamp_seconds("2026-04-24T00:00:04Z").unwrap();
        assert_eq!(ts.as_secs(), 1_776_988_804);
    }
}
