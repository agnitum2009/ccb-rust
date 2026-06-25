//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/controller.py`.

use std::collections::HashMap;
use std::path::PathBuf;

use ccb_storage::paths::PathLayout;

use super::backend::BackendFactory;
use super::destroy::destroy_project_namespace;
use super::ensure::ensure_project_namespace;
use super::ensure_context::{
    Clock, EventStore as RuntimeEventStore, LayoutConfig, NamespaceController,
    NamespaceWindowPlan as InternalWindowPlan, SidebarPanePlan as InternalSidebarPanePlan,
    StateStore as RuntimeStateStore, TopologyPlan as InternalTopologyPlan,
};
use super::models::{
    ProjectNamespace, ProjectNamespaceDestroySummary, ProjectNamespaceEvent as RuntimeEvent,
    ProjectNamespaceState as RuntimeState,
};
use super::records::namespace_from_state;
use super::reflow::reflow_project_workspace;
use super::topology_plan::{NamespaceTopologyPlan, NamespaceWindowPlan, SidebarPanePlan};
use crate::services::project_namespace_state::{
    ProjectNamespaceEvent as PersistentEvent, ProjectNamespaceEventStore,
    ProjectNamespaceState as PersistentState, ProjectNamespaceStateStore,
};
use crate::{DaemonError, Result};

/// High-level controller that owns a project's tmux namespace lifecycle.
///
/// Mirrors Python `ProjectNamespaceController`.
pub struct ProjectNamespaceController {
    project_id: String,
    layout_version: i64,
    layout: PathLayout,
    clock: Clock,
    backend_factory: BackendFactory,
    state_store: ProjectNamespaceStateStore,
    event_store: ProjectNamespaceEventStore,
    pub last_materialized_agent_panes: HashMap<String, String>,
    pub last_topology_active_panes: Vec<String>,
    pub session_alive_override: Option<bool>,
}

