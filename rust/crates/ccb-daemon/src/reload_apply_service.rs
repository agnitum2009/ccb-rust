//! Mirrors Python `lib/ccbd/reload_apply_service.py`.

use crate::app::CcbdApp;
use crate::reload_apply_graph::build_reload_service_graph;
use crate::reload_apply_models::{AdditiveReloadApplyResult, ServiceGraph};
use crate::reload_apply_namespace::{
    apply_namespace_patch, current_namespace, topology_for, NamespacePatchContext,
};
use crate::reload_apply_plan::{plan_blocked_result, plan_blocker};
use crate::reload_apply_results::{noop_result, status_of};
use crate::reload_apply_runtime::{run_runtime_mount, PUBLISH_READY_RUNTIME_STATUSES};
use crate::reload_apply_stages::{namespace_patch_failed, publish_stage, runtime_mount_failed};
use crate::reload_handoff::{begin_reload_handoff, clear_reload_handoff};
use crate::reload_plan::build_reload_dry_run_plan;
use crate::reload_plan::project_config_identity_payload;
use crate::reload_runtime_unload::pre_namespace_unload_blocker;
use crate::services::project_namespace::ProjectNamespace;
use crate::services::project_namespace_runtime::models::NamespacePatchApplyResult;
use crate::services::project_namespace_runtime::topology_plan::NamespaceTopologyPlan;
use ccb_agents::models::ProjectConfig;
use serde_json::Value;
use std::collections::HashMap;

/// Custom namespace patch implementation.
type ApplyNamespacePatchFn<'a> = &'a dyn Fn(
    &HashMap<String, Value>,
    &NamespaceTopologyPlan,
    &NamespaceTopologyPlan,
) -> NamespacePatchApplyResult;

/// Custom runtime mount implementation.
type RunRuntimeMountFn<'a> = &'a dyn Fn(
    &ServiceGraph,
    &ServiceGraph,
    &ProjectNamespace,
    &crate::reload_apply_results::NamespacePatch,
) -> crate::reload_apply_results::RuntimeMount;

/// Optional post-mount start flow trigger.
type RunStartFlowFn<'a> = &'a dyn Fn(&ServiceGraph);

/// Custom publish-transaction implementation.
type PublishTransactionFn<'a> =
    &'a dyn Fn(
        &mut CcbdApp,
        &ServiceGraph,
        &crate::reload_transaction_context::TransactionContext,
        NamespacePatchContext,
    ) -> crate::reload_transaction_models::ReloadPublishTransactionResult;

/// Custom graph-publishing implementation.
type PublishGraphFn<'a> = &'a dyn Fn(&mut CcbdApp, &ServiceGraph);

/// Lease config signature updater.
type UpdateLeaseConfigSignatureFn<'a> = &'a dyn Fn(&mut CcbdApp, &str, u64) -> Option<Value>;

/// Lifecycle config signature updater.
type UpdateLifecycleConfigSignatureFn<'a> =
    &'a dyn Fn(&mut CcbdApp, &str, Option<u64>, u64) -> Option<Value>;

/// Run an additive reload apply under the optional maintenance lock.
///
/// Arity mirrors the Python `reload_apply_service.run_additive_reload_apply` entrypoint.
#[allow(clippy::too_many_arguments)]
pub fn run_additive_reload_apply(
    app: &mut CcbdApp,
    new_config: &ProjectConfig,
    provided_namespace: Option<&ProjectNamespace>,
    apply_namespace_patch_fn: Option<ApplyNamespacePatchFn<'_>>,
    run_runtime_mount_fn: Option<RunRuntimeMountFn<'_>>,
    run_start_flow_fn: Option<RunStartFlowFn<'_>>,
    publish_transaction_fn: Option<PublishTransactionFn<'_>>,
    publish_graph_fn: Option<PublishGraphFn<'_>>,
    update_lease_config_signature_fn: Option<UpdateLeaseConfigSignatureFn<'_>>,
    update_lifecycle_config_signature_fn: Option<UpdateLifecycleConfigSignatureFn<'_>>,
) -> AdditiveReloadApplyResult {
    if let Some(_lock) = app.start_maintenance_lock().as_ref() {
        return run_locked(
            app,
            new_config,
            provided_namespace,
            apply_namespace_patch_fn,
            run_runtime_mount_fn,
            run_start_flow_fn,
            publish_transaction_fn,
            publish_graph_fn,
            update_lease_config_signature_fn,
            update_lifecycle_config_signature_fn,
        );
    }
    run_locked(
        app,
        new_config,
        provided_namespace,
        apply_namespace_patch_fn,
        run_runtime_mount_fn,
        run_start_flow_fn,
        publish_transaction_fn,
        publish_graph_fn,
        update_lease_config_signature_fn,
        update_lifecycle_config_signature_fn,
    )
}

