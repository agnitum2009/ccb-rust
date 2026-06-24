//! Mirrors Python `lib/cli/services/daemon_runtime/lease.py`.

use serde_json::Value;

/// Mark the inspected lease as unmounted via the manager.
///
/// Mirrors Python `mark_inspected_lease_unmounted(manager, inspection)`.
/// Simplified no-op since manager operations are injected via closures.
pub fn mark_inspected_lease_unmounted(inspection: &Value) {
    let _lease = inspection.get("lease");
    let _expected_pid = _expected_pid(_lease);
    let _expected_daemon_instance_id = _expected_daemon_instance_id(_lease);
    // Placeholder - actual implementation would call manager.mark_unmounted
}

/// Extract expected PID from lease.
///
/// Mirrors Python `_expected_pid(lease)`.
fn _expected_pid(lease: Option<&Value>) -> Option<i64> {
    let lease = lease?;
    let pid_val = lease.get("ccbd_pid").and_then(|v| v.as_i64()).unwrap_or(0);
    if pid_val <= 0 {
        None
    } else {
        Some(pid_val)
    }
}

/// Extract expected daemon instance ID from lease.
///
/// Mirrors Python `_expected_daemon_instance_id(lease)`.
fn _expected_daemon_instance_id(lease: Option<&Value>) -> Option<String> {
    let lease = lease?;
    let instance_id = lease.get("daemon_instance_id")?.as_str()?;
    let trimmed = instance_id.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
