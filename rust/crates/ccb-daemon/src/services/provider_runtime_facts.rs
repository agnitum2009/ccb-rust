//! Mirrors Python `lib/ccbd/services/provider_runtime_facts.py`.

use std::path::Path;

use crate::services::health_assessment::models::SessionBinding;
use ccb_provider_core::instance_resolution::named_agent_instance;
use ccb_provider_core::session_binding::{
    session_ccb_session_id, session_file, session_id, session_pane_title_marker, session_ref,
    session_runtime_pid, session_runtime_ref, session_runtime_root, session_terminal,
    session_tmux_socket_name, session_tmux_socket_path, Session,
};

/// Provider runtime facts extracted from a loaded session and binding.
///
/// Mirrors Python `ccbd.services.provider_runtime_facts.ProviderRuntimeFacts`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderRuntimeFacts {
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub runtime_root: Option<String>,
    pub runtime_pid: Option<i64>,
    pub terminal_backend: Option<String>,
    pub pane_id: Option<String>,
    pub pane_title_marker: Option<String>,
    pub pane_state: Option<String>,
    pub tmux_socket_name: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub session_file: Option<String>,
    pub session_id: Option<String>,
    pub ccb_session_id: Option<String>,
}

/// Build provider runtime facts from a loaded session and binding.
///
/// Mirrors Python `ccbd.services.provider_runtime_facts.build_provider_runtime_facts`.
pub fn build_provider_runtime_facts(
    session: &Session,
    binding: &dyn SessionBinding,
    provider: &str,
    pane_id_override: Option<&str>,
) -> ProviderRuntimeFacts {
    let pane_id = pane_id_override
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            session
                .pane_id
                .as_deref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    ProviderRuntimeFacts {
        runtime_ref: session_runtime_ref(session, pane_id_override),
        session_ref: session_ref(
            session,
            binding.session_id_attr(),
            binding.session_path_attr(),
        ),
        runtime_root: session_runtime_root(session),
        runtime_pid: session_runtime_pid(session, provider).map(i64::from),
        terminal_backend: session_terminal(session),
        pane_id: pane_id.clone(),
        pane_title_marker: session_pane_title_marker(session),
        pane_state: pane_id.as_deref().map(|_| "alive".to_string()),
        tmux_socket_name: session_tmux_socket_name(session),
        tmux_socket_path: session_tmux_socket_path(session),
        session_file: session_file(session),
        session_id: session_id(session, binding.session_id_attr()),
        ccb_session_id: session_ccb_session_id(session),
    }
}

/// Load a provider session for an agent using the supplied binding.
///
/// Mirrors Python `load_provider_session`.
pub fn load_provider_session(
    binding: &dyn SessionBinding,
    workspace_path: &Path,
    agent_name: &str,
) -> Option<Session> {
    let instance = named_agent_instance(agent_name, binding.provider());
    binding.load_session(workspace_path, instance.as_deref())
}
