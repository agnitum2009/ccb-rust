//! Mirrors Python `lib/cli/services/daemon_runtime/facade.py`.
//!
//! Thin facade that re-exports functions from other modules with policy defaults.
//! All ccbrd-specific operations are injected via closures.

use crate::services::daemon_runtime::keeper;
use crate::services::daemon_runtime::policy;
use crate::services::daemon_runtime::processes;

/// Shutdown timeout constant (seconds).
pub const SHUTDOWN_TIMEOUT_S: f64 = 2.0;

/// Start timeout (uses policy startup transaction timeout).
pub const START_TIMEOUT_S: f64 = 0.0; // Placeholder, calls policy at runtime

/// Return error message for incompatible daemon.
///
/// Mirrors Python `incompatible_daemon_error()`.
pub fn incompatible_daemon_error() -> String {
    "mounted ccbrd config does not match current .ccbr/ccbr.config".to_string()
}

/// Ensure keeper is started with default timeouts.
///
/// Mirrors Python `ensure_keeper_started(context)`.
/// Uses closure injection for mount manager and ownership guard factories.
pub fn ensure_keeper_started<A, G>(mount_manager_factory: A, ownership_guard_factory: G) -> bool
where
    A: Fn() -> serde_json::Value,
    G: Fn(serde_json::Value) -> serde_json::Value,
{
    keeper::ensure_keeper_started(
        mount_manager_factory,
        ownership_guard_factory,
        || {}, // spawn_keeper_fn closure
        policy::keeper_ready_timeout_s(),
    )
}

/// Wait for keeper to exit with timeout.
///
/// Mirrors Python `wait_for_keeper_exit(context, timeout_s)`.
/// Uses closure injection for keeper state operations.
pub fn wait_for_keeper_exit<L, F>(
    timeout_s: f64,
    keeper_state_load_fn: L,
    keeper_is_running_fn: F,
) -> bool
where
    L: Fn(&serde_json::Value) -> bool,
    F: Fn(&serde_json::Value) -> bool,
{
    keeper::wait_for_keeper_exit(timeout_s, keeper_state_load_fn, keeper_is_running_fn)
}

/// Get keeper PID from state or lease.
///
/// Mirrors Python `keeper_pid(context, lease)`.
/// Uses closure injection for keeper state operations.
pub fn keeper_pid<L, F>(
    lease: &serde_json::Value,
    keeper_state_load_fn: L,
    keeper_is_running_fn: F,
) -> i64
where
    L: Fn(&serde_json::Value) -> serde_json::Value,
    F: Fn(&serde_json::Value) -> bool,
{
    keeper::keeper_pid(lease, keeper_state_load_fn, keeper_is_running_fn)
}

/// Check if unreachable daemon should be restarted.
///
/// Mirrors Python `should_restart_unreachable_daemon(inspection)`.
pub fn should_restart_unreachable_daemon(inspection: &serde_json::Value) -> bool {
    processes::should_restart_unreachable_daemon(inspection)
}

/// Spawn ccbrd process with default startup timeout.
///
/// Mirrors Python `spawn_ccbrd_process(context)`.
/// Uses closure injection for spawn operation.
pub fn spawn_ccbrd_process<S>(
    spawn_fn: S,
) -> Result<(), crate::services::daemon_runtime::models::CcbdServiceError>
where
    S: Fn(f64) -> Result<(), String>,
{
    processes::spawn_ccbrd(spawn_fn, policy::startup_transaction_timeout_s())
}
