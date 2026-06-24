use super::api_models::common::SCHEMA_VERSION;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcbdRestoreEntry {
    pub job_id: String,
    pub agent_name: String,
    pub provider: String,
    pub status: String,
    pub reason: String,
    pub resume_capable: bool,
    #[serde(default)]
    pub pending_items_count: u32,
}

impl CcbdRestoreEntry {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "job_id": self.job_id,
            "agent_name": self.agent_name,
            "provider": self.provider,
            "status": self.status,
            "reason": self.reason,
            "resume_capable": self.resume_capable,
            "pending_items_count": self.pending_items_count,
        })
    }

    pub fn summary_token(&self) -> String {
        let pending = if self.pending_items_count > 0 {
            format!(",pending_items={}", self.pending_items_count)
        } else {
            String::new()
        };
        format!(
            "{}:{}:{}({}{})",
            self.agent_name, self.provider, self.status, self.reason, pending
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcbdRestoreReport {
    pub project_id: String,
    pub generated_at: String,
    pub running_job_count: u32,
    pub restored_execution_count: u32,
    pub replay_pending_count: u32,
    pub terminal_pending_count: u32,
    pub abandoned_execution_count: u32,
    pub already_active_count: u32,
    #[serde(default)]
    pub entries: Vec<CcbdRestoreEntry>,
    pub api_version: u32,
}

impl CcbdRestoreReport {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "ccbd_restore_report",
            "api_version": self.api_version,
            "project_id": self.project_id,
            "generated_at": self.generated_at,
            "running_job_count": self.running_job_count,
            "restored_execution_count": self.restored_execution_count,
            "replay_pending_count": self.replay_pending_count,
            "terminal_pending_count": self.terminal_pending_count,
            "abandoned_execution_count": self.abandoned_execution_count,
            "already_active_count": self.already_active_count,
            "entries": self.entries.iter().map(|e| e.to_record()).collect::<Vec<_>>(),
        })
    }

    pub fn summary_fields(&self) -> serde_json::Value {
        let results_text = if self.entries.is_empty() {
            "none".into()
        } else {
            self.entries
                .iter()
                .map(|e| e.summary_token())
                .collect::<Vec<_>>()
                .join("; ")
        };
        serde_json::json!({
            "last_restore_at": self.generated_at,
            "last_restore_running_job_count": self.running_job_count,
            "last_restore_restored_execution_count": self.restored_execution_count,
            "last_restore_replay_pending_count": self.replay_pending_count,
            "last_restore_terminal_pending_count": self.terminal_pending_count,
            "last_restore_abandoned_execution_count": self.abandoned_execution_count,
            "last_restore_already_active_count": self.already_active_count,
            "last_restore_results_text": results_text,
        })
    }
}
