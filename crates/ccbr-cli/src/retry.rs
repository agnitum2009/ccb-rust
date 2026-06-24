//! Mirrors Python `lib/cli/services/retry.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrySummary {
    pub project_id: String,
    pub target: String,
    pub message_id: String,
    pub original_attempt_id: String,
    pub attempt_id: String,
    pub job_id: String,
    pub agent_name: String,
    pub status: String,
}

// TODO: align `retry_attempt` with Python once `invoke_mounted_daemon` is available.