/// Internal locked variant; arity kept to match the Python source.
#[allow(clippy::too_many_arguments)]
fn run_locked(
    app: &mut CcbdApp,
    new_config: &ProjectConfig,
    provided_namespace: Option<&ProjectNamespace>,
    apply_namespace_patch_fn: Option<ApplyNamespacePatchFn<'_>>,
    run_runtime_mount_fn: Option<RunRuntimeMountFn<'_>>,
    run_start_flow_fn: Option<RunStartFlowFn<'_>>,
    publish_transaction_fn: Option<PublishTransactionFn<'_>>,
    publish_graph_fn: Option<PublishGraphFn<'_>>,
    update_lease_config_signature_fn: Option<UpdateLeaseConfigSignatureFn<'_>>,
    update_lifecycle_config_signature_fn: Option<UpdateLifecycleConfigSignatureFn<'_>>,
) -> AdditiveReloadApplyResult {
    let old_graph = app.current_service_graph();
    let (namespace, namespace_diagnostics) = current_namespace_for_apply(app, provided_namespace);
    let plan = dry_run_plan(app, &old_graph, new_config, namespace.as_ref());

    let plan_json = plan_to_json(&plan);

    if let Some(blocker) = plan_blocker(&plan_json) {
        return plan_blocked_result(
            &old_graph_value(&old_graph),
            &plan_json,
            &blocker,
            &namespace_diagnostics.into_iter().collect(),
        );
    }

    if let Some(blocker) = pre_namespace_unload_blocker(app, &old_graph, &plan) {
        return plan_blocked_result(
            &old_graph_value(&old_graph),
            &plan_json,
            &blocker,
            &namespace_diagnostics.into_iter().collect(),
        );
    }

    if plan.plan_class == "no_change" {
        return noop_result(&old_graph, &plan_json);
    }

    let target_identity = project_config_identity_payload(new_config);
    let handoff = begin_reload_handoff(app, &target_identity);

    let result = run_apply_stages(
        app,
        &old_graph,
        new_config,
        &plan,
        &plan_json,
        namespace.as_ref(),
        apply_namespace_patch_fn,
        run_runtime_mount_fn,
        run_start_flow_fn,
        publish_transaction_fn,
        publish_graph_fn,
        update_lease_config_signature_fn,
        update_lifecycle_config_signature_fn,
    );

    if handoff.is_some() {
        clear_reload_handoff(app);
    }

    result
}

/// Apply namespace/runtime/publish stages; arity kept to match the Python source.
#[allow(clippy::too_many_arguments)]
fn run_apply_stages(
    app: &mut CcbdApp,
    old_graph: &ServiceGraph,
    new_config: &ProjectConfig,
    _plan: &crate::reload_plan::ReloadPlan,
    plan_json: &HashMap<String, Value>,
    namespace: Option<&ProjectNamespace>,
    apply_namespace_patch_fn: Option<ApplyNamespacePatchFn<'_>>,
    run_runtime_mount_fn: Option<RunRuntimeMountFn<'_>>,
    run_start_flow_fn: Option<RunStartFlowFn<'_>>,
    publish_transaction_fn: Option<PublishTransactionFn<'_>>,
    publish_graph_fn: Option<PublishGraphFn<'_>>,
    update_lease_config_signature_fn: Option<UpdateLeaseConfigSignatureFn<'_>>,
    update_lifecycle_config_signature_fn: Option<UpdateLifecycleConfigSignatureFn<'_>>,
) -> AdditiveReloadApplyResult {
    let target_graph = build_reload_service_graph(app, new_config);

    let namespace_patch = namespace_patch_stage(
        app,
        &old_graph.config,
        new_config,
        plan_json,
        apply_namespace_patch_fn,
    );
    if status_of(&namespace_patch_wrapper(&namespace_patch)) != "applied" {
        return namespace_patch_failed(
            old_graph,
            &target_graph,
            plan_json,
            &namespace_patch_wrapper(&namespace_patch),
        );
    }

    let ns = match namespace {
        Some(ns) => ns.clone(),
        None => {
            return namespace_patch_failed(
                old_graph,
                &target_graph,
                plan_json,
                &namespace_patch_wrapper(&NamespacePatchApplyResult {
                    status: "failed".to_string(),
                    diagnostics: serde_json::json!({
                        "reason": "namespace_missing",
                        "message": "no namespace available for runtime mount",
                    }),
                }),
            )
        }
    };

    let runtime_mount = run_runtime_mount(
        app,
        &target_graph,
        old_graph,
        &ns,
        &namespace_patch_wrapper(&namespace_patch),
        run_runtime_mount_fn,
        run_start_flow_fn,
    );

    if !PUBLISH_READY_RUNTIME_STATUSES
        .contains(&status_of(&runtime_mount_wrapper(&runtime_mount)).as_str())
    {
        return runtime_mount_failed(
            old_graph,
            &target_graph,
            plan_json,
            &namespace_patch_wrapper(&namespace_patch),
            &runtime_mount_wrapper(&runtime_mount),
        );
    }

    publish_stage(
        app,
        old_graph,
        &target_graph,
        plan_json,
        &ns,
        &namespace_patch_wrapper(&namespace_patch),
        &runtime_mount_wrapper(&runtime_mount),
        publish_transaction_fn,
        publish_graph_fn,
        update_lease_config_signature_fn,
        update_lifecycle_config_signature_fn,
    )
}

