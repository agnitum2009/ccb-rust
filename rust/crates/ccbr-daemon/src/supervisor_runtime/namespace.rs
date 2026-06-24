//! Mirrors Python `lib/ccbrd/supervisor_runtime/namespace.py`.
//! 1:1 file alignment stub.

use std::collections::HashMap;

/// Ensure project namespace with optional reflow and recreation
pub fn ensure_project_namespace(
    project_namespace: &dyn ProjectNamespace,
    layout_signature: Option<&str>,
    topology_plan: Option<&TopologyPlan>,
    recreate_namespace: bool,
    reflow_workspace: bool,
    recreate_reason: Option<&str>,
    background_maintenance: bool,
    terminal_size: Option<(u32, u32)>,
) -> Result<NamespaceResult, String> {
    if reflow_workspace && topology_plan.is_none() {
        return reflow_project_workspace(project_namespace, layout_signature, recreate_reason, background_maintenance);
    }

    let recreate_namespace = if reflow_workspace && topology_plan.is_some() { true } else { recreate_namespace };

    if !namespace_kwargs_requested(layout_signature, topology_plan, recreate_namespace, recreate_reason, background_maintenance, terminal_size) {
        return project_namespace.ensure();
    }

    let kwargs = build_ensure_kwargs(layout_signature, topology_plan, recreate_namespace, recreate_reason, background_maintenance, terminal_size);
    project_namespace.ensure_with_kwargs(&kwargs)
}

/// Check if namespace kwargs are requested
fn namespace_kwargs_requested(
    layout_signature: Option<&str>,
    topology_plan: Option<&TopologyPlan>,
    recreate_namespace: bool,
    recreate_reason: Option<&str>,
    background_maintenance: bool,
    terminal_size: Option<(u32, u32)>,
) -> bool {
    recreate_namespace
        || recreate_reason.map_or(false, |r| !r.trim().is_empty())
        || topology_plan.is_some()
        || layout_signature.map_or(false, |s| !s.trim().is_empty())
        || background_maintenance
        || terminal_size.is_some()
}

/// Reflow project workspace
fn reflow_project_workspace(
    project_namespace: &dyn ProjectNamespace,
    layout_signature: Option<&str>,
    recreate_reason: Option<&str>,
    background_maintenance: bool,
) -> Result<NamespaceResult, String> {
    if let Some(reflow_fn) = project_namespace.reflow_workspace() {
        let mut kwargs = HashMap::new();
        if let Some(signature) = layout_signature {
            kwargs.insert("layout_signature".to_string(), signature.to_string());
        }
        if let Some(reason) = recreate_reason {
            kwargs.insert("reason".to_string(), reason.to_string());
        }
        if background_maintenance {
            kwargs.insert("session_probe_timeout_s".to_string(), "0.0".to_string());
        }
        return reflow_fn.call(&kwargs);
    }
    project_namespace.ensure()
}

/// Build ensure kwargs
fn build_ensure_kwargs(
    layout_signature: Option<&str>,
    topology_plan: Option<&TopologyPlan>,
    recreate_namespace: bool,
    recreate_reason: Option<&str>,
    background_maintenance: bool,
    terminal_size: Option<(u32, u32)>,
) -> HashMap<String, String> {
    let mut kwargs = HashMap::new();
    if let Some(signature) = layout_signature {
        kwargs.insert("layout_signature".to_string(), signature.to_string());
    }
    if let Some(reason) = recreate_reason {
        kwargs.insert("recreate_reason".to_string(), reason.to_string());
    }
    kwargs.insert("force_recreate".to_string(), recreate_namespace.to_string());
    if background_maintenance {
        kwargs.insert("session_probe_timeout_s".to_string(), "0.0".to_string());
    }
    if let Some((width, height)) = terminal_size {
        kwargs.insert("terminal_size".to_string(), format!("({}, {})", width, height));
    }
    kwargs
}

pub trait ProjectNamespace {
    fn ensure(&self) -> Result<NamespaceResult, String>;
    fn ensure_with_kwargs(&self, kwargs: &HashMap<String, String>) -> Result<NamespaceResult, String>;
    fn reflow_workspace(&self) -> Option<&dyn ReflowWorkspaceFn>;
}

pub trait ReflowWorkspaceFn {
    fn call(&self, kwargs: &HashMap<String, String>) -> Result<NamespaceResult, String>;
}

pub struct TopologyPlan;
pub struct NamespaceResult { pub success: bool }
