//! Mirrors Python `lib/cli/services/wait_runtime/models.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaitSummary {
    pub wait_status: String,
    pub project_id: String,
    pub mode: String,
    pub target: String,
    pub resolved_kind: String,
    pub expected_count: usize,
    pub received_count: usize,
    pub terminal_count: usize,
    pub notice_count: usize,
    pub waited_s: f64,
    pub replies: Vec<serde_json::Value>,
}
