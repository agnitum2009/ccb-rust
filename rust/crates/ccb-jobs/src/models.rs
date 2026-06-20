use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Delivery scope for a message envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum DeliveryScope {
    #[default]
    Agent,
    Group,
    Broadcast,
}

/// A message envelope submitted to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub project_id: String,
    pub to_agent: String,
    pub from_actor: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    pub message_type: String,
    pub delivery_scope: DeliveryScope,
    #[serde(default)]
    pub silence_on_success: bool,
    #[serde(default)]
    pub route_options: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_artifact: Option<Value>,
}

/// Job status enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum JobStatus {
    #[default]
    Accepted,
    Running,
    Completed,
    Failed,
    Incomplete,
    Cancelled,
}

/// Target kind for job routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum TargetKind {
    #[default]
    Agent,
    Group,
}

/// A job record persisted per target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission_id: Option<String>,
    #[serde(default)]
    pub agent_name: String,
    pub provider: String,
    pub request: MessageEnvelope,
    pub status: JobStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_decision: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_requested_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub target_kind: TargetKind,
    #[serde(default)]
    pub target_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_instance: Option<String>,
    #[serde(default)]
    pub provider_options: Value,
}

/// A job event persisted per target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEvent {
    pub event_id: String,
    pub job_id: String,
    #[serde(default)]
    pub agent_name: String,
    #[serde(default)]
    pub target_kind: TargetKind,
    #[serde(default)]
    pub target_name: String,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub payload: Value,
    pub timestamp: String,
}

/// A submission record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionRecord {
    pub submission_id: String,
    pub project_id: String,
    pub from_actor: String,
    pub target_scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default)]
    pub job_ids: Vec<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}
