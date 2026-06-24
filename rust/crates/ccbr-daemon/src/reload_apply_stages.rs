//! Mirrors Python `lib/ccbrd/reload_apply_stages.py`.
//! 1:1 file alignment stub.

use crate::reload_apply_models::AdditiveReloadApplyResult;
use crate::reload_apply_results::{
    self, message_of, namespace_residue, not_published_diagnostics, reason_of, runtime_residue,
    status_of, GraphVersion, NamespacePatch, PublishTransaction, RuntimeMount,
};
use std::collections::HashMap;

/// Custom publish-transaction implementation.
type PublishTransactionFn<'a> =
    &'a dyn Fn(
        &mut crate::app::CcbdApp,
        &crate::reload_apply_models::ServiceGraph,
        &crate::reload_transaction_context::TransactionContext,
        crate::reload_apply_namespace::NamespacePatchContext,
    ) -> crate::reload_transaction_models::ReloadPublishTransactionResult;

/// Custom graph-publishing implementation.
type PublishGraphFn<'a> =
    &'a dyn Fn(&mut crate::app::CcbdApp, &crate::reload_apply_models::ServiceGraph);

/// Lease config signature updater.
type UpdateLeaseConfigSignatureFn<'a> =
    &'a dyn Fn(&mut crate::app::CcbdApp, &str, u64) -> Option<serde_json::Value>;

/// Lifecycle config signature updater.
type UpdateLifecycleConfigSignatureFn<'a> =
    &'a dyn Fn(&mut crate::app::CcbdApp, &str, Option<u64>, u64) -> Option<serde_json::Value>;

/// Generate a failed namespace patch stage result
pub fn namespace_patch_failed(
    old_graph: &dyn GraphVersion,
    target_graph: &dyn GraphVersion,
    plan: &HashMap<String, serde_json::Value>,
    namespace_patch: &NamespacePatch,
) -> AdditiveReloadApplyResult {
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
) -> AdditiveReloadApplyResult {
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
) -> AdditiveReloadApplyResult {
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

/// Execute the publish stage.
///
/// Arity mirrors the Python `reload_apply_stages.publish_stage` helper.
#[allow(clippy::too_many_arguments)]
pub fn publish_stage(
    _app: &mut crate::app::CcbdApp,
    old_graph: &dyn GraphVersion,
    target_graph: &dyn GraphVersion,
    plan: &HashMap<String, serde_json::Value>,
    _namespace: &crate::services::project_namespace::ProjectNamespace,
    namespace_patch: &NamespacePatch,
    runtime_mount: &RuntimeMount,
    publish_transaction_fn: Option<PublishTransactionFn<'_>>,
    _publish_graph_fn: Option<PublishGraphFn<'_>>,
    _update_lease_config_signature_fn: Option<UpdateLeaseConfigSignatureFn<'_>>,
    _update_lifecycle_config_signature_fn: Option<UpdateLifecycleConfigSignatureFn<'_>>,
) -> AdditiveReloadApplyResult {
    let mut diagnostics = HashMap::new();
    diagnostics.insert("reason".to_string(), "publish_success".to_string());

    if publish_transaction_fn.is_some() {
        diagnostics.insert("transaction_injected".to_string(), "true".to_string());
    }

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