impl ProjectNamespaceController {
    /// Create a new project namespace controller.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        layout: &PathLayout,
        project_id: &str,
        clock: Option<Clock>,
        backend_factory: Option<BackendFactory>,
        state_store: Option<ProjectNamespaceStateStore>,
        event_store: Option<ProjectNamespaceEventStore>,
        layout_version: i64,
    ) -> Result<Self> {
        let project_id = project_id.trim().to_string();
        if project_id.is_empty() {
            return Err(DaemonError::Config(
                "project_id cannot be empty".to_string(),
            ));
        }
        if layout_version <= 0 {
            return Err(DaemonError::Config(
                "layout_version must be positive".to_string(),
            ));
        }
        Ok(Self {
            project_id,
            layout_version,
            layout: layout.clone(),
            clock: clock.unwrap_or_else(|| Clock::new(crate::system::utc_now)),
            backend_factory: backend_factory.unwrap_or_default(),
            state_store: state_store.unwrap_or_else(|| ProjectNamespaceStateStore::new(layout)),
            event_store: event_store.unwrap_or_else(|| ProjectNamespaceEventStore::new(layout)),
            last_materialized_agent_panes: HashMap::new(),
            last_topology_active_panes: Vec::new(),
            session_alive_override: None,
        })
    }

    /// Load the current namespace view from persistent state.
    pub fn load(&self) -> Result<Option<ProjectNamespace>> {
        let state = self.state_store.load().map_err(storage_err)?;
        match state {
            Some(s) => {
                let runtime = runtime_state_from_persistent(&s)?;
                Ok(Some(namespace_from_state(&runtime)))
            }
            None => Ok(None),
        }
    }

    /// Ensure the project namespace exists and matches the desired configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn ensure(
        &mut self,
        layout_signature: Option<&str>,
        topology_plan: Option<&NamespaceTopologyPlan>,
        force_recreate: bool,
        recreate_reason: Option<&str>,
        session_probe_timeout_s: Option<f64>,
        terminal_size: Option<(i32, i32)>,
    ) -> Result<ProjectNamespace> {
        std::fs::create_dir_all(self.layout.ccbd_dir())?;

        let mut inner = self.build_inner_controller()?;
        let internal_plan = topology_plan.map(convert_topology_plan);

        let result = ensure_project_namespace(
            &mut inner,
            layout_signature,
            internal_plan.as_ref(),
            force_recreate,
            recreate_reason,
            session_probe_timeout_s,
            terminal_size,
        )?;

        self.persist_inner_results(&inner)?;
        self.last_materialized_agent_panes = inner.last_materialized_agent_panes.clone();
        self.last_topology_active_panes = inner.last_topology_active_panes.clone();
        Ok(result)
    }

    /// Reflow the project workspace window without killing the tmux server.
    pub fn reflow_workspace(
        &mut self,
        layout_signature: Option<&str>,
        reason: Option<&str>,
        session_probe_timeout_s: Option<f64>,
    ) -> Result<ProjectNamespace> {
        std::fs::create_dir_all(self.layout.ccbd_dir())?;

        let mut inner = self.build_inner_controller()?;
        let result = reflow_project_workspace(
            &mut inner,
            layout_signature,
            reason,
            session_probe_timeout_s,
        )?;

        self.persist_inner_results(&inner)?;
        self.last_materialized_agent_panes = inner.last_materialized_agent_panes.clone();
        self.last_topology_active_panes = inner.last_topology_active_panes.clone();
        Ok(result)
    }

    /// Destroy the project namespace and persist the destroyed state.
    pub fn destroy(
        &mut self,
        reason: &str,
        #[allow(unused_variables)] force: bool,
    ) -> Result<ProjectNamespaceDestroySummary> {
        std::fs::create_dir_all(self.layout.ccbd_dir())?;

        let mut inner = self.build_inner_controller()?;
        let summary = destroy_project_namespace(&mut inner, reason)?;
        self.persist_inner_results(&inner)?;
        Ok(summary)
    }

    fn build_inner_controller(&self) -> Result<NamespaceController> {
        let current = match self.state_store.load().map_err(storage_err)? {
            Some(state) => Some(runtime_state_from_persistent(&state)?),
            None => None,
        };
        let layout = LayoutConfig {
            project_root: self.layout.project_root.as_str().to_string(),
            ccbd_dir: PathBuf::from(self.layout.ccbd_dir().as_str()),
            ccbd_socket_path: self.layout.ccbd_socket_path().to_string(),
            ccbd_tmux_socket_path: self.layout.ccbd_tmux_socket_path().to_string(),
            ccbd_tmux_session_name: self.layout.ccbd_tmux_session_name(),
            ccbd_tmux_control_window_name: self
                .layout
                .ccbd_tmux_control_window_name()
                .to_string(),
            ccbd_tmux_workspace_window_name: self
                .layout
                .ccbd_tmux_workspace_window_name()
                .to_string(),
        };

        Ok(NamespaceController {
            project_id: self.project_id.clone(),
            layout_version: self.layout_version,
            layout,
            backend_factory: self.backend_factory.clone(),
            state_store: RuntimeStateStore { namespace: current },
            event_store: RuntimeEventStore { events: Vec::new() },
            clock: self.clock.clone(),
            last_materialized_agent_panes: self.last_materialized_agent_panes.clone(),
            last_topology_active_panes: self.last_topology_active_panes.clone(),
            session_alive_override: self.session_alive_override,
        })
    }

    fn persist_inner_results(&self, inner: &NamespaceController) -> Result<()> {
        if let Some(state) = inner.state_store.load()? {
            let persistent = persistent_state_from_runtime(&state)?;
            self.state_store.save(&persistent).map_err(storage_err)?;
        }
        for event in &inner.event_store.events {
            self.event_store
                .append(&persistent_event_from_runtime(event)?)
                .map_err(storage_err)?;
        }
        Ok(())
    }
}

fn convert_topology_plan(plan: &NamespaceTopologyPlan) -> InternalTopologyPlan {
    InternalTopologyPlan {
        signature: {
            let sig = plan.signature.trim();
            if sig.is_empty() {
                None
            } else {
                Some(sig.to_string())
            }
        },
        entry_window: plan.entry_window.clone(),
        windows: plan.windows.iter().map(convert_window_plan).collect(),
        sidebar_enabled: plan.sidebar_enabled,
    }
}

fn convert_window_plan(window: &NamespaceWindowPlan) -> InternalWindowPlan {
    InternalWindowPlan {
        name: window.name.clone(),
        order: window.order as i64,
        kind: window.kind.clone(),
        label: window.label.clone(),
        command: window.command.clone(),
        user_layout: window.user_layout.clone(),
        agent_names: window.agent_names.clone(),
        sidebar: window.sidebar.as_ref().map(convert_sidebar_plan),
    }
}

fn convert_sidebar_plan(sidebar: &SidebarPanePlan) -> InternalSidebarPanePlan {
    InternalSidebarPanePlan {
        width: sidebar.width.clone(),
        launch_args: sidebar.launch_args.clone(),
    }
}

