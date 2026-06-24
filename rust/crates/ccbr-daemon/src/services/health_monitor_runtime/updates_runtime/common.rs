//! Mirrors Python `lib/ccbrd/services/health_monitor_runtime/updates_runtime/common.py`.

use ccbr_agents::models::AgentRuntime;

use crate::services::provider_runtime_facts::ProviderRuntimeFacts;
use ccbr_provider_core::session_binding::{
    session_pane_title_marker, session_ref, session_runtime_ref, session_terminal, Session,
};

/// Build a map of runtime fields from provider facts, falling back to the
/// current runtime for missing values.
///
/// Mirrors Python `runtime_fields_from_facts`.
pub fn runtime_fields_from_facts(
    runtime: &AgentRuntime,
    facts: &ProviderRuntimeFacts,
) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    if let Some(v) = facts.runtime_ref.as_ref().or(runtime.runtime_ref.as_ref()) {
        fields.push(("runtime_ref".to_string(), v.clone()));
    }
    if let Some(v) = facts
        .runtime_root
        .as_ref()
        .or(runtime.runtime_root.as_ref())
    {
        fields.push(("runtime_root".to_string(), v.clone()));
    }
    if facts.runtime_pid.is_some() || runtime.runtime_pid.is_some() {
        let pid = facts.runtime_pid.or(runtime.runtime_pid).unwrap_or(0);
        fields.push(("runtime_pid".to_string(), pid.to_string()));
    }
    if let Some(v) = facts
        .terminal_backend
        .as_ref()
        .or(runtime.terminal_backend.as_ref())
    {
        fields.push(("terminal_backend".to_string(), v.clone()));
    }
    if let Some(v) = facts.pane_id.as_ref().or(runtime.pane_id.as_ref()) {
        fields.push(("pane_id".to_string(), v.clone()));
    }
    if let Some(v) = facts
        .pane_title_marker
        .as_ref()
        .or(runtime.pane_title_marker.as_ref())
    {
        fields.push(("pane_title_marker".to_string(), v.clone()));
    }
    if let Some(v) = facts
        .tmux_socket_name
        .as_ref()
        .or(runtime.tmux_socket_name.as_ref())
    {
        fields.push(("tmux_socket_name".to_string(), v.clone()));
    }
    if let Some(v) = facts
        .tmux_socket_path
        .as_ref()
        .or(runtime.tmux_socket_path.as_ref())
    {
        fields.push(("tmux_socket_path".to_string(), v.clone()));
    }
    if let Some(v) = facts
        .session_file
        .as_ref()
        .or(runtime.session_file.as_ref())
    {
        fields.push(("session_file".to_string(), v.clone()));
    }
    if let Some(v) = facts.session_id.as_ref().or(runtime.session_id.as_ref()) {
        fields.push(("session_id".to_string(), v.clone()));
    }
    fields
}

/// Build a map of runtime fields from a session, falling back to the current
/// runtime for missing values.
///
/// Mirrors Python `runtime_fields_from_session`.
pub fn runtime_fields_from_session(
    runtime: &AgentRuntime,
    session: &Session,
    binding: Option<&dyn crate::services::health_assessment::models::SessionBinding>,
) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let next_runtime_ref = session_runtime_ref(session, None)
        .or_else(|| runtime.runtime_ref.clone())
        .unwrap_or_default();
    if !next_runtime_ref.is_empty() {
        fields.push(("runtime_ref".to_string(), next_runtime_ref));
    }
    if let Some(v) = runtime.runtime_root.as_ref() {
        fields.push(("runtime_root".to_string(), v.clone()));
    }
    if let Some(v) = runtime.runtime_pid {
        fields.push(("runtime_pid".to_string(), v.to_string()));
    }
    let terminal = session_terminal(session)
        .or_else(|| runtime.terminal_backend.clone())
        .unwrap_or_default();
    if !terminal.is_empty() {
        fields.push(("terminal_backend".to_string(), terminal));
    }
    let pane_id = session
        .pane_id
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| runtime.pane_id.clone())
        .unwrap_or_default();
    if !pane_id.is_empty() {
        fields.push(("pane_id".to_string(), pane_id));
    }
    let pane_title = session_pane_title_marker(session)
        .or_else(|| runtime.pane_title_marker.clone())
        .unwrap_or_default();
    if !pane_title.is_empty() {
        fields.push(("pane_title_marker".to_string(), pane_title));
    }
    if let Some(v) = runtime.tmux_socket_name.as_ref() {
        fields.push(("tmux_socket_name".to_string(), v.clone()));
    }
    if let Some(v) = runtime.tmux_socket_path.as_ref() {
        fields.push(("tmux_socket_path".to_string(), v.clone()));
    }
    if let Some(v) = runtime.session_file.as_ref() {
        fields.push(("session_file".to_string(), v.clone()));
    }
    if let Some(v) = runtime.session_id.as_ref() {
        fields.push(("session_id".to_string(), v.clone()));
    }
    if let Some(b) = binding {
        if let Some(v) = session_ref(session, b.session_id_attr(), b.session_path_attr()) {
            fields.push(("session_ref".to_string(), v));
        }
    }
    fields
}

/// Drop fields whose values are managed explicitly by the caller.
///
/// Mirrors Python `drop_explicit_runtime_fields`.
pub fn drop_explicit_runtime_fields(
    fields: &[(String, String)],
    explicit_fields: &[&str],
) -> Vec<(String, String)> {
    fields
        .iter()
        .filter(|(k, _)| !explicit_fields.contains(&k.as_str()))
        .cloned()
        .collect()
}

/// Compute pane state and active pane id for a given health classification.
///
/// Mirrors Python `pane_state_for_health`.
pub fn pane_state_for_health(
    runtime: &AgentRuntime,
    health: &str,
    pane_id: Option<&str>,
) -> (Option<String>, Option<String>) {
    let mut next_pane_state = runtime.pane_state.clone();
    let mut next_active_pane_id = runtime
        .active_pane_id
        .as_deref()
        .or(pane_id)
        .or(runtime.pane_id.as_deref())
        .map(|s| s.to_string());
    match health {
        "pane-dead" | "orphaned" => next_pane_state = Some("dead".to_string()),
        "pane-missing" | "session-missing" => next_pane_state = Some("missing".to_string()),
        "pane-foreign" => {
            next_pane_state = Some("foreign".to_string());
            next_active_pane_id = None;
        }
        _ => {}
    }
    (next_pane_state, next_active_pane_id)
}
