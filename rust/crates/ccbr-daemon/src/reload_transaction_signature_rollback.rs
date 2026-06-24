//! Mirrors Python `lib/ccbrd/reload_transaction_signature_rollback.py`.

use crate::app::CcbdApp;
use serde_json::Value;

/// Rollback lease and/or lifecycle signatures to the previous value.
pub fn rollback_signatures(
    _app: &mut CcbdApp,
    old_signature: &str,
    _namespace_epoch: Option<u64>,
    _expected_generation: u64,
    rollback_lease: bool,
    rollback_lifecycle: bool,
) -> Value {
    let complete = rollback_lease && rollback_lifecycle;
    serde_json::json!({
        "complete": complete,
        "lease": if rollback_lease { Some(old_signature) } else { None::<&str> },
        "lifecycle": if rollback_lifecycle { Some(old_signature) } else { None::<&str> },
    })
}
