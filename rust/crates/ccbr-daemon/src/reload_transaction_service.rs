//! Mirrors Python `lib/ccbd/reload_transaction_service.py`.

use crate::app::CcbdApp;
use crate::reload_apply_models::ServiceGraph;
use crate::reload_transaction_context::{transaction_context, TransactionContext};
use crate::reload_transaction_models::ReloadPublishTransactionResult;
use crate::reload_transaction_preflight::initial_failure;
use crate::reload_transaction_publish::publish_or_rollback;
use crate::reload_transaction_records::record;
use crate::reload_transaction_results::failed_result;
use crate::reload_transaction_signature::{
    expected_generation, update_current_lease_config_signature,
    update_mounted_lifecycle_config_signature,
};
use crate::reload_transaction_signature_rollback::rollback_signatures;
use serde_json::Value;

/// Lease config signature updater.
type UpdateLeaseConfigSignatureFn<'a> = &'a dyn Fn(&mut CcbdApp, &str, u64) -> Option<Value>;

/// Lifecycle config signature updater.
type UpdateLifecycleConfigSignatureFn<'a> =
    &'a dyn Fn(&mut CcbdApp, &str, Option<u64>, u64) -> Option<Value>;

/// Custom graph-publishing implementation.
type PublishGraphFn<'a> = &'a dyn Fn(&mut CcbdApp, &ServiceGraph);

/// Publish an additive reload transaction: write signatures, publish graph, rollback on failure.
///
/// Arity mirrors the Python `reload_transaction_service.publish_additive_reload_transaction`
/// entrypoint.
#[allow(clippy::too_many_arguments)]
pub fn publish_additive_reload_transaction(
    app: &mut CcbdApp,
    new_graph: &ServiceGraph,
    namespace: Option<&crate::services::project_namespace::ProjectNamespace>,
    namespace_patch_result: Option<Value>,
    runtime_mount_result: Option<Value>,
    update_lease_config_signature_fn: Option<UpdateLeaseConfigSignatureFn<'_>>,
    update_lifecycle_config_signature_fn: Option<UpdateLifecycleConfigSignatureFn<'_>>,
    publish_graph_fn: Option<PublishGraphFn<'_>>,
) -> ReloadPublishTransactionResult {
    let old_graph = app.current_service_graph();
    let context = transaction_context(
        &old_graph,
        new_graph,
        namespace_patch_result,
        runtime_mount_result,
    );

    if let Some(failure) = initial_failure(
        app,
        &context,
        context.namespace_patch.clone(),
        context.runtime_mount.clone(),
    ) {
        return failure;
    }

    let generation = match expected_generation(app) {
        Some(g) => g,
        None => {
            return blocked_transaction_result(
                "missing_generation",
                "could not determine expected daemon generation",
                &context,
            );
        }
    };

    let namespace_epoch = namespace.map(|ns| ns.namespace_epoch);
    let signatures = write_signatures(
        app,
        &context,
        namespace_epoch,
        generation,
        update_lease_config_signature_fn,
        update_lifecycle_config_signature_fn,
    );

    if let Some(failure) = signatures.failure {
        return failure;
    }

    publish_or_rollback(
        app,
        new_graph,
        &context,
        namespace_epoch,
        generation,
        publish_graph_fn,
        signatures.lease,
        signatures.lifecycle,
    )
}

#[derive(Debug, Clone, Default)]
struct SignatureWriteResult {
    lease: Option<Value>,
    lifecycle: Option<Value>,
    failure: Option<ReloadPublishTransactionResult>,
}

