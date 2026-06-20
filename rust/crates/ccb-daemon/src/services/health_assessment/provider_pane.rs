//! Mirrors Python `lib/ccbd/services/health_assessment/provider_pane.py`.

use std::collections::HashMap;
use std::path::PathBuf;

use ccb_provider_core::session_binding::session_terminal;

use super::models::{ProviderPaneAssessment, SessionBinding};
use super::tmux::session_backend;
use super::tmux_runtime::namespace::{
    pane_outside_project_namespace, NamespaceStateStore, RuntimeInfo as NamespaceRuntimeInfo,
};
use super::tmux_runtime::state::tmux_pane_state;
use crate::services::provider_runtime_facts::load_provider_session;

/// Runtime information needed to assess a provider pane.
#[derive(Debug, Clone)]
pub struct ProviderRuntimeInfo {
    pub agent_name: String,
    pub runtime_ref: Option<String>,
    pub workspace_path: Option<String>,
    pub project_id: String,
    pub slot_key: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub tmux_window_name: Option<String>,
}

/// View of an agent spec used during pane assessment.
pub trait AgentSpecView {
    fn provider(&self) -> &str;
}

/// Registry that can resolve an agent spec view by name.
pub trait AgentSpecResolver {
    fn spec_for(&self, agent_name: &str) -> Option<&dyn AgentSpecView>;
}

impl AgentSpecView for ccb_agents::models::AgentSpec {
    fn provider(&self) -> &str {
        &self.provider
    }
}

/// Assess a provider pane for health monitoring.
///
/// Mirrors Python `assess_provider_pane`.
pub fn assess_provider_pane<Registry, Store>(
    runtime: &ProviderRuntimeInfo,
    registry: &Registry,
    session_bindings: &HashMap<String, Box<dyn SessionBinding>>,
    namespace_state_store: &Store,
) -> Option<ProviderPaneAssessment>
where
    Registry: AgentSpecResolver,
    Store: NamespaceStateStore,
{
    if !is_tmux_runtime(runtime) {
        return None;
    }
    let binding = resolve_binding(runtime, registry, session_bindings)?;
    let workspace_path = workspace_path(runtime)?;
    let session = load_provider_session(binding.as_ref(), &workspace_path, &runtime.agent_name);
    if session.is_none() {
        return Some(ProviderPaneAssessment {
            binding: Some(binding),
            session: None,
            terminal: None,
            pane_state: Some("missing".to_string()),
            health: "session-missing".to_string(),
        });
    }
    let session = session.unwrap();
    let terminal = session_terminal(&session)
        .map(|s| s.to_lowercase())
        .filter(|s| !s.is_empty());

    if terminal.as_deref() != Some("tmux") {
        return Some(ProviderPaneAssessment {
            binding: Some(binding),
            session: Some(session),
            terminal,
            pane_state: None,
            health: "healthy".to_string(),
        });
    }

    let backend = session_backend(&session);
    let pane_id = session.pane_id.as_deref().unwrap_or("").trim();
    let pane_state = if pane_id.is_empty() {
        "missing".to_string()
    } else {
        let state = tmux_pane_state(&session, backend.as_ref(), pane_id);
        if state == "alive" {
            if let Some(ref adapter) = backend {
                let ns_runtime = namespace_runtime_info(runtime);
                if pane_outside_project_namespace(
                    &ns_runtime,
                    namespace_state_store,
                    Some(adapter),
                    pane_id,
                ) {
                    "foreign".to_string()
                } else {
                    state
                }
            } else {
                state
            }
        } else {
            state
        }
    };
    let health = health_from_pane_state(&pane_state);
    Some(ProviderPaneAssessment {
        binding: Some(binding),
        session: Some(session),
        terminal,
        pane_state: Some(pane_state),
        health,
    })
}

fn is_tmux_runtime(runtime: &ProviderRuntimeInfo) -> bool {
    runtime
        .runtime_ref
        .as_deref()
        .unwrap_or("")
        .trim()
        .starts_with("tmux:")
}

fn resolve_binding<Registry>(
    runtime: &ProviderRuntimeInfo,
    registry: &Registry,
    session_bindings: &HashMap<String, Box<dyn SessionBinding>>,
) -> Option<Box<dyn SessionBinding>>
where
    Registry: AgentSpecResolver,
{
    let spec = registry.spec_for(&runtime.agent_name)?;
    let binding = session_bindings.get(spec.provider())?;
    Some(binding.clone_box())
}

fn workspace_path(runtime: &ProviderRuntimeInfo) -> Option<PathBuf> {
    runtime
        .workspace_path
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

fn namespace_runtime_info(runtime: &ProviderRuntimeInfo) -> NamespaceRuntimeInfo {
    NamespaceRuntimeInfo {
        project_id: runtime.project_id.clone(),
        agent_name: runtime.agent_name.clone(),
        slot_key: runtime.slot_key.clone(),
        tmux_socket_path: runtime.tmux_socket_path.clone(),
        tmux_window_name: runtime.tmux_window_name.clone(),
    }
}

fn health_from_pane_state(pane_state: &str) -> String {
    match pane_state {
        "alive" => "healthy",
        "missing" => "pane-missing",
        "foreign" => "pane-foreign",
        _ => "pane-dead",
    }
    .to_string()
}
