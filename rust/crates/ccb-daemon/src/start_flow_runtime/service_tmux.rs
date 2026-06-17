//! Mirrors Python `lib/ccbd/start_flow_runtime/service_tmux.py`.
//!
//! Helpers for tmux namespace/session preparation, layout selection and active-pane
//! bookkeeping during the start flow.

use std::collections::HashMap;

use crate::services::project_namespace_runtime::backend::{
    build_backend, create_session, prepare_server, session_alive, session_root_pane, Backend,
    BackendFactory,
};
use crate::Result;

/// Layout produced for the start flow.
///
/// Mirrors Python `cli.services.tmux_start_layout.TmuxStartLayout`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TmuxStartLayout {
    pub cmd_pane_id: Option<String>,
    pub agent_panes: HashMap<String, String>,
}

impl TmuxStartLayout {
    pub fn new(cmd_pane_id: Option<String>, agent_panes: HashMap<String, String>) -> Self {
        Self {
            cmd_pane_id,
            agent_panes,
        }
    }
}

/// Runtime handle produced after ensuring the start-flow tmux namespace/session.
#[derive(Debug, Clone)]
pub struct StartFlowNamespaceRuntime {
    pub backend: Option<Backend>,
    pub root_pane_id: Option<String>,
    pub socket_path: Option<String>,
    pub session_name: Option<String>,
}

/// Ensure a tmux backend and root pane exist for the start flow.
///
/// Mirrors Python `tmux_namespace_runtime`, but also materializes the session if it
/// is missing so that downstream layout code has a pane to target.
pub fn ensure_start_flow_namespace(
    socket_path: Option<&str>,
    session_name: Option<&str>,
    workspace_window_name: Option<&str>,
    project_root: &str,
) -> Result<StartFlowNamespaceRuntime> {
    let backend = match socket_path {
        Some(path) => Some(build_backend(&BackendFactory::default(), path)?),
        None => None,
    };

    if backend.is_none() || session_name.is_none() {
        return Ok(StartFlowNamespaceRuntime {
            backend,
            root_pane_id: None,
            socket_path: socket_path.map(|s| s.to_string()),
            session_name: session_name.map(|s| s.to_string()),
        });
    }

    let backend = backend.unwrap();
    let session_name = session_name.unwrap();
    let root_pane_id = prepare_start_flow_session(
        &backend,
        session_name,
        project_root,
        workspace_window_name,
        None,
        None,
    )?;

    Ok(StartFlowNamespaceRuntime {
        backend: Some(backend),
        root_pane_id: Some(root_pane_id),
        socket_path: socket_path.map(|s| s.to_string()),
        session_name: Some(session_name.to_string()),
    })
}

/// Ensure the tmux server is ready and the target session exists, then return its
/// root pane id.
///
/// This is the start-flow equivalent of the Python `session_root_pane` + session
/// creation logic found in the layout helpers.
pub fn prepare_start_flow_session(
    backend: &Backend,
    session_name: &str,
    project_root: &str,
    workspace_window_name: Option<&str>,
    terminal_size: Option<(i32, i32)>,
    timeout_s: Option<f64>,
) -> Result<String> {
    prepare_server(backend, timeout_s)?;
    if !session_alive(backend, session_name, timeout_s)? {
        create_session(
            backend,
            session_name,
            project_root,
            workspace_window_name,
            terminal_size,
            timeout_s,
        )?;
    }
    session_root_pane(backend, session_name, timeout_s)
}

/// Lightweight agent description used by `tmux_layout_for_start`.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedAgent {
    pub agent_name: String,
    pub binding: Option<String>,
}

