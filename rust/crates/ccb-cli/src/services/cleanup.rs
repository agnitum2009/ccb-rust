//! Mirrors Python `lib/cli/services/cleanup.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CleanupAction {
    pub provider: String,
    pub kind: String,
    pub path: String,
    pub bytes_removed: u64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CleanupSkipped {
    pub provider: String,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CleanupSummary {
    pub project_root: String,
    pub project_id: String,
    pub status: String,
    pub deleted_bytes: u64,
    pub deleted_count: usize,
    pub skipped_count: usize,
    #[serde(default)]
    pub actions: Vec<CleanupAction>,
    #[serde(default)]
    pub skipped: Vec<CleanupSkipped>,
}

// TODO: align `cleanup_project_storage` with Python once daemon inspection + state store ready.
