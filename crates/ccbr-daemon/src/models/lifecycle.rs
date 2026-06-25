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

// ---------------------------------------------------------------------------
// CcbdLifecycle
// ---------------------------------------------------------------------------

pub const LIFECYCLE_DESIRED_STATE_RUNNING: &str = "running";
pub const LIFECYCLE_DESIRED_STATE_STOPPED: &str = "stopped";

pub const LIFECYCLE_PHASE_UNMOUNTED: &str = "unmounted";
pub const LIFECYCLE_PHASE_STARTING: &str = "starting";
pub const LIFECYCLE_PHASE_MOUNTED: &str = "mounted";
pub const LIFECYCLE_PHASE_STOPPING: &str = "stopping";
pub const LIFECYCLE_PHASE_FAILED: &str = "failed";

/// Persisted lifecycle record for the CCBR daemon.
///
/// Mirrors Python `ccbd.services.lifecycle.CcbdLifecycle` and the fields
/// exercised by `test_v2_daemon_startup_wait.py`:
/// `startup_stage`, `startup_deadline_at`, `keeper_pid`, `socket_path`, and the
/// shared startup deadline used by CLI wait logic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CcbdLifecycle {
    pub project_id: String,
    pub desired_state: String,
    pub phase: String,
    pub generation: u32,
    pub phase_started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup_stage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_progress_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup_deadline_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keeper_pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_daemon_instance_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_inode: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace_epoch: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failure_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shutdown_intent: Option<String>,
}

/// Optional field updates for `CcbdLifecycle::with_updates` / `with_phase`.
/// A field is left unchanged when its update value is `None`. An
/// `Option<Option<T>>` value of `Some(None)` clears the field, while
/// `Some(Some(v))` sets it.
#[derive(Debug, Clone, Default)]
pub struct CcbdLifecycleUpdates {
    pub desired_state: Option<String>,
    pub generation: Option<u32>,
    pub startup_id: Option<Option<String>>,
    pub startup_stage: Option<Option<String>>,
    pub last_progress_at: Option<Option<String>>,
    pub startup_deadline_at: Option<Option<String>>,
    pub keeper_pid: Option<Option<u32>>,
    pub owner_pid: Option<Option<u32>>,
    pub owner_daemon_instance_id: Option<Option<String>>,
    pub config_signature: Option<Option<String>>,
    pub socket_path: Option<Option<String>>,
    pub socket_inode: Option<Option<u64>>,
    pub namespace_epoch: Option<Option<u32>>,
    pub last_failure_reason: Option<Option<String>>,
    pub shutdown_intent: Option<Option<String>>,
}

impl CcbdLifecycle {
    pub fn with_phase(
        mut self,
        phase: impl Into<String>,
        occurred_at: impl Into<String>,
        updates: CcbdLifecycleUpdates,
    ) -> Self {
        self.phase = phase.into();
        self.phase_started_at = occurred_at.into();
        self.apply_updates(updates)
    }

    pub fn with_updates(self, updates: CcbdLifecycleUpdates) -> Self {
        self.apply_updates(updates)
    }

    fn apply_updates(mut self, updates: CcbdLifecycleUpdates) -> Self {
        if let Some(v) = updates.desired_state {
            self.desired_state = v;
        }
        if let Some(v) = updates.generation {
            self.generation = v;
        }
        if let Some(v) = updates.startup_id {
            self.startup_id = v;
        }
        if let Some(v) = updates.startup_stage {
            self.startup_stage = v;
        }
        if let Some(v) = updates.last_progress_at {
            self.last_progress_at = v;
        }
        if let Some(v) = updates.startup_deadline_at {
            self.startup_deadline_at = v;
        }
        if let Some(v) = updates.keeper_pid {
            self.keeper_pid = v;
        }
        if let Some(v) = updates.owner_pid {
            self.owner_pid = v;
        }
        if let Some(v) = updates.owner_daemon_instance_id {
            self.owner_daemon_instance_id = v;
        }
        if let Some(v) = updates.config_signature {
            self.config_signature = v;
        }
        if let Some(v) = updates.socket_path {
            self.socket_path = v;
        }
        if let Some(v) = updates.socket_inode {
            self.socket_inode = v;
        }
        if let Some(v) = updates.namespace_epoch {
            self.namespace_epoch = v;
        }
        if let Some(v) = updates.last_failure_reason {
            self.last_failure_reason = v;
        }
        if let Some(v) = updates.shutdown_intent {
            self.shutdown_intent = v;
        }
        self
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "ccbd_lifecycle",
            "project_id": self.project_id,
            "desired_state": self.desired_state,
            "phase": self.phase,
            "generation": self.generation,
            "phase_started_at": self.phase_started_at,
            "startup_id": self.startup_id,
            "startup_stage": self.startup_stage,
            "last_progress_at": self.last_progress_at,
            "startup_deadline_at": self.startup_deadline_at,
            "keeper_pid": self.keeper_pid,
            "owner_pid": self.owner_pid,
            "owner_daemon_instance_id": self.owner_daemon_instance_id,
            "config_signature": self.config_signature,
            "socket_path": self.socket_path,
            "socket_inode": self.socket_inode,
            "namespace_epoch": self.namespace_epoch,
            "last_failure_reason": self.last_failure_reason,
            "shutdown_intent": self.shutdown_intent,
        })
    }

    pub fn from_record(value: serde_json::Value) -> Option<Self> {
        serde_json::from_value(value).ok()
    }

    /// Returns true when the shared startup deadline has passed.
    pub fn is_startup_deadline_expired(&self, now: &chrono::DateTime<chrono::Utc>) -> bool {
        match self.startup_deadline_at.as_deref() {
            Some(text) => match chrono::DateTime::parse_from_rfc3339(text) {
                Ok(dt) => now >= &dt.with_timezone(now.offset()),
                Err(_) => false,
            },
            None => false,
        }
    }
}

/// Build a validated `CcbdLifecycle` record.
pub fn build_lifecycle(
    project_id: impl Into<String>,
    occurred_at: impl Into<String>,
    desired_state: impl Into<String>,
    phase: impl Into<String>,
    generation: u32,
    updates: CcbdLifecycleUpdates,
) -> CcbdLifecycle {
    CcbdLifecycle {
        project_id: project_id.into(),
        desired_state: desired_state.into(),
        phase: phase.into(),
        generation,
        phase_started_at: occurred_at.into(),
        startup_id: updates.startup_id.unwrap_or(None),
        startup_stage: updates.startup_stage.unwrap_or(None),
        last_progress_at: updates.last_progress_at.unwrap_or(None),
        startup_deadline_at: updates.startup_deadline_at.unwrap_or(None),
        keeper_pid: updates.keeper_pid.unwrap_or(None),
        owner_pid: updates.owner_pid.unwrap_or(None),
        owner_daemon_instance_id: updates.owner_daemon_instance_id.unwrap_or(None),
        config_signature: updates.config_signature.unwrap_or(None),
        socket_path: updates.socket_path.unwrap_or(None),
        socket_inode: updates.socket_inode.unwrap_or(None),
        namespace_epoch: updates.namespace_epoch.unwrap_or(None),
        last_failure_reason: updates.last_failure_reason.unwrap_or(None),
        shutdown_intent: updates.shutdown_intent.unwrap_or(None),
    }
}
