//! Mirrors Python `lib/cli/services/reset_project.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResetProjectSummary {
    pub project_root: String,
    pub project_id: String,
    pub preserved_config: bool,
    pub reset_performed: bool,
    #[serde(default)]
    pub preserved_provider_histories: usize,
    #[serde(default)]
    pub preserved_session_files: usize,
    #[serde(default)]
    pub preserved_user_files: usize,
}

// TODO: align `reset_project` / `kill_project` with Python once resolver + providers ready.
