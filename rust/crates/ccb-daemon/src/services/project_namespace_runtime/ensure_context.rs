//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/ensure_context.py`.
//! 1:1 file alignment stub.

use std::path::PathBuf;
use std::sync::Arc;

use crate::services::project_namespace_runtime::backend::{
    build_backend, prepare_server, session_alive, Backend, BackendFactory,
};
use crate::services::project_namespace_runtime::models::{
    ProjectNamespaceEvent, ProjectNamespaceState,
};
use crate::services::project_namespace_runtime::records::normalized_layout_signature;
use crate::Result;

pub type NamespaceState = ProjectNamespaceState;

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

    let topology_signature = topology_plan
        .and_then(|p| p.signature.as_ref())
        .map(|s| s.as_str());
    let layout_sig = topology_signature.unwrap_or(layout_signature.unwrap_or(""));
    let desired_layout_signature = normalized_layout_signature(Some(layout_sig));

    let desired_control_window_name = controller.layout.ccbd_tmux_control_window_name.clone();

    let desired_workspace_window_name = topology_plan
        .and_then(|p| {
            let entry = p.entry_window.trim();
            if entry.is_empty() {
                None
            } else {
                Some(entry.to_string())
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

    if let Some(override_value) = controller.session_alive_override {
        return Ok(context.with_updates(None, Some(override_value), None));
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
    pub project_id: String,
    pub layout_version: i64,
    pub layout: LayoutConfig,
    pub backend_factory: BackendFactory,
    pub state_store: StateStore,
    pub event_store: EventStore,
    pub clock: Clock,
    pub last_materialized_agent_panes: std::collections::HashMap<String, String>,
    pub last_topology_active_panes: Vec<String>,
    /// Test-only override for session liveness. When `Some`, `refresh_session_liveness`
    /// returns this value instead of probing tmux.
    pub session_alive_override: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub project_root: String,
    pub ccbd_dir: PathBuf,
    pub ccbd_socket_path: String,
    pub ccbd_tmux_socket_path: String,
    pub ccbd_tmux_session_name: String,
    pub ccbd_tmux_control_window_name: String,
    pub ccbd_tmux_workspace_window_name: String,
}

#[derive(Debug, Clone, Default)]
pub struct StateStore {
    pub namespace: Option<NamespaceState>,
}

impl StateStore {
    pub fn load(&self) -> Result<Option<NamespaceState>> {
        Ok(self.namespace.clone())
    }

    pub fn save(&mut self, state: NamespaceState) {
        self.namespace = Some(state);
    }
}

#[derive(Debug, Clone, Default)]
pub struct EventStore {
    pub events: Vec<ProjectNamespaceEvent>,
}

impl EventStore {
    pub fn append(&mut self, event: ProjectNamespaceEvent) {
        self.events.push(event);
    }
}

/// Callable clock used to produce timestamp strings.
#[derive(Clone)]
pub struct Clock {
    inner: Arc<dyn Fn() -> String + Send + Sync>,
}

impl Clock {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        Self { inner: Arc::new(f) }
    }
}

impl std::ops::Deref for Clock {
    type Target = dyn Fn() -> String + Send + Sync;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl std::fmt::Debug for Clock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Clock").finish()
    }
}

#[derive(Debug, Clone)]
pub struct TopologyPlan {
    pub signature: Option<String>,
    pub entry_window: String,
    pub windows: Vec<NamespaceWindowPlan>,
    pub sidebar_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct NamespaceWindowPlan {
    pub name: String,
    pub order: i64,
    pub kind: String,
    pub label: Option<String>,
    pub command: Option<String>,
    pub user_layout: String,
    pub agent_names: Vec<String>,
    pub sidebar: Option<SidebarPanePlan>,
}

#[derive(Debug, Clone)]
pub struct SidebarPanePlan {
    pub width: String,
    pub launch_args: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_layout() -> LayoutConfig {
        LayoutConfig {
            project_root: "/tmp/ccb-test".to_string(),
            ccbd_dir: PathBuf::from("/tmp/ccb-test/.ccbr"),
            ccbd_socket_path: "/tmp/ccb-test/.ccbr/ccbd.sock".to_string(),
            ccbd_tmux_socket_path: "/tmp/ccb-test/.ccbr/tmux.sock".to_string(),
            ccbd_tmux_session_name: "ccb-test".to_string(),
            ccbd_tmux_control_window_name: "control".to_string(),
            ccbd_tmux_workspace_window_name: "workspace".to_string(),
        }
    }

    fn test_controller() -> NamespaceController {
        NamespaceController {
            project_id: "p1".to_string(),
            layout_version: 1,
            layout: test_layout(),
            backend_factory: BackendFactory::default(),
            state_store: StateStore::default(),
            event_store: EventStore::default(),
            clock: Clock::new(|| "2024-01-01T00:00:00Z".to_string()),
            last_materialized_agent_panes: std::collections::HashMap::new(),
            last_topology_active_panes: Vec::new(),
            session_alive_override: None,
        }
    }

    #[test]
    fn test_desired_namespace_state_basic() {
        let controller = test_controller();
        let (socket, session, sig, control, workspace) =
            desired_namespace_state(&controller, Some("sig-a"), None);
        assert_eq!(socket, "/tmp/ccb-test/.ccbr/tmux.sock");
        assert_eq!(session, "ccb-test");
        assert_eq!(sig, Some("sig-a".to_string()));
        assert_eq!(control, "control");
        assert_eq!(workspace, "workspace");
    }

    #[test]
    fn test_desired_namespace_state_topology_overrides_signature() {
        let controller = test_controller();
        let plan = TopologyPlan {
            signature: Some("topo-sig".to_string()),
            entry_window: "entry".to_string(),
            windows: Vec::new(),
            sidebar_enabled: false,
        };
        let (_, _, sig, _, workspace) =
            desired_namespace_state(&controller, Some("sig-a"), Some(&plan));
        assert_eq!(sig, Some("topo-sig".to_string()));
        assert_eq!(workspace, "entry");
    }

    #[test]
    fn test_desired_namespace_state_empty_signature_normalized() {
        let controller = test_controller();
        let (_, _, sig, _, _) = desired_namespace_state(&controller, Some("   "), None);
        assert_eq!(sig, None);
    }

    #[test]
    fn test_load_namespace_context_defaults() {
        let controller = test_controller();
        let ctx = load_namespace_context(&controller, None, None, None).unwrap();
        assert!(ctx.current.is_none());
        assert!(!ctx.session_is_alive);
        assert_eq!(ctx.desired_session_name, "ccb-test");
        assert_eq!(ctx.recreate_cause, None);
    }

    #[test]
    fn test_load_namespace_context_preserves_recreate_reason() {
        let mut controller = test_controller();
        controller.state_store.namespace = Some(NamespaceState {
            project_id: "p1".to_string(),
            namespace_epoch: 2,
            tmux_socket_path: "/tmp/ccb-test/.ccbr/tmux.sock".to_string(),
            tmux_session_name: "ccb-test".to_string(),
            layout_version: 1,
            layout_signature: Some("old".to_string()),
            control_window_name: Some("control".to_string()),
            control_window_id: Some("@0".to_string()),
            workspace_window_name: Some("workspace".to_string()),
            workspace_window_id: Some("@1".to_string()),
            workspace_epoch: 1,
            ui_attachable: true,
            last_started_at: Some("2024-01-01".to_string()),
            last_destroyed_at: None,
            last_destroy_reason: None,
        });
        let ctx = load_namespace_context(&controller, Some("new"), None, Some("forced")).unwrap();
        assert!(ctx.current.is_some());
        assert_eq!(ctx.desired_layout_signature, Some("new".to_string()));
        assert_eq!(ctx.recreate_cause, Some("forced".to_string()));
    }

    #[test]
    fn test_with_updates_selective() {
        let controller = test_controller();
        let ctx = load_namespace_context(&controller, None, None, Some("original")).unwrap();
        let updated = ctx.with_updates(None, Some(true), Some("updated".to_string()));
        assert!(updated.session_is_alive);
        assert_eq!(updated.recreate_cause, Some("updated".to_string()));
        assert_eq!(updated.desired_session_name, ctx.desired_session_name);
    }

    #[test]
    fn test_rebuild_namespace_backend() {
        let controller = test_controller();
        let backend = rebuild_namespace_backend(&controller, "/tmp/another.sock").unwrap();
        assert_eq!(backend.socket_path, "/tmp/another.sock");
    }
}
