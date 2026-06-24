//! Mirrors Python `lib/ccbrd/reload_transaction_preflight.py`.

use crate::app::CcbdApp;
use crate::reload_transaction_context::TransactionContext;
use crate::reload_transaction_models::ReloadPublishTransactionResult;
use serde_json::Value;

/// Check for an initial failure before writing signatures.
pub fn initial_failure(
    _app: &CcbdApp,
    _context: &TransactionContext,
    namespace_patch: Option<Value>,
    runtime_mount: Option<Value>,
) -> Option<ReloadPublishTransactionResult> {
    if let Some(patch) = &namespace_patch {
        if let Some(status) = patch.get("status").and_then(|v| v.as_str()) {
            if status != "applied" {
                return Some(blocked_preflight_result(
                    "namespace_patch_not_applied",
                    "namespace patch did not reach applied state",
                    namespace_patch,
                    runtime_mount,
                ));
            }
        }
    }
    None
}

fn blocked_preflight_result(
    reason: &str,
    message: &str,
    namespace_patch: Option<Value>,
    runtime_mount: Option<Value>,
) -> ReloadPublishTransactionResult {
    ReloadPublishTransactionResult {
        status: "blocked".to_string(),
        published_graph_version: None,
        old_graph_version: None,
        new_graph_version: None,
        old_config_signature: None,
        new_config_signature: None,
        namespace_patch,
        runtime_mount,
        lease: None,
        lifecycle: None,
        diagnostics: serde_json::json!({
            "reason": reason,
            "message": message,
            "graph_published": false,
            "lease_or_lifecycle_written": false,
            "config_watch_started": false,
            "unload_or_replace_executed": false,
        }),
    }
}
