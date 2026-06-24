//! Mirrors Python `lib/cli/services/daemon_runtime/compat.py`.

use std::thread;
use std::time::{Duration, Instant};

use crate::services::daemon_runtime::models::{CcbdServiceError, DaemonHandle};
use serde_json::Value;

/// Check if daemon matches project config via client ping.
///
/// Mirrors Python `daemon_matches_project_config(context, client)`.
/// Uses closure injection for config payload and client ping.
pub fn daemon_matches_project_config<P, I>(config_identity_payload_fn: P, client_ping_fn: I) -> bool
where
    P: FnOnce() -> Value,
    I: FnOnce(&str) -> Value,
{
    let expected = config_identity_payload_fn();
    let payload = client_ping_fn("ccbrd");

    let actual_signature = payload
        .get("config_signature")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if let Some(sig) = actual_signature {
        let expected_sig = expected
            .get("config_signature")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if sig == expected_sig {
            return true;
        }
        // Config drift is reload-pending, still compatible
        return true;
    }

    // Fallback to known_agents comparison
    let known_agents = payload.get("known_agents");
    if let Some(arr) = known_agents.and_then(|v| v.as_array()) {
        let actual_agents: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let expected_agents: Vec<String> = expected
            .get("known_agents")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        return actual_agents == expected_agents;
    }

    false
}

/// Check if inspection matches project config.
///
/// Mirrors Python `inspection_matches_project_config(context, inspection)`.
/// Uses closure injection for config payload.
pub fn inspection_matches_project_config<P>(
    inspection: &Value,
    config_identity_payload_fn: P,
) -> bool
where
    P: FnOnce() -> Value,
{
    let expected = config_identity_payload_fn();
    let actual_signature = _inspection_config_signature(inspection);

    if let Some(sig) = actual_signature {
        let expected_sig = expected
            .get("config_signature")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        return !sig.is_empty() && sig == expected_sig;
    }

    false
}

/// Connect to compatible daemon, optionally restarting on mismatch.
///
/// Mirrors Python `connect_compatible_daemon(...)`.
/// Uses closure injection for client factories and config matching.
pub fn connect_compatible_daemon<P, R, D, S>(
    inspection: &Value,
    restart_on_mismatch: bool,
    probe_client_factory: P,
    runtime_client_factory: R,
    daemon_matches_project_config_fn: D,
    inspection_matches_project_config_fn: S,
    shutdown_incompatible_daemon_fn: Option<Box<dyn FnOnce(Value, Value)>>,
) -> Option<DaemonHandle>
where
    P: FnOnce() -> Value,
    R: FnOnce() -> Value,
    D: FnOnce() -> bool,
    S: FnOnce() -> bool,
{
    let socket_connectable = inspection
        .get("socket_connectable")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !socket_connectable {
        return None;
    }

    if inspection_matches_project_config_fn() {
        let client = runtime_client_factory();
        return Some(DaemonHandle {
            client: Some(client),
            inspection: inspection.clone(),
            started: false,
        });
    }

    let _probe_client = probe_client_factory();
    let matches_config = daemon_matches_project_config_fn();

    if matches_config {
        let client = runtime_client_factory();
        return Some(DaemonHandle {
            client: Some(client),
            inspection: inspection.clone(),
            started: false,
        });
    }

    if !restart_on_mismatch {
        return None;
    }

    if let Some(shutdown_fn) = shutdown_incompatible_daemon_fn {
        let runtime_client = runtime_client_factory();
        shutdown_fn(runtime_client, Value::Null);
        None
    } else {
        None
    }
}

/// Extract config signature from inspection.
fn _inspection_config_signature(inspection: &Value) -> Option<String> {
    if let Some(lifecycle) = inspection.get("lifecycle") {
        if let Some(sig) = lifecycle.get("config_signature").and_then(|v| v.as_str()) {
            let trimmed = sig.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    if let Some(lease) = inspection.get("lease") {
        if let Some(sig) = lease.get("config_signature").and_then(|v| v.as_str()) {
            let trimmed = sig.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    None
}

/// Shutdown incompatible daemon.
///
/// Mirrors Python `shutdown_incompatible_daemon(...)`.
/// Uses closure injection for client and inspect operations.
pub fn shutdown_incompatible_daemon<C, I, H>(
    client: C,
    inspect_daemon_fn: I,
    unavailable_health_states: H,
    shutdown_timeout_s: f64,
    incompatible_daemon_error: &str,
) -> Result<(), CcbdServiceError>
where
    C: FnOnce() -> Value,
    I: Fn() -> (Value, Value, Value),
    H: Fn(&str) -> bool,
{
    let _client = client();
    // client.stop_all(force=false) would go here
    // Placeholder for ccbrd client call

    let deadline = Instant::now() + Duration::from_secs_f64(shutdown_timeout_s);
    while Instant::now() < deadline {
        let (_, _, inspection) = inspect_daemon_fn();
        let socket_connectable = inspection
            .get("socket_connectable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let health = inspection.get("health").and_then(|v| v.as_str());

        if !socket_connectable || unavailable_health_states(health.unwrap_or("")) {
            return Ok(());
        }

        thread::sleep(Duration::from_millis(50));
    }

    Err(CcbdServiceError(format!(
        "{}; old ccbrd did not shut down in time",
        incompatible_daemon_error
    )))
}
