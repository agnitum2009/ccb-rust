//! Mirrors Python `lib/ccbd/reload_transaction_publish.py`.

use crate::app::CcbdApp;
use crate::reload_apply_models::ServiceGraph;
use crate::reload_transaction_context::TransactionContext;
use crate::reload_transaction_models::ReloadPublishTransactionResult;
use crate::reload_transaction_records::record;
use crate::reload_transaction_results::failed_result;
use crate::reload_transaction_signature_rollback::rollback_signatures;
use serde_json::Value;

/// Publish the new graph or roll back signatures on failure.
pub fn publish_or_rollback(
    app: &mut CcbdApp,
    new_graph: &ServiceGraph,
    context: &TransactionContext,
    namespace_epoch: Option<u64>,
    expected_generation: u64,
    publish_graph_fn: Option<&dyn Fn(&mut CcbdApp, &ServiceGraph)>,
    lease: Option<Value>,
    lifecycle: Option<Value>,
) -> ReloadPublishTransactionResult {
    let publish_result = if let Some(publish_fn) = publish_graph_fn {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| publish_fn(app, new_graph)))
            .map_err(|e| {
                if let Some(s) = e.downcast_ref::<&str>() {
                    anyhow::anyhow!("publish panic: {}", s)
                } else {
                    anyhow::anyhow!("publish panic")
                }
            })
    } else {
        app.publish_service_graph(new_graph);
        Ok(())
    };

    if let Err(err) = publish_result {
        return publish_failed(app, context, err, namespace_epoch, expected_generation);
    }

    crate::reload_transaction_results::published_result(
        new_graph,
        &context.result_kwargs(),
        record(lease),
        record(lifecycle),
    )
}

fn publish_failed(
    app: &mut CcbdApp,
    context: &TransactionContext,
    error: anyhow::Error,
    namespace_epoch: Option<u64>,
    expected_generation: u64,
) -> ReloadPublishTransactionResult {
    let rollback = rollback_signatures(
        app,
        &context.old_config_signature,
        namespace_epoch,
        expected_generation,
        true,
        true,
    );
    let complete = rollback
        .get("complete")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if let Some(lease) = rollback.get("lease") {
        app.set_lease(record(Some(lease)));
    }
    failed_result(
        "service_graph_publish_failed",
        error.as_ref(),
        &context.result_kwargs(),
        record(app.mount_manager_state()),
        record(app.lifecycle_store_state()),
        !complete,
        Some(rollback),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reload_apply_models::ServiceGraph;
    use crate::reload_transaction_context::transaction_context;
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
            config: ccb_agents::models::ProjectConfig::default(),
            config_identity: json!({}),
            config_signature: "new-sig".to_string(),
        }
    }

    #[test]
    fn publish_or_rollback_publishes_graph() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        let new_graph = sample_graph();
        let context = transaction_context(&app.current_service_graph(), &new_graph, None, None);
        let result = publish_or_rollback(
            &mut app,
            &new_graph,
            &context,
            None,
            1,
            None,
            Some(json!({"lease": "x"})),
            Some(json!({"lifecycle": "y"})),
        );
        assert_eq!(result.status, "published");
        assert_eq!(result.diagnostics["graph_published"], true);
    }

    #[test]
    fn publish_or_rollback_rolls_back_on_failure() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        let new_graph = sample_graph();
        let context = transaction_context(&app.current_service_graph(), &new_graph, None, None);
        let result = publish_or_rollback(
            &mut app,
            &new_graph,
            &context,
            None,
            1,
            Some(&|_app, _graph| panic!("publish exploded")),
            Some(json!({"lease": "x"})),
            Some(json!({"lifecycle": "y"})),
        );
        assert_eq!(result.status, "failed");
        assert_eq!(result.diagnostics["reason"], "service_graph_publish_failed");
    }
}
