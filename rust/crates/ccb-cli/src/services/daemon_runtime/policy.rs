//! Mirrors Python `lib/cli/services/daemon_runtime/policy.py`.
//!
//! Startup / RPC timeout policy. Values are read from the environment at call
//! time (matching the Python module-level constants computed at import), with
//! the same defaults and floor constraints.

/// Read a float env var, falling back to `default` when unset/empty/invalid.
fn float_env(name: &str, default: f64) -> f64 {
    match std::env::var(name) {
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                default
            } else {
                trimmed.parse::<f64>().unwrap_or(default)
            }
        }
        Err(_) => default,
    }
}

/// Overall startup transaction timeout (floor 0.1s).
pub fn startup_transaction_timeout_s() -> f64 {
    float_env("CCB_STARTUP_TRANSACTION_TIMEOUT_S", 20.0).max(0.1)
}

/// Allowed startup progress stall duration (floor 0.0s).
pub fn startup_progress_stall_timeout_s() -> f64 {
    float_env("CCB_STARTUP_PROGRESS_STALL_TIMEOUT_S", 0.0).max(0.0)
}

/// Keeper readiness probe timeout (floor 0.1s).
pub fn keeper_ready_timeout_s() -> f64 {
    float_env("CCB_KEEPER_READY_TIMEOUT_S", 2.0).max(0.1)
}

/// Control-plane RPC timeout (floor 0.1s).
pub fn control_plane_rpc_timeout_s() -> f64 {
    float_env("CCB_CONTROL_PLANE_RPC_TIMEOUT_S", 0.5).max(0.1)
}

/// Foreground-attach RPC timeout (floor 0.1s).
pub fn foreground_attach_rpc_timeout_s() -> f64 {
    float_env("CCB_FOREGROUND_ATTACH_RPC_TIMEOUT_S", 3.0).max(0.1)
}

/// Foreground-attach target-ready timeout, capped at the startup transaction
/// timeout (floor 0.1s).
pub fn foreground_attach_target_ready_timeout_s() -> f64 {
    startup_transaction_timeout_s()
        .min(float_env("CCB_FOREGROUND_ATTACH_TARGET_READY_TIMEOUT_S", 10.0).max(0.1))
}

/// Legacy alias for `startup_transaction_timeout_s`.
pub fn start_timeout_s() -> f64 {
    startup_transaction_timeout_s()
}
