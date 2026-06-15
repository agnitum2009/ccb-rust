//! Mirrors Python `lib/cli/services/start_foreground.py`.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForegroundAttachSummary {
    pub project_id: String,
    pub tmux_socket_path: String,
    pub tmux_session_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForegroundAttachError(pub String);

impl fmt::Display for ForegroundAttachError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ForegroundAttachError {}

// TODO: align `attach_started_project_namespace` with Python once daemon client ready.
