use super::common::{JobStatus, TargetKind, SCHEMA_VERSION};
use super::messages::MessageEnvelope;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: String,
    pub submission_id: Option<String>,
    pub agent_name: String,
    pub provider: String,
    pub request: MessageEnvelope,
    pub status: JobStatus,
    pub terminal_decision: Option<serde_json::Value>,
    pub cancel_requested_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub target_kind: TargetKind,
    #[serde(default)]
    pub target_name: String,
}

impl JobRecord {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "job_record",
            "job_id": self.job_id,
            "submission_id": self.submission_id,
            "agent_name": self.agent_name,
            "target_kind": self.target_kind,
            "target_name": self.target_name,
            "provider": self.provider,
            "request": self.request.to_record(),
            "status": self.status,
            "terminal_decision": self.terminal_decision,
            "cancel_requested_at": self.cancel_requested_at,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "workspace_path": self.workspace_path,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionRecord {
    pub submission_id: String,
    pub project_id: String,
    pub from_actor: String,
    pub target_scope: String,
    pub task_id: Option<String>,
    #[serde(default)]
    pub job_ids: Vec<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEvent {
    pub event_id: String,
    pub job_id: String,
    pub agent_name: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: serde_json::Value,
    pub timestamp: String,
    #[serde(default)]
    pub target_kind: TargetKind,
    #[serde(default)]
    pub target_name: String,
}
