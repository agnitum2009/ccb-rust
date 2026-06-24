//! Mirrors Python `lib/cli/services/logs.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogExcerpt {
    pub source: String,
    pub path: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogsSummary {
    pub project_id: String,
    pub agent_name: String,
    pub provider: String,
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub entries: Vec<LogExcerpt>,
}

// TODO: align `agent_logs` with Python once config loader / agent store are available.