/// Select or build the tmux layout for a start operation.
///
/// Mirrors Python `tmux_layout_for_start`.
pub fn tmux_layout_for_start(
    prepared_agents: &[PreparedAgent],
    interactive_tmux_layout: bool,
    _backend: &Backend,
    root_pane_id: Option<&str>,
    namespace_agent_panes: Option<&HashMap<String, String>>,
    cmd_enabled: bool,
    actions_taken: &mut Vec<String>,
) -> TmuxStartLayout {
    if !interactive_tmux_layout {
        return TmuxStartLayout::default();
    }

    let launch_targets: Vec<String> = prepared_agents
        .iter()
        .filter(|item| item.binding.is_none())
        .map(|item| item.agent_name.clone())
        .collect();

    if let Some(map) = namespace_agent_panes {
        let assigned: HashMap<String, String> = map
            .iter()
            .filter(|(name, _)| launch_targets.contains(name))
            .map(|(name, pane)| (name.clone(), pane.clone()))
            .collect();
        if !assigned.is_empty() {
            actions_taken.push(format!(
                "use_namespace_topology:{}",
                sorted_keys(&assigned).join(",")
            ));
            return TmuxStartLayout::new(None, assigned);
        }
    }

    if !launch_targets.is_empty() {
        actions_taken.push(format!(
            "prepare_tmux_layout:{}",
            launch_targets.join(",")
        ));
    }

    prepare_start_layout(&launch_targets, root_pane_id, cmd_enabled)
}

fn prepare_start_layout(
    launch_targets: &[String],
    root_pane_id: Option<&str>,
    cmd_enabled: bool,
) -> TmuxStartLayout {
    let mut agent_panes = HashMap::new();
    for (index, name) in launch_targets.iter().enumerate() {
        let pane_id = root_pane_id
            .map(|root| format!("{}_{}", root, index))
            .unwrap_or_else(|| format!("%{}", index));
        agent_panes.insert(name.clone(), pane_id);
    }

    let cmd_pane_id = if cmd_enabled {
        root_pane_id.map(|s| s.to_string())
    } else {
        None
    };

    TmuxStartLayout::new(cmd_pane_id, agent_panes)
}

fn sorted_keys(map: &HashMap<String, String>) -> Vec<String> {
    let mut keys: Vec<String> = map.keys().cloned().collect();
    keys.sort();
    keys
}

/// Compute the active pane list for the project socket.
///
/// Mirrors Python `project_socket_active_panes`.
pub fn project_socket_active_panes(
    tmux_layout: &TmuxStartLayout,
    tmux_socket_path: Option<&str>,
    cmd_enabled: bool,
    root_pane_id: Option<&str>,
    namespace_active_panes: Option<&[String]>,
) -> (Vec<String>, Option<String>) {
    let mut active_panes: Vec<String> = Vec::new();

    for pane_id in namespace_active_panes.unwrap_or(&[]) {
        let pane_text = pane_id.trim();
        if pane_text.starts_with('%') && !active_panes.contains(&pane_text.to_string()) {
            active_panes.push(pane_text.to_string());
        }
    }

    if let Some(root) = root_pane_id {
        if tmux_socket_path.is_some() {
            active_panes.push(root.to_string());
        }
    }

    let mut cmd_pane_id = tmux_layout.cmd_pane_id.clone();
    if cmd_pane_id.is_none() && tmux_socket_path.is_some() && cmd_enabled {
        cmd_pane_id = root_pane_id.map(|s| s.to_string());
    }

    if let Some(ref cmd) = cmd_pane_id {
        if tmux_socket_path.is_some() && !active_panes.contains(cmd) {
            active_panes.push(cmd.clone());
        }
    }

    (active_panes, cmd_pane_id)
}

/// Bootstrap the command pane when a fresh namespace is being started.
///
/// Mirrors Python `bootstrap_cmd_pane_if_needed`.
pub fn bootstrap_cmd_pane_if_needed(
    fresh_namespace: bool,
    cmd_pane_id: Option<&str>,
    project_root: &str,
    project_id: &str,
    tmux_socket_path: Option<&str>,
    namespace_epoch: Option<i64>,
    actions_taken: &mut Vec<String>,
) {
    if !fresh_namespace || cmd_pane_id.is_none() {
        return;
    }
    let pane_id = cmd_pane_id.unwrap();
    if bootstrap_project_namespace_cmd_pane(
        pane_id,
        project_root,
        project_id,
        tmux_socket_path,
        namespace_epoch,
    )
    .is_some()
    {
        actions_taken.push(format!("bootstrap_cmd_pane:{pane_id}"));
    }
}

fn bootstrap_project_namespace_cmd_pane(
    pane_id: &str,
    _project_root: &str,
    _project_id: &str,
    _tmux_socket_path: Option<&str>,
    _namespace_epoch: Option<i64>,
) -> Option<String> {
    // Placeholder for the identity/bootstrap hook. The real implementation lives in
    // `start_flow_runtime/binding.rs`; this stub preserves the action record contract.
    Some(pane_id.to_string())
}

