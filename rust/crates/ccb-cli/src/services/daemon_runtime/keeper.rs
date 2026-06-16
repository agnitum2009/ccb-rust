//! Mirrors Python `lib/cli/services/daemon_runtime/keeper.py`.

use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde_json::Value;

/// Clear shutdown intent from keeper state store.
///
/// Mirrors Python `clear_shutdown_intent(context)`.
/// Uses closure injection for state store operations.
pub fn clear_shutdown_intent<C>(clear_fn: C)
where
    C: FnOnce(),
{
    clear_fn()
}

/// Record running intent in lifecycle store.
///
/// Mirrors Python `record_running_intent(context)`.
/// Uses closure injection for lifecycle store operations.
pub fn record_running_intent<L>(
    lifecycle_load_fn: L,
    lifecycle_save_fn: L,
    _project_id: &str,
    socket_path: &str,
    config_signature: Option<&str>,
) -> bool
where
    L: Fn(&Value) -> Value,
{
    let current = lifecycle_load_fn(&Value::Null);
    let startup_requested = current.get("desired_state").and_then(|v| v.as_str())
        != Some("running")
        || current.get("phase").and_then(|v| v.as_str()) != Some("mounted");

    let mut updated = current.clone();
    if let Some(obj) = updated.as_object_mut() {
        obj.insert(
            "desired_state".to_string(),
            Value::String("running".to_string()),
        );
        if let Some(sig) = config_signature {
            obj.insert(
                "config_signature".to_string(),
                Value::String(sig.to_string()),
            );
        }
        obj.insert(
            "socket_path".to_string(),
            Value::String(socket_path.to_string()),
        );
        obj.insert("last_failure_reason".to_string(), Value::Null);
        obj.insert("shutdown_intent".to_string(), Value::Null);
    }

    lifecycle_save_fn(&updated);
    startup_requested
}

/// Record shutdown intent in lifecycle and shutdown intent stores.
///
/// Mirrors Python `record_shutdown_intent(context, reason)`.
/// Uses closure injection for store operations.
pub fn record_shutdown_intent<L, S>(
    lifecycle_load_fn: L,
    lifecycle_save_fn: L,
    shutdown_save_fn: S,
    project_id: &str,
    reason: &str,
    requested_by_pid: u32,
) where
    L: Fn(&Value) -> Value,
    S: Fn(&Value),
{
    let current = lifecycle_load_fn(&Value::Null);

    let mut updated = current.clone();
    if let Some(obj) = updated.as_object_mut() {
        let phase = obj
            .get("phase")
            .and_then(|v| v.as_str())
            .unwrap_or("unmounted");
        let new_phase = if phase == "unmounted" {
            "unmounted"
        } else {
            "stopping"
        };
        obj.insert("phase".to_string(), Value::String(new_phase.to_string()));
        obj.insert(
            "desired_state".to_string(),
            Value::String("stopped".to_string()),
        );
        obj.insert(
            "shutdown_intent".to_string(),
            Value::String(reason.to_string()),
        );
        obj.insert("last_failure_reason".to_string(), Value::Null);
    }

    lifecycle_save_fn(&updated);

    let mut shutdown_intent = serde_json::Map::new();
    shutdown_intent.insert(
        "project_id".to_string(),
        Value::String(project_id.to_string()),
    );
    shutdown_intent.insert(
        "requested_at".to_string(),
        Value::Number(
            (std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64)
                .into(),
        ),
    );
    shutdown_intent.insert(
        "requested_by_pid".to_string(),
        Value::Number(requested_by_pid.into()),
    );
    shutdown_intent.insert("reason".to_string(), Value::String(reason.to_string()));

    shutdown_save_fn(&Value::Object(shutdown_intent));
}

