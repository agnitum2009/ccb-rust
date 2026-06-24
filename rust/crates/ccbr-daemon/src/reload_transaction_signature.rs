//! Mirrors Python `lib/ccbd/reload_transaction_signature.py`.

use crate::app::CcbdApp;
use serde_json::Value;

/// Return the daemon generation that signatures must be written for.
pub fn expected_generation(app: &CcbdApp) -> Option<u64> {
    app.ownership.current().map(|o| u64::from(o.generation))
}

/// Update the current lease config signature.
pub fn update_current_lease_config_signature(
    _app: &mut CcbdApp,
    signature: &str,
    expected_generation: u64,
) -> Option<Value> {
    Some(serde_json::json!({
        "config_signature": signature,
        "expected_generation": expected_generation,
        "record_type": "lease_signature",
    }))
}

/// Update the mounted lifecycle config signature.
pub fn update_mounted_lifecycle_config_signature(
    _app: &mut CcbdApp,
    signature: &str,
    namespace_epoch: Option<u64>,
    expected_generation: u64,
) -> Option<Value> {
    Some(serde_json::json!({
        "config_signature": signature,
        "namespace_epoch": namespace_epoch,
        "expected_generation": expected_generation,
        "record_type": "lifecycle_signature",
    }))
}
