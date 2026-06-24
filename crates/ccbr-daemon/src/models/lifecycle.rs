use super::api_models::common::SCHEMA_VERSION;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcbdStartupReport {
    pub project_id: String,
    pub generated_at: String,
    pub trigger: String,
    pub status: String,
    pub actions_taken: Vec<String>,
    pub agent_results: Vec<CcbdStartupAgentResult>,
    #[serde(default)]
    pub failure_reason: Option<String>,
    pub api_version: u32,
}

impl CcbdStartupReport {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "ccbrd_startup_report",
            "api_version": self.api_version,
            "project_id": self.project_id,
            "generated_at": self.generated_at,
            "trigger": self.trigger,
            "status": self.status,
            "actions_taken": self.actions_taken,
            "agent_results": self.agent_results.iter().map(|a| a.to_record()).collect::<Vec<_>>(),
            "failure_reason": self.failure_reason,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcbdStartupAgentResult {
    pub agent_name: String,
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub pane_id: Option<String>,
}

impl CcbdStartupAgentResult {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "agent_name": self.agent_name,
            "status": self.status,
            "reason": self.reason,
            "pane_id": self.pane_id,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcbdShutdownReport {
    pub project_id: String,
    pub generated_at: String,
    pub trigger: String,
    pub status: String,
    pub forced: bool,
    pub stopped_agents: Vec<String>,
    #[serde(default)]
    pub daemon_generation: Option<u32>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub actions_taken: Vec<String>,
    #[serde(default)]
    pub cleanup_summaries: Vec<CcbdTmuxCleanupSummary>,
    #[serde(default)]
    pub runtime_snapshots: Vec<CcbdRuntimeSnapshot>,
    #[serde(default)]
    pub failure_reason: Option<String>,
    pub api_version: u32,
}

impl CcbdShutdownReport {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "ccbrd_shutdown_report",
            "api_version": self.api_version,
            "project_id": self.project_id,
            "generated_at": self.generated_at,
            "trigger": self.trigger,
            "status": self.status,
            "forced": self.forced,
            "stopped_agents": self.stopped_agents,
            "daemon_generation": self.daemon_generation,
            "reason": self.reason,
            "actions_taken": self.actions_taken,
            "cleanup_summaries": self.cleanup_summaries.iter().map(|c| c.to_record()).collect::<Vec<_>>(),
            "runtime_snapshots": self.runtime_snapshots.iter().map(|r| r.to_record()).collect::<Vec<_>>(),
            "failure_reason": self.failure_reason,
        })
    }

    pub fn summary_fields(&self) -> serde_json::Value {
        let total_killed: usize = self
            .cleanup_summaries
            .iter()
            .map(|c| c.killed_panes.len())
            .sum();
        serde_json::json!({
            "shutdown_last_at": self.generated_at,
            "shutdown_last_trigger": self.trigger,
            "shutdown_last_status": self.status,
            "shutdown_last_forced": self.forced,
            "shutdown_last_generation": self.daemon_generation,
            "shutdown_last_reason": self.reason,
            "shutdown_last_stopped_agents": self.stopped_agents,
            "shutdown_last_actions": self.actions_taken,
            "shutdown_last_cleanup_killed": total_killed,
            "shutdown_last_failure_reason": self.failure_reason,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcbdTmuxCleanupSummary {
    pub socket_name: String,
    pub killed_panes: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

impl CcbdTmuxCleanupSummary {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "socket_name": self.socket_name,
            "killed_panes": self.killed_panes,
            "errors": self.errors,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcbdRuntimeSnapshot {
    pub agent_name: String,
    pub state: String,
    pub health: String,
    #[serde(default)]
    pub pane_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub runtime_pid: Option<u32>,
}

impl CcbdRuntimeSnapshot {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "agent_name": self.agent_name,
            "state": self.state,
            "health": self.health,
            "pane_id": self.pane_id,
            "provider": self.provider,
            "runtime_pid": self.runtime_pid,
        })
    }
}

pub fn runtime_snapshots_summary(snapshots: &[CcbdRuntimeSnapshot]) -> String {
    if snapshots.is_empty() {
        return "none".into();
    }
    snapshots
        .iter()
        .map(|s| format!("{}:{}/{}", s.agent_name, s.state, s.health))
        .collect::<Vec<_>>()
        .join("; ")
}
