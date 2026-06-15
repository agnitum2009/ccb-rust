//! Mirrors Python `lib/cli/services/start_runtime.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartSummary {
    pub project_root: String,
    pub project_id: String,
    pub started: Vec<String>,
    pub daemon_started: bool,
    pub socket_path: String,
    #[serde(default)]
    pub cleanup_summaries: Vec<serde_json::Value>,
    #[serde(default)]
    pub worktree_warnings: Vec<serde_json::Value>,
    #[serde(default)]
    pub worktree_retired: Vec<serde_json::Value>,
    #[serde(default)]
    pub maintenance_heartbeat: Option<serde_json::Value>,
}

// TODO: align `start_agents` with Python once daemon runtime + report store are available.
