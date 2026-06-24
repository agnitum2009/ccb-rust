//! Mirrors Python `lib/ccbd/reload_transaction_results.py`.

use crate::reload_apply_models::ServiceGraph;
use crate::reload_transaction_context::TransactionResultKwargs;
use crate::reload_transaction_models::ReloadPublishTransactionResult;
use crate::reload_transaction_records::rollback_record;
use serde_json::Value;

/// Build a blocked transaction result.
pub fn blocked_result(
    reason: &str,
    message: &str,
    kwargs: &TransactionResultKwargs,
) -> ReloadPublishTransactionResult {
    ReloadPublishTransactionResult {
        status: "blocked".to_string(),
        published_graph_version: None,
        old_graph_version: kwargs.old_graph_version.clone(),
        new_graph_version: kwargs.new_graph_version.clone(),
        old_config_signature: kwargs.old_config_signature.clone(),
        new_config_signature: kwargs.new_config_signature.clone(),
        namespace_patch: kwargs.namespace_patch.clone(),
        runtime_mount: kwargs.runtime_mount.clone(),
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

/// Build a failed transaction result.
pub fn failed_result(
    reason: &str,
    error: &dyn std::error::Error,
    kwargs: &TransactionResultKwargs,
    lease: Option<Value>,
    lifecycle: Option<Value>,
    lease_or_lifecycle_written: bool,
    signature_rollback: Option<Value>,
) -> ReloadPublishTransactionResult {
    ReloadPublishTransactionResult {
        status: "failed".to_string(),
        published_graph_version: None,
        old_graph_version: kwargs.old_graph_version.clone(),
        new_graph_version: kwargs.new_graph_version.clone(),
        old_config_signature: kwargs.old_config_signature.clone(),
        new_config_signature: kwargs.new_config_signature.clone(),
        namespace_patch: kwargs.namespace_patch.clone(),
        runtime_mount: kwargs.runtime_mount.clone(),
        lease,
        lifecycle,
        diagnostics: serde_json::json!({
            "reason": reason,
            "error_type": std::any::type_name_of_val(error).split("::").last().unwrap_or("Error"),
            "error": error.to_string(),
            "signature_rollback": rollback_record(signature_rollback),
            "graph_published": false,
            "lease_or_lifecycle_written": lease_or_lifecycle_written,
            "config_watch_started": false,
            "unload_or_replace_executed": false,
        }),
    }
}

/// Build a published transaction result.
pub fn published_result(
    new_graph: &ServiceGraph,
    kwargs: &TransactionResultKwargs,
    lease: Option<Value>,
    lifecycle: Option<Value>,
) -> ReloadPublishTransactionResult {
    let runtime_diagnostics: serde_json::Map<String, Value> = kwargs
        .runtime_mount
        .as_ref()
        .and_then(|v| v.get("diagnostics").and_then(|d| d.as_object().cloned()))
        .unwrap_or_default();

    ReloadPublishTransactionResult {
        status: "published".to_string(),
        published_graph_version: new_graph.version.clone(),
        old_graph_version: kwargs.old_graph_version.clone(),
        new_graph_version: kwargs.new_graph_version.clone(),
        old_config_signature: kwargs.old_config_signature.clone(),
        new_config_signature: kwargs.new_config_signature.clone(),
        namespace_patch: kwargs.namespace_patch.clone(),
        runtime_mount: kwargs.runtime_mount.clone(),
        lease,
        lifecycle,
        diagnostics: serde_json::json!({
            "reason": None::<&str>,
            "graph_published": true,
            "lease_or_lifecycle_written": true,
            "config_watch_started": false,
            "unload_or_replace_executed": runtime_diagnostics.get("unload_or_replace_executed").and_then(|v| v.as_bool()).unwrap_or(false),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reload_apply_models::ServiceGraph;
    use crate::reload_transaction_context::TransactionResultKwargs;
    use serde_json::json;

    fn sample_kwargs() -> TransactionResultKwargs {
        TransactionResultKwargs {
            old_graph_version: Some("v1".to_string()),
            new_graph_version: Some("v2".to_string()),
            old_config_signature: Some("old-sig".to_string()),
            new_config_signature: Some("new-sig".to_string()),
            namespace_patch: Some(json!({"status": "applied"})),
            runtime_mount: Some(json!({"status": "mounted"})),
        }
    }

    fn sample_graph() -> ServiceGraph {
        ServiceGraph {
            version: Some("v2".to_string()),
            config: ccb_agents::models::ProjectConfig::default(),
            config_identity: json!({}),
            config_signature: "new-sig".to_string(),
        }
    }

    #[test]
    fn blocked_result_sets_status_and_reason() {
        let kwargs = sample_kwargs();
        let result = blocked_result("plan_blocked", "not future safe", &kwargs);
        assert_eq!(result.status, "blocked");
        assert_eq!(result.diagnostics["reason"], "plan_blocked");
        assert_eq!(result.diagnostics["message"], "not future safe");
        assert_eq!(result.diagnostics["graph_published"], false);
    }

    #[test]
    fn failed_result_includes_error_details() {
        let kwargs = sample_kwargs();
        let error = std::io::Error::other("boom");
        let result = failed_result(
            "publish_failed",
            &error,
            &kwargs,
            Some(json!({"lease": "x"})),
            None,
            true,
            Some(json!({"complete": false})),
        );
        assert_eq!(result.status, "failed");
        assert_eq!(result.diagnostics["reason"], "publish_failed");
        assert!(result.diagnostics["error"]
            .as_str()
            .unwrap()
            .contains("boom"));
        assert_eq!(result.diagnostics["lease_or_lifecycle_written"], true);
    }

    #[test]
    fn published_result_marks_graph_published() {
        let kwargs = sample_kwargs();
        let graph = sample_graph();
        let result = published_result(
            &graph,
            &kwargs,
            Some(json!({"lease": "x"})),
            Some(json!({"lifecycle": "y"})),
        );
        assert_eq!(result.status, "published");
        assert_eq!(result.published_graph_version, Some("v2".to_string()));
        assert_eq!(result.diagnostics["graph_published"], true);
        assert_eq!(result.diagnostics["lease_or_lifecycle_written"], true);
    }
}