/// Arity mirrors the Python `reload_transaction_service` signature helper.
#[allow(clippy::too_many_arguments)]
fn write_signatures(
    app: &mut CcbdApp,
    context: &TransactionContext,
    namespace_epoch: Option<u64>,
    expected_generation: u64,
    update_lease_config_signature_fn: Option<UpdateLeaseConfigSignatureFn<'_>>,
    update_lifecycle_config_signature_fn: Option<UpdateLifecycleConfigSignatureFn<'_>>,
) -> SignatureWriteResult {
    let mut lease: Option<Value> = None;
    let mut lifecycle: Option<Value> = None;

    let lease_result = if let Some(update_fn) = update_lease_config_signature_fn {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            update_fn(app, &context.new_config_signature, expected_generation)
        }))
        .map_err(|e| {
            if let Some(s) = e.downcast_ref::<&str>() {
                anyhow::anyhow!("lease signature panic: {}", s)
            } else {
                anyhow::anyhow!("lease signature panic")
            }
        })
    } else {
        Ok(update_current_lease_config_signature(
            app,
            &context.new_config_signature,
            expected_generation,
        ))
    };

    match lease_result {
        Ok(Some(sig)) => lease = Some(sig),
        Ok(None) => {}
        Err(err) => {
            return signature_write_failed(
                app,
                context,
                err,
                namespace_epoch,
                expected_generation,
                false,
                false,
            );
        }
    }

    let lifecycle_result = update_lifecycle_signature(
        app,
        context,
        namespace_epoch,
        expected_generation,
        update_lifecycle_config_signature_fn,
    );

    match lifecycle_result {
        Ok(Some(sig)) => lifecycle = Some(sig),
        Ok(None) => {}
        Err(err) => {
            return signature_write_failed(
                app,
                context,
                err,
                namespace_epoch,
                expected_generation,
                lease.is_some(),
                false,
            );
        }
    }

    app.set_lease(record(lease.clone()));
    SignatureWriteResult {
        lease,
        lifecycle,
        failure: None,
    }
}

fn update_lifecycle_signature(
    app: &mut CcbdApp,
    context: &TransactionContext,
    namespace_epoch: Option<u64>,
    expected_generation: u64,
    update_lifecycle_config_signature_fn: Option<UpdateLifecycleConfigSignatureFn<'_>>,
) -> Result<Option<Value>, anyhow::Error> {
    if let Some(update_fn) = update_lifecycle_config_signature_fn {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            update_fn(
                app,
                &context.new_config_signature,
                namespace_epoch,
                expected_generation,
            )
        }))
        .map_err(|e| {
            if let Some(s) = e.downcast_ref::<&str>() {
                anyhow::anyhow!("lifecycle signature panic: {}", s)
            } else {
                anyhow::anyhow!("lifecycle signature panic")
            }
        })
    } else {
        Ok(update_mounted_lifecycle_config_signature(
            app,
            &context.new_config_signature,
            namespace_epoch,
            expected_generation,
        ))
    }
}

fn signature_write_failed(
    app: &mut CcbdApp,
    context: &TransactionContext,
    error: anyhow::Error,
    namespace_epoch: Option<u64>,
    expected_generation: u64,
    rollback_lease: bool,
    rollback_lifecycle: bool,
) -> SignatureWriteResult {
    let rollback = rollback_signatures(
        app,
        &context.old_config_signature,
        namespace_epoch,
        expected_generation,
        rollback_lease,
        rollback_lifecycle,
    );
    let complete = rollback
        .get("complete")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let failure = failed_result(
        "signature_handoff_failed",
        error.as_ref(),
        &context.result_kwargs(),
        record(app.mount_manager_state()),
        record(app.lifecycle_store_state()),
        !complete,
        Some(rollback),
    );
    SignatureWriteResult {
        lease: None,
        lifecycle: None,
        failure: Some(failure),
    }
}

fn blocked_transaction_result(
    reason: &str,
    message: &str,
    context: &TransactionContext,
) -> ReloadPublishTransactionResult {
    crate::reload_transaction_results::blocked_result(reason, message, &context.result_kwargs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reload_apply_models::ServiceGraph;
    use crate::start_flow::service::StartFlowService;
    use crate::stop_flow::service::StopFlowService;
    use serde_json::json;
    use tempfile::TempDir;

    fn stub_app(dir: &TempDir) -> CcbdApp {
        CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        )
    }

    fn sample_graph() -> ServiceGraph {
        ServiceGraph {
            version: Some("v2".to_string()),
            config: ccbr_agents::models::ProjectConfig::default(),
            config_identity: json!({}),
            config_signature: "new-sig".to_string(),
        }
    }

    #[test]
    fn publish_transaction_blocks_when_preflight_fails() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        let new_graph = sample_graph();
        let result = publish_additive_reload_transaction(
            &mut app,
            &new_graph,
            None,
            Some(json!({"status": "blocked"})),
            None,
            None,
            None,
            None,
        );
        assert_eq!(result.status, "blocked");
    }

    #[test]
    fn publish_transaction_fails_when_generation_missing() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        let new_graph = sample_graph();
        let result = publish_additive_reload_transaction(
            &mut app,
            &new_graph,
            None,
            Some(json!({"status": "applied"})),
            None,
            None,
            None,
            None,
        );
        assert_eq!(result.status, "blocked");
        assert_eq!(result.diagnostics["reason"], "missing_generation");
    }
}