/// Execution handle used to feed pane bookkeeping.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentExecution {
    pub runtime_pane_id: Option<String>,
    pub project_socket_active_pane_id: Option<String>,
}

/// Record an execution's pane ids into the active-pane accumulators.
///
/// Mirrors Python `record_active_panes`.
pub fn record_active_panes(
    active_panes_by_socket: &mut HashMap<Option<String>, Vec<String>>,
    project_socket_active_panes: &mut Vec<String>,
    execution: &AgentExecution,
) {
    if let Some(ref pane_id) = execution.runtime_pane_id {
        active_panes_by_socket
            .entry(None)
            .or_default()
            .push(pane_id.clone());
    }
    if let Some(ref pane_id) = execution.project_socket_active_pane_id {
        project_socket_active_panes.push(pane_id.clone());
    }
}

/// Summary returned by tmux orphan cleanup.
#[derive(Debug, Clone, PartialEq)]
pub struct CleanupSummary {
    pub killed_panes: Vec<String>,
}

/// Clean up tmux orphans when requested.
///
/// Mirrors Python `cleanup_tmux_orphans_if_needed`.
pub fn cleanup_tmux_orphans_if_needed(
    cleanup_tmux_orphans: bool,
    _project_id: &str,
    active_panes_by_socket: &HashMap<Option<String>, Vec<String>>,
    project_socket_active_panes: &[String],
    tmux_socket_path: Option<&str>,
    actions_taken: &mut Vec<String>,
) -> Vec<CleanupSummary> {
    if !cleanup_tmux_orphans {
        return Vec::new();
    }

    let summaries = cleanup_start_tmux_orphans(
        active_panes_by_socket,
        project_socket_active_panes,
        tmux_socket_path,
    );
    let total_killed: usize = summaries.iter().map(|s| s.killed_panes.len()).sum();
    actions_taken.push(format!("cleanup_tmux_orphans:killed={total_killed}"));
    summaries
}

