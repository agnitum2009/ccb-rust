//! Mirrors Python `lib/cli/services/ask_runtime/models.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AskSummary {
    pub project_id: String,
    pub submission_id: Option<String>,
    pub jobs: Vec<serde_json::Value>,
}
