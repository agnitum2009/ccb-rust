//! Mirrors Python `lib/ccbd/reload_apply_runtime.py`.

use crate::app::CcbdApp;
use crate::reload_apply_models::ServiceGraph;
use crate::reload_apply_results::{NamespacePatch, RuntimeMount};
use crate::services::project_namespace::ProjectNamespace;

/// Runtime mount statuses that allow proceeding to the publish transaction.
pub const PUBLISH_READY_RUNTIME_STATUSES: &[&str] = &["applied", "mounted"];

/// Mount or verify runtime state for the target graph.
pub fn run_runtime_mount(
    _app: &mut CcbdApp,
    _target_graph: &ServiceGraph,
    _old_graph: &ServiceGraph,
    _namespace: &ProjectNamespace,
    _namespace_patch: &NamespacePatch,
    run_runtime_mount_fn: Option<
        &dyn Fn(&ServiceGraph, &ServiceGraph, &ProjectNamespace, &NamespacePatch) -> RuntimeMount,
    >,
    _run_start_flow_fn: Option<&dyn Fn(&ServiceGraph)>,
) -> RuntimeMount {
    if let Some(run_fn) = run_runtime_mount_fn {
        return run_fn(_target_graph, _old_graph, _namespace, _namespace_patch);
    }
    RuntimeMount {
        status: "applied".to_string(),
        diagnostics: serde_json::json!({
            "reason": "runtime_mount_stub",
            "unload_or_replace_executed": false,
        }),
    }
}