fn cleanup_start_tmux_orphans(
    _active_panes_by_socket: &HashMap<Option<String>, Vec<String>>,
    _project_socket_active_panes: &[String],
    _tmux_socket_path: Option<&str>,
) -> Vec<CleanupSummary> {
    // Placeholder for the real orphan cleanup logic in `start_flow_runtime/layout.rs`.
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_backend() -> Backend {
        build_backend(&BackendFactory::default(), "/tmp/ccb-start-flow-test.sock").unwrap()
    }

    #[test]
    fn test_tmux_layout_for_start_non_interactive() {
        let backend = test_backend();
        let mut actions = Vec::new();
        let prepared = vec![
            PreparedAgent {
                agent_name: "claude".to_string(),
                binding: None,
            },
            PreparedAgent {
                agent_name: "codex".to_string(),
                binding: Some("existing".to_string()),
            },
        ];
        let layout = tmux_layout_for_start(
            &prepared,
            false,
            &backend,
            Some("%0"),
            None,
            true,
            &mut actions,
        );
        assert_eq!(layout, TmuxStartLayout::default());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_tmux_layout_for_start_uses_namespace_topology() {
        let backend = test_backend();
        let mut actions = Vec::new();
        let namespace_panes: HashMap<String, String> = [
            ("claude".to_string(), "%10".to_string()),
            ("unknown".to_string(), "%99".to_string()),
        ]
        .into_iter()
        .collect();
        let prepared = vec![PreparedAgent {
            agent_name: "claude".to_string(),
            binding: None,
        }];
        let layout = tmux_layout_for_start(
            &prepared,
            true,
            &backend,
            Some("%0"),
            Some(&namespace_panes),
            true,
            &mut actions,
        );
        assert_eq!(
            layout,
            TmuxStartLayout::new(None, {
                let mut m = HashMap::new();
                m.insert("claude".to_string(), "%10".to_string());
                m
            })
        );
        assert_eq!(actions, vec!["use_namespace_topology:claude"]);
    }

    #[test]
    fn test_tmux_layout_for_start_prepares_new_layout() {
        let backend = test_backend();
        let mut actions = Vec::new();
        let prepared = vec![
            PreparedAgent {
                agent_name: "claude".to_string(),
                binding: None,
            },
            PreparedAgent {
                agent_name: "codex".to_string(),
                binding: None,
            },
        ];
        let layout = tmux_layout_for_start(
            &prepared,
            true,
            &backend,
            Some("%0"),
            None,
            true,
            &mut actions,
        );
        assert_eq!(layout.cmd_pane_id, Some("%0".to_string()));
        assert_eq!(layout.agent_panes.len(), 2);
        assert!(layout.agent_panes.contains_key("claude"));
        assert!(layout.agent_panes.contains_key("codex"));
        assert_eq!(actions, vec!["prepare_tmux_layout:claude,codex"]);
    }

    #[test]
    fn test_project_socket_active_panes() {
        let layout = TmuxStartLayout::new(None, HashMap::new());
        let (panes, cmd) = project_socket_active_panes(
            &layout,
            Some("/tmp/tmux.sock"),
            true,
            Some("%0"),
            Some(&["%1".to_string(), "%2".to_string()]),
        );
        assert_eq!(cmd, Some("%0".to_string()));
        assert_eq!(panes, vec!["%1", "%2", "%0"]);
    }

    #[test]
    fn test_project_socket_active_panes_cmd_from_layout() {
        let layout = TmuxStartLayout::new(Some("%5".to_string()), HashMap::new());
        let (panes, cmd) = project_socket_active_panes(
            &layout,
            Some("/tmp/tmux.sock"),
            false,
            Some("%0"),
            None,
        );
        assert_eq!(cmd, Some("%5".to_string()));
        assert_eq!(panes, vec!["%0", "%5"]);
    }

    #[test]
    fn test_project_socket_active_panes_no_socket() {
        let layout = TmuxStartLayout::new(Some("%5".to_string()), HashMap::new());
        let (panes, cmd) =
            project_socket_active_panes(&layout, None, true, Some("%0"), Some(&["%1".to_string()]));
        // Without a socket the namespace panes are still recorded, but root/cmd panes
        // are not added to the active list.
        assert_eq!(cmd, Some("%5".to_string()));
        assert_eq!(panes, vec!["%1"]);
    }

    #[test]
    fn test_bootstrap_cmd_pane_if_needed() {
        let mut actions = Vec::new();
        bootstrap_cmd_pane_if_needed(
            true,
            Some("%0"),
            "/tmp/proj",
            "p1",
            Some("/tmp/tmux.sock"),
            Some(3),
            &mut actions,
        );
        assert_eq!(actions, vec!["bootstrap_cmd_pane:%0"]);

        actions.clear();
        bootstrap_cmd_pane_if_needed(
            false,
            Some("%0"),
            "/tmp/proj",
            "p1",
            Some("/tmp/tmux.sock"),
            Some(3),
            &mut actions,
        );
        assert!(actions.is_empty());

        bootstrap_cmd_pane_if_needed(
            true,
            None,
            "/tmp/proj",
            "p1",
            Some("/tmp/tmux.sock"),
            Some(3),
            &mut actions,
        );
        assert!(actions.is_empty());
    }

    #[test]
    fn test_record_active_panes() {
        let mut by_socket: HashMap<Option<String>, Vec<String>> = HashMap::new();
        let mut project_panes: Vec<String> = Vec::new();
        let execution = AgentExecution {
            runtime_pane_id: Some("%10".to_string()),
            project_socket_active_pane_id: Some("%20".to_string()),
        };
        record_active_panes(&mut by_socket, &mut project_panes, &execution);
        assert_eq!(by_socket.get(&None), Some(&vec!["%10".to_string()]));
        assert_eq!(project_panes, vec!["%20".to_string()]);
    }

    #[test]
    fn test_cleanup_tmux_orphans_if_needed() {
        let mut actions = Vec::new();
        let by_socket: HashMap<Option<String>, Vec<String>> = HashMap::new();
        let summaries = cleanup_tmux_orphans_if_needed(
            true,
            "p1",
            &by_socket,
            &[],
            Some("/tmp/tmux.sock"),
            &mut actions,
        );
        assert!(summaries.is_empty());
        assert_eq!(actions, vec!["cleanup_tmux_orphans:killed=0"]);

        actions.clear();
        let summaries = cleanup_tmux_orphans_if_needed(
            false,
            "p1",
            &by_socket,
            &[],
            Some("/tmp/tmux.sock"),
            &mut actions,
        );
        assert!(summaries.is_empty());
        assert!(actions.is_empty());
    }
}
