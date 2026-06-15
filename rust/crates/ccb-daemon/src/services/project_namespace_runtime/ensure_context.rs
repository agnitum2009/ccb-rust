//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/ensure_context.py`.
//! 1:1 file alignment stub.

use crate::services::project_namespace_runtime::backend::{
    build_backend, prepare_server, session_alive, Backend, BackendFactory,
};
use crate::services::project_namespace_runtime::records::normalized_layout_signature;
use crate::Result;

/// Context for namespace ensure operations
#[derive(Debug, Clone)]
pub struct NamespaceEnsureContext {
    pub current: Option<NamespaceState>,
    pub backend: Backend,
    pub session_is_alive: bool,
    pub desired_socket_path: String,
    pub desired_session_name: String,
    pub desired_layout_signature: Option<String>,
    pub desired_control_window_name: String,
    pub desired_workspace_window_name: String,
    pub topology_plan: Option<TopologyPlan>,
    pub recreate_cause: Option<String>,
}

impl NamespaceEnsureContext {
    /// Create updated context with selective field changes
    pub fn with_updates(
        &self,
        backend: Option<Backend>,
        session_is_alive: Option<bool>,
        recreate_cause: Option<String>,
    ) -> Self {
        Self {
            backend: backend.unwrap_or_else(|| self.backend.clone()),
            session_is_alive: session_is_alive.unwrap_or(self.session_is_alive),
            recreate_cause: recreate_cause.or_else(|| self.recreate_cause.clone()),
            ..self.clone()
        }
    }
}

/// Get desired namespace state parameters
pub fn desired_namespace_state(
    controller: &NamespaceController,
    layout_signature: Option<&str>,
    topology_plan: Option<&TopologyPlan>,
) -> (String, String, Option<String>, String, String) {
    let desired_socket_path = controller.layout.ccbd_tmux_socket_path.clone();
    let desired_session_name = controller.layout.ccbd_tmux_session_name.clone();

    let layout_sig = topology_plan
        .and_then(|p| p.signature.as_ref().map(|s| s.as_str()))
        .unwrap_or(layout_signature.unwrap_or(""));

    let desired_layout_signature = normalized_layout_signature(Some(layout_sig));

    let desired_control_window_name = controller.layout.ccbd_tmux_control_window_name.clone();

    let desired_workspace_window_name = topology_plan
        .and_then(|p| {
            let entry = p.entry_window.as_ref()?;
            if entry.trim().is_empty() {
                None
            } else {
                Some(entry.clone())
            }
        })
        .unwrap_or_else(|| controller.layout.ccbd_tmux_workspace_window_name.clone());

    (
        desired_socket_path,
        desired_session_name,
        desired_layout_signature,
        desired_control_window_name,
        desired_workspace_window_name,
    )
}

/// Load namespace context from controller state
pub fn load_namespace_context(
    controller: &NamespaceController,
    layout_signature: Option<&str>,
    topology_plan: Option<&TopologyPlan>,
    recreate_reason: Option<&str>,
) -> Result<NamespaceEnsureContext> {
    let (
        desired_socket_path,
        desired_session_name,
        desired_layout_signature,
        desired_control_window_name,
        desired_workspace_window_name,
    ) = desired_namespace_state(controller, layout_signature, topology_plan);

    let current = controller.state_store.load()?;
    let backend = build_backend(&controller.backend_factory, &desired_socket_path)?;

    Ok(NamespaceEnsureContext {
        current,
        backend,
        session_is_alive: false,
        desired_socket_path,
        desired_session_name,
        desired_layout_signature,
        desired_control_window_name,
        desired_workspace_window_name,
        topology_plan: topology_plan.cloned(),
        recreate_cause: recreate_reason
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
    })
}

/// Refresh session liveness status
pub fn refresh_session_liveness(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
    timeout_s: Option<f64>,
) -> Result<NamespaceEnsureContext> {
    if context.current.is_none() {
        return Ok(context.with_updates(None, Some(false), None));
    }

    prepare_server(&context.backend, timeout_s)?;
    let is_alive = session_alive(&context.backend, &context.desired_session_name, timeout_s)?;

    Ok(context.with_updates(None, Some(is_alive), None))
}

/// Rebuild namespace backend with new socket path
pub fn rebuild_namespace_backend(
    controller: &NamespaceController,
    socket_path: &str,
) -> Result<Backend> {
    build_backend(&controller.backend_factory, socket_path)
}

// Type definitions

#[derive(Debug, Clone)]
pub struct NamespaceController {
    pub layout: LayoutConfig,
    pub backend_factory: BackendFactory,
    pub state_store: StateStore,
}

#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub ccbd_tmux_socket_path: String,
    pub ccbd_tmux_session_name: String,
    pub ccbd_tmux_control_window_name: String,
    pub ccbd_tmux_workspace_window_name: String,
}

#[derive(Debug, Clone)]
pub struct StateStore {
    pub namespace: Option<NamespaceState>,
}

impl StateStore {
    pub fn load(&self) -> Result<Option<NamespaceState>> {
        Ok(self.namespace.clone())
    }
}

#[derive(Debug, Clone)]
pub struct NamespaceState {
    pub session_name: String,
    pub socket_path: String,
    pub layout_signature: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TopologyPlan {
    pub signature: Option<String>,
    pub entry_window: Option<String>,
}

impl TopologyPlan {
    pub fn cloned(&self) -> Option<Self> {
        Some(self.clone())
    }
}
