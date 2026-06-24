use super::common::{JobStatus, TargetKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedJobReceipt {
    pub job_id: String,
    pub agent_name: String,
    pub status: JobStatus,
    pub accepted_at: String,
    #[serde(default)]
    pub target_kind: TargetKind,
    #[serde(default)]
    pub target_name: String,
    #[serde(default)]
    pub provider_instance: Option<String>,
}

impl AcceptedJobReceipt {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "job_id": self.job_id,
            "agent_name": self.agent_name,
            "target_kind": self.target_kind,
            "target_name": self.target_name,
            "provider_instance": self.provider_instance,
            "status": self.status,
            "accepted_at": self.accepted_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitReceipt {
    pub accepted_at: String,
    pub jobs: Vec<AcceptedJobReceipt>,
    pub submission_id: Option<String>,
}

impl SubmitReceipt {
    pub fn to_record(&self) -> serde_json::Value {
        if self.submission_id.is_none() && self.jobs.len() == 1 {
            return self.jobs[0].to_record();
        }
        serde_json::json!({
            "submission_id": self.submission_id,
            "accepted_at": self.accepted_at,
            "jobs": self.jobs.iter().map(|j| j.to_record()).collect::<Vec<_>>(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelReceipt {
    pub job_id: String,
    pub agent_name: String,
    pub status: JobStatus,
    pub cancelled_at: String,
    #[serde(default)]
    pub target_kind: TargetKind,
    #[serde(default)]
    pub target_name: String,
}

impl CancelReceipt {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "job_id": self.job_id,
            "agent_name": self.agent_name,
            "status": self.status,
            "cancelled_at": self.cancelled_at,
        })
    }
}
