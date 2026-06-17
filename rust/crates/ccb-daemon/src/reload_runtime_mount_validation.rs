//! Mirrors Python `lib/ccbd/reload_runtime_mount_validation.py`.
//! 1:1 file alignment stub.

use std::collections::{HashMap, HashSet};

/// Check if runtime mount is blocked and return reason
pub fn blocked_mount_reason(
    graph: &ServiceGraph,
    namespace: Option<&NamespaceState>,
    agent_panes: &HashMap<String, String>,
    preserved_agents: &[String],
) -> Option<(String, String)> {
    if namespace.is_none() {
        return Some((
            "namespace_unavailable".to_string(),
            "runtime mounts require current namespace scope".to_string(),
        ));
    }

    let namespace = namespace.unwrap();

    if let Some(reason) = namespace_scope_reason(graph, namespace) {
        return Some(reason);
    }

    if let Some(reason) = agent_scope_reason(graph, agent_panes, preserved_agents) {
        return Some(reason);
    }

    None
}

/// Get existing runtime agents that would block new mount
pub fn existing_runtime_agents(
    before_new: &HashMap<String, Option<AgentRecord>>,
    requested_agents: &[String],
) -> Vec<String> {
    requested_agents
        .iter()
        .filter(|agent| blocks_new_runtime_mount(before_new.get(*agent).and_then(|o| o.as_ref())))
        .cloned()
        .collect()
}

/// Check if record blocks new runtime mount
fn blocks_new_runtime_mount(record: Option<&AgentRecord>) -> bool {
    match record {
        None => false,
        Some(rec) => !is_retired_runtime_residue(rec),
    }
}

/// Check if record is retired runtime residue
fn is_retired_runtime_residue(record: &AgentRecord) -> bool {
    if text(record.state.as_deref()).as_str() != "stopped" {
        return false;
    }
    if text(record.health.as_deref()).as_str() != "stopped" {
        return false;
    }
    if text(record.desired_state.as_deref()).as_str() != "stopped" {
        return false;
    }
    if text(record.reconcile_state.as_deref()).as_str() != "stopped" {
        return false;
    }

    let live_authority_fields = [
        "pid",
        "runtime_ref",
        "session_ref",
        "socket_path",
        "runtime_pid",
        "pane_id",
        "active_pane_id",
        "mount_attempt_id",
    ];

    !live_authority_fields
        .iter()
        .any(|field| has_value(record.get_field(field)))
}

/// Check if value has meaningful content
fn has_value(value: Option<&String>) -> bool {
    match value {
        None => false,
        Some(v) => !v.trim().is_empty(),
    }
}

/// Convert value to trimmed text
fn text(value: Option<&str>) -> String {
    value.map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Check namespace scope validity
fn namespace_scope_reason(
    graph: &ServiceGraph,
    namespace: &NamespaceState,
) -> Option<(String, String)> {
    let graph_project_id = graph_project_id(graph);
    let namespace_project_id = clean_text(namespace.project_id.as_deref());

    if let (Some(gp_id), Some(np_id)) = (graph_project_id, namespace_project_id) {
        if gp_id != np_id {
            return Some((
                "namespace_project_mismatch".to_string(),
                "namespace project_id does not match target service graph".to_string(),
            ));
        }
    }

    if !namespace.ui_attachable {
        return Some((
            "namespace_not_attachable".to_string(),
            "runtime mounts require an attachable namespace".to_string(),
        ));
    }

    if clean_text(namespace.tmux_socket_path.as_deref()).is_none_or(|s| s.is_empty()) {
        return Some((
            "namespace_scope_missing".to_string(),
            "namespace tmux socket path is missing".to_string(),
        ));
    }

    if clean_text(namespace.tmux_session_name.as_deref()).is_none_or(|s| s.is_empty()) {
        return Some((
            "namespace_scope_missing".to_string(),
            "namespace tmux session name is missing".to_string(),
        ));
    }

    if namespace.namespace_epoch.is_none() {
        return Some((
            "namespace_scope_missing".to_string(),
            "namespace epoch is missing".to_string(),
        ));
    }

    None
}

/// Check agent scope validity
fn agent_scope_reason(
    graph: &ServiceGraph,
    agent_panes: &HashMap<String, String>,
    preserved_agents: &[String],
) -> Option<(String, String)> {
    let agent_panes_set: HashSet<&String> = agent_panes.keys().collect();
    let preserved_set: HashSet<&String> = preserved_agents.iter().collect();
    let overlap: Vec<_> = agent_panes_set
        .intersection(&preserved_set)
        .map(|s| (*s).clone())
        .collect();

    if !overlap.is_empty() {
        let overlap_str = overlap.to_vec().join(",");
        return Some((
            "preserved_agent_mount_blocked".to_string(),
            format!(
                "runtime mounts cannot target preserved agents: {}",
                overlap_str
            ),
        ));
    }

    let missing_panes: Vec<_> = agent_panes
        .iter()
        .filter(|(_, pane)| !valid_pane_id(pane))
        .map(|(agent, _)| agent.clone())
        .collect();

    if !missing_panes.is_empty() {
        let missing_str = missing_panes.to_vec().join(",");
        return Some((
            "agent_pane_missing".to_string(),
            format!("new agent pane evidence is missing: {}", missing_str),
        ));
    }

    let configured = configured_agents(graph);
    let unknown: Vec<_> = agent_panes
        .keys()
        .filter(|agent| !configured.contains(*agent))
        .cloned()
        .collect();

    if !unknown.is_empty() {
        let unknown_str = unknown.to_vec().join(",");
        return Some((
            "agent_not_configured".to_string(),
            format!("new agent is not in target config: {}", unknown_str),
        ));
    }

    None
}

/// Get configured agents from graph
fn configured_agents(graph: &ServiceGraph) -> HashSet<String> {
    graph.config.agents.keys().cloned().collect()
}

/// Get project ID from graph
fn graph_project_id(graph: &ServiceGraph) -> Option<String> {
    graph.runtime_supervisor.as_ref()?.project_id.clone()
}

/// Clean text value
fn clean_text(value: Option<&str>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Validate pane ID format
fn valid_pane_id(pane: &str) -> bool {
    !pane.trim().is_empty()
}

// Type definitions

#[derive(Debug, Clone)]
pub struct ServiceGraph {
    pub config: GraphConfig,
    pub runtime_supervisor: Option<RuntimeSupervisor>,
}

#[derive(Debug, Clone)]
pub struct GraphConfig {
    pub agents: HashMap<String, AgentConfig>,
}

#[derive(Debug, Clone)]
pub struct AgentConfig {}

#[derive(Debug, Clone)]
pub struct RuntimeSupervisor {
    pub project_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NamespaceState {
    pub project_id: Option<String>,
    pub ui_attachable: bool,
    pub tmux_socket_path: Option<String>,
    pub tmux_session_name: Option<String>,
    pub namespace_epoch: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AgentRecord {
    pub state: Option<String>,
    pub health: Option<String>,
    pub desired_state: Option<String>,
    pub reconcile_state: Option<String>,
    // Additional fields accessed via get_field
    pub fields: HashMap<String, String>,
}

impl Default for AgentRecord {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRecord {
    pub fn get_field(&self, field: &str) -> Option<&String> {
        self.fields.get(field)
    }

    pub fn new() -> Self {
        Self {
            state: None,
            health: None,
            desired_state: None,
            reconcile_state: None,
            fields: HashMap::new(),
        }
    }
}
