//! Mirrors Python `lib/ccbd/reload_apply_stages.py`.
//! 1:1 file alignment stub.

use crate::reload_apply_results::{
    self, message_of, namespace_residue, not_published_diagnostics, reason_of, runtime_residue,
    status_of, GraphVersion, NamespacePatch, PublishTransaction, RuntimeMount,
};
use std::collections::HashMap;

/// Generate a failed namespace patch stage result
pub fn namespace_patch_failed(
    old_graph: &dyn GraphVersion,
    target_graph: &dyn GraphVersion,
    plan: &HashMap<String, serde_json::Value>,
    namespace_patch: &NamespacePatch,
) -> reload_apply_results::AdditiveReloadApplyResult {
    let status = if status_of(namespace_patch) == "blocked" {
        "blocked"
    } else {
        "failed"
    };

    let mut diagnostics = HashMap::new();
    diagnostics.insert(
        "reason".to_string(),
        reason_of(namespace_patch, "namespace_patch_failed".to_string()),
    );
    if let Some(msg) = message_of(namespace_patch) {
        diagnostics.insert("message".to_string(), msg);
    }

    for (key, value) in namespace_residue(namespace_patch) {
        diagnostics.insert(key, value);
    }

    for (key, value) in not_published_diagnostics() {
        diagnostics.insert(key, value);
    }

    reload_apply_results::stage_result(
        status,
        "namespace_patch",
        old_graph,
        target_graph,
        plan,
        Some(namespace_patch),
        None,
        None,
        diagnostics,
    )
}

/// Generate a failed runtime mount stage result
pub fn runtime_mount_failed(
    old_graph: &dyn GraphVersion,
    target_graph: &dyn GraphVersion,
    plan: &HashMap<String, serde_json::Value>,
    namespace_patch: &NamespacePatch,
    runtime_mount: &RuntimeMount,
) -> reload_apply_results::AdditiveReloadApplyResult {
    let status = if status_of(runtime_mount) == "blocked" {
        "blocked"
    } else {
        "failed"
    };

    let mut diagnostics = HashMap::new();
    diagnostics.insert(
        "reason".to_string(),
        reason_of(runtime_mount, "runtime_mount_failed".to_string()),
    );
    if let Some(msg) = message_of(runtime_mount) {
        diagnostics.insert("message".to_string(), msg);
    }

    for (key, value) in namespace_residue(namespace_patch) {
        diagnostics.insert(key, value);
    }

    for (key, value) in runtime_residue(runtime_mount) {
        diagnostics.insert(key, value);
    }

    for (key, value) in not_published_diagnostics() {
        diagnostics.insert(key, value);
    }

    reload_apply_results::stage_result(
        status,
        "runtime_mount",
        old_graph,
        target_graph,
        plan,
        Some(namespace_patch),
        Some(runtime_mount),
        None,
        diagnostics,
    )
}

/// Generate a failed publish stage result
pub fn publish_failed(
    old_graph: &dyn GraphVersion,
    target_graph: &dyn GraphVersion,
    plan: &HashMap<String, serde_json::Value>,
    namespace_patch: &NamespacePatch,
    runtime_mount: &RuntimeMount,
    transaction: &PublishTransaction,
) -> reload_apply_results::AdditiveReloadApplyResult {
    let status = if status_of(transaction) == "blocked" {
        "blocked"
    } else {
        "failed"
    };

    let mut diagnostics = HashMap::new();
    diagnostics.insert(
        "reason".to_string(),
        reason_of(transaction, "publish_transaction_failed".to_string()),
    );
    if let Some(msg) = message_of(transaction) {
        diagnostics.insert("message".to_string(), msg);
    }

    for (key, value) in namespace_residue(namespace_patch) {
        diagnostics.insert(key, value);
    }

    for (key, value) in runtime_residue(runtime_mount) {
        diagnostics.insert(key, value);
    }

    for (key, value) in not_published_diagnostics() {
        diagnostics.insert(key, value);
    }

    reload_apply_results::stage_result(
        status,
        "publish_transaction",
        old_graph,
        target_graph,
        plan,
        Some(namespace_patch),
        Some(runtime_mount),
        Some(transaction),
        diagnostics,
    )
}

/// Execute the publish stage
pub fn publish_stage(
    _app: &dyn CcbdApp,
    old_graph: &dyn GraphVersion,
    target_graph: &dyn GraphVersion,
    plan: &HashMap<String, serde_json::Value>,
    _namespace: &NamespaceContext,
    namespace_patch: &NamespacePatch,
    runtime_mount: &RuntimeMount,
    _publish_transaction_fn: &dyn PublishTransactionFn,
    _publish_graph_fn: &dyn PublishGraphFn,
    _update_lease_config_signature_fn: &dyn UpdateLeaseConfigSignatureFn,
    _update_lifecycle_config_signature_fn: &dyn UpdateLifecycleConfigSignatureFn,
) -> reload_apply_results::AdditiveReloadApplyResult {
    let mut diagnostics = HashMap::new();
    diagnostics.insert("reason".to_string(), "publish_success".to_string());

    reload_apply_results::stage_result(
        "published",
        "publish_transaction",
        old_graph,
        target_graph,
        plan,
        Some(namespace_patch),
        Some(runtime_mount),
        None,
        diagnostics,
    )
}

pub struct NamespaceContext;
pub trait CcbdApp {}
pub trait PublishTransactionFn {}
pub trait PublishGraphFn {}
pub trait UpdateLeaseConfigSignatureFn {}
pub trait UpdateLifecycleConfigSignatureFn {}