fn runtime_state_from_persistent(state: &PersistentState) -> Result<RuntimeState> {
    Ok(RuntimeState {
        project_id: state.project_id.clone(),
        namespace_epoch: i64::try_from(state.namespace_epoch).map_err(|_| {
            DaemonError::Config(format!(
                "namespace_epoch {} exceeds i64 range",
                state.namespace_epoch
            ))
        })?,
        tmux_socket_path: state.tmux_socket_path.clone(),
        tmux_session_name: state.tmux_session_name.clone(),
        layout_version: i64::try_from(state.layout_version).map_err(|_| {
            DaemonError::Config(format!(
                "layout_version {} exceeds i64 range",
                state.layout_version
            ))
        })?,
        layout_signature: state.layout_signature.clone(),
        control_window_name: state.control_window_name.clone(),
        control_window_id: state.control_window_id.clone(),
        workspace_window_name: state.workspace_window_name.clone(),
        workspace_window_id: state.workspace_window_id.clone(),
        workspace_epoch: i64::try_from(state.workspace_epoch).map_err(|_| {
            DaemonError::Config(format!(
                "workspace_epoch {} exceeds i64 range",
                state.workspace_epoch
            ))
        })?,
        ui_attachable: state.ui_attachable,
        last_started_at: state.last_started_at.clone(),
        last_destroyed_at: state.last_destroyed_at.clone(),
        last_destroy_reason: state.last_destroy_reason.clone(),
    })
}

fn persistent_state_from_runtime(state: &RuntimeState) -> Result<PersistentState> {
    Ok(PersistentState {
        project_id: state.project_id.clone(),
        namespace_epoch: u64::try_from(state.namespace_epoch).map_err(|_| {
            DaemonError::Config(format!(
                "namespace_epoch {} cannot be negative for persistence",
                state.namespace_epoch
            ))
        })?,
        tmux_socket_path: state.tmux_socket_path.clone(),
        tmux_session_name: state.tmux_session_name.clone(),
        layout_version: u64::try_from(state.layout_version).map_err(|_| {
            DaemonError::Config(format!(
                "layout_version {} cannot be negative for persistence",
                state.layout_version
            ))
        })?,
        layout_signature: state.layout_signature.clone(),
        control_window_name: state.control_window_name.clone(),
        control_window_id: state.control_window_id.clone(),
        workspace_window_name: state.workspace_window_name.clone(),
        workspace_window_id: state.workspace_window_id.clone(),
        workspace_epoch: u64::try_from(state.workspace_epoch).map_err(|_| {
            DaemonError::Config(format!(
                "workspace_epoch {} cannot be negative for persistence",
                state.workspace_epoch
            ))
        })?,
        ui_attachable: state.ui_attachable,
        last_started_at: state.last_started_at.clone(),
        last_destroyed_at: state.last_destroyed_at.clone(),
        last_destroy_reason: state.last_destroy_reason.clone(),
    })
}

fn persistent_event_from_runtime(event: &RuntimeEvent) -> Result<PersistentEvent> {
    let mut details = std::collections::HashMap::new();
    for (k, v) in &event.details {
        details.insert(k.clone(), v.clone());
    }
    Ok(PersistentEvent {
        event_kind: event.event_kind.clone(),
        project_id: event.project_id.clone(),
        occurred_at: event.occurred_at.clone(),
        namespace_epoch: event
            .namespace_epoch
            .map(u64::try_from)
            .transpose()
            .map_err(|_| {
                DaemonError::Config(format!(
                    "namespace_epoch {:?} cannot be negative for persistence",
                    event.namespace_epoch
                ))
            })?,
        tmux_socket_path: event.tmux_socket_path.clone(),
        tmux_session_name: event.tmux_session_name.clone(),
        details,
    })
}

fn storage_err(err: anyhow::Error) -> DaemonError {
    DaemonError::Config(format!("namespace storage error: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_new_rejects_empty_project_id() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let layout = PathLayout::new(camino::Utf8PathBuf::from_path_buf(root).unwrap());
        assert!(
            ProjectNamespaceController::new(&layout, "   ", None, None, None, None, 1,).is_err()
        );
    }

    #[test]
    fn test_controller_new_rejects_non_positive_layout_version() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let layout = PathLayout::new(camino::Utf8PathBuf::from_path_buf(root).unwrap());
        assert!(
            ProjectNamespaceController::new(&layout, "p1", None, None, None, None, 0,).is_err()
        );
    }
}