fn namespace_patch_stage(
    app: &mut CcbdApp,
    old_config: &ProjectConfig,
    new_config: &ProjectConfig,
    plan_json: &HashMap<String, Value>,
    apply_namespace_patch_fn: Option<ApplyNamespacePatchFn<'_>>,
) -> NamespacePatchApplyResult {
    apply_namespace_patch(
        app,
        plan_json,
        &topology_for(app, old_config),
        &topology_for(app, new_config),
        apply_namespace_patch_fn,
    )
}

fn namespace_patch_wrapper(
    patch: &NamespacePatchApplyResult,
) -> crate::reload_apply_results::NamespacePatch {
    crate::reload_apply_results::NamespacePatch {
        status: patch.status.clone(),
        diagnostics: patch.diagnostics.clone(),
    }
}

fn runtime_mount_wrapper(
    mount: &crate::reload_apply_results::RuntimeMount,
) -> crate::reload_apply_results::RuntimeMount {
    mount.clone()
}

/// Resolve the namespace to use for apply.
pub fn current_namespace_for_apply(
    app: &CcbdApp,
    provided_namespace: Option<&ProjectNamespace>,
) -> (Option<ProjectNamespace>, HashMap<String, Value>) {
    current_namespace(app, provided_namespace)
}

fn dry_run_plan(
    app: &CcbdApp,
    old_graph: &ServiceGraph,
    new_config: &ProjectConfig,
    namespace: Option<&ProjectNamespace>,
) -> crate::reload_plan::ReloadPlan {
    build_reload_dry_run_plan(
        &old_graph.config,
        new_config,
        Some(&old_graph.config_identity),
        app.project_id().into(),
        namespace,
    )
}

fn plan_to_json(plan: &crate::reload_plan::ReloadPlan) -> HashMap<String, Value> {
    match serde_json::to_value(plan) {
        Ok(Value::Object(map)) => map.into_iter().collect(),
        _ => HashMap::new(),
    }
}

fn old_graph_value(graph: &ServiceGraph) -> Option<HashMap<String, Value>> {
    let mut map = HashMap::new();
    map.insert("version".to_string(), serde_json::json!(graph.version));
    map.insert(
        "config_signature".to_string(),
        serde_json::json!(graph.config_signature),
    );
    Some(map)
}

// CcbdApp extension methods used by the reload hot-swap path.
impl CcbdApp {
    pub(crate) fn current_service_graph(&self) -> ServiceGraph {
        let config =
            self.current_config
                .clone()
                .unwrap_or_else(|| ccb_agents::models::ProjectConfig {
                    version: 2,
                    default_agents: Vec::new(),
                    agents: std::collections::HashMap::new(),
                    windows: None,
                    tool_windows: None,
                    entry_window: None,
                    sidebar: None,
                    ..Default::default()
                });
        let identity = project_config_identity_payload(&config);
        let signature = identity
            .get("config_signature")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default();
        ServiceGraph {
            version: Some(self.project_id().to_string()),
            config,
            config_identity: identity,
            config_signature: signature,
        }
    }

    pub(crate) fn publish_service_graph(&mut self, graph: &ServiceGraph) {
        self.current_config = Some(graph.config.clone());
    }

    pub(crate) fn start_maintenance_lock(&self) -> Option<&std::sync::Mutex<()>> {
        None
    }

    pub(crate) fn set_lease(&mut self, _lease: Option<Value>) {}

    pub(crate) fn mount_manager_state(&self) -> Option<Value> {
        self.project_namespace.load().map(|ns| ns.to_record())
    }

    pub(crate) fn lifecycle_store_state(&self) -> Option<Value> {
        self.lifecycle
            .recent_reports(1)
            .last()
            .map(|r| serde_json::to_value(r).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::start_flow::service::StartFlowService;
    use crate::stop_flow::service::StopFlowService;
    use tempfile::TempDir;

    fn stub_app(dir: &TempDir) -> CcbdApp {
        CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        )
    }

    #[test]
    fn current_service_graph_has_signature() {
        let dir = TempDir::new().unwrap();
        let app = stub_app(&dir);
        let graph = app.current_service_graph();
        assert!(!graph.config_signature.is_empty());
    }

    #[test]
    fn run_additive_reload_apply_no_change_is_noop() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        let graph = app.current_service_graph();
        let config = graph.config.clone();
        let result = run_additive_reload_apply(
            &mut app, &config, None, None, None, None, None, None, None, None,
        );
        assert_eq!(result.status, "noop");
        assert_eq!(result.stage, "no_op");
    }
}
