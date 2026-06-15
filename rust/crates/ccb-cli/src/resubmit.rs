//! Mirrors Python `lib/cli/services/resubmit.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResubmitSummary {
    pub project_id: String,
    pub original_message_id: String,
    pub message_id: String,
    pub submission_id: Option<String>,
    pub jobs: Vec<serde_json::Value>,
}

// TODO: align `resubmit_message` with Python once `invoke_mounted_daemon` is available.
