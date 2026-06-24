//! Mirrors Python `lib/provider_execution/active_runtime/models.py`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Result of preparing an active provider start.
/// Mirrors Python `PreparedActiveStart`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedActiveStart<S> {
    pub work_dir: PathBuf,
    pub session: S,
    pub pane_id: String,
    pub backend: Value,
}

/// Minimal runtime state needed for an active poll.
/// Mirrors Python `PreparedActivePoll`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedActivePoll<R> {
    pub reader: R,
    pub backend: Value,
    pub pane_id: String,
}