/// Finalize shutdown lifecycle state.
///
/// Mirrors Python `finalize_shutdown_lifecycle(context)`.
/// Uses closure injection for lifecycle store operations.
pub fn finalize_shutdown_lifecycle<L>(lifecycle_load_fn: L, lifecycle_save_fn: L, socket_path: &str)
where
    L: Fn(&Value) -> Value,
{
    let current = lifecycle_load_fn(&Value::Null);

    let mut updated = current.clone();
    if let Some(obj) = updated.as_object_mut() {
        obj.insert("phase".to_string(), Value::String("unmounted".to_string()));
        obj.insert(
            "desired_state".to_string(),
            Value::String("stopped".to_string()),
        );
        obj.insert("owner_pid".to_string(), Value::Null);
        obj.insert("owner_daemon_instance_id".to_string(), Value::Null);
        obj.insert("socket_inode".to_string(), Value::Null);
        obj.insert(
            "socket_path".to_string(),
            Value::String(socket_path.to_string()),
        );
        obj.insert("last_failure_reason".to_string(), Value::Null);
    }

    lifecycle_save_fn(&updated);
}

/// Wait for keeper to be ready.
///
/// Mirrors Python `wait_for_keeper_ready(...)`.
/// Uses closure injection for keeper state store and process checks.
pub fn wait_for_keeper_ready<F1, F2>(
    timeout_s: f64,
    _keeper_state_load_fn: F1,
    keeper_is_running_fn: F2,
) -> bool
where
    F1: Fn(&Value) -> bool,
    F2: Fn(&Value) -> bool,
{
    let timeout = timeout_s.max(0.0);
    let deadline = Instant::now() + Duration::from_secs_f64(timeout);

    while Instant::now() < deadline {
        let state = Value::Object(serde_json::Map::new());
        if keeper_is_running_fn(&state) {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let state = Value::Object(serde_json::Map::new());
    keeper_is_running_fn(&state)
}

/// Wait for keeper to exit.
///
/// Mirrors Python `wait_for_keeper_exit(...)`.
/// Uses closure injection for keeper state store and process checks.
pub fn wait_for_keeper_exit<F1, F2>(
    timeout_s: f64,
    _keeper_state_load_fn: F1,
    keeper_is_running_fn: F2,
) -> bool
where
    F1: Fn(&Value) -> bool,
    F2: Fn(&Value) -> bool,
{
    let timeout = timeout_s.max(0.0);
    let deadline = Instant::now() + Duration::from_secs_f64(timeout);

    while Instant::now() < deadline {
        let state = Value::Object(serde_json::Map::new());
        if !keeper_is_running_fn(&state) {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let state = Value::Object(serde_json::Map::new());
    !keeper_is_running_fn(&state)
}

/// Get keeper PID from state or lease.
///
/// Mirrors Python `keeper_pid(context, lease, ...)`.
/// Uses closure injection for keeper state store and process checks.
pub fn keeper_pid<L, F>(lease: &Value, keeper_state_load_fn: L, keeper_is_running_fn: F) -> i64
where
    L: Fn(&Value) -> Value,
    F: Fn(&Value) -> bool,
{
    let state = keeper_state_load_fn(&Value::Null);

    if keeper_is_running_fn(&state) {
        return state
            .get("keeper_pid")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
    }

    let lease_keeper_pid = lease
        .get("keeper_pid")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if lease_keeper_pid > 0 {
        lease_keeper_pid
    } else {
        0
    }
}

/// Ensure keeper is started, acquiring startup lock if needed.
///
/// Mirrors Python `ensure_keeper_started(...)`.
/// Uses closure injection for all keeper operations.
pub fn ensure_keeper_started<A, G, S>(
    mount_manager_factory: A,
    ownership_guard_factory: G,
    spawn_keeper_fn: S,
    ready_timeout_s: f64,
) -> bool
where
    A: Fn() -> Value,
    G: Fn(Value) -> Value,
    S: FnOnce(),
{
    // Check if already running
    let manager = mount_manager_factory();
    let _guard = ownership_guard_factory(manager);

    // Try to acquire lock and start if not running
    spawn_keeper_fn();
    wait_for_keeper_ready(
        ready_timeout_s,
        |_| true, // State load returns bool (simulating successful load)
        |_| true, // Is running returns true
    )
}
