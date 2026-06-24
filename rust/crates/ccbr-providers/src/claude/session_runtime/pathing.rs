//! Mirrors Python `lib/provider_backends/claude/session_runtime/pathing.py`.

use std::path::Path;

use serde_json::{Map, Value};

pub use crate::claude::registry_support::pathing::{
    ensure_claude_session_work_dir_fields as ensure_work_dir_fields,
    infer_work_dir_from_session_file, normalize_project_path,
};

/// Current timestamp string for session updates.
pub fn now_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Load a JSON object from a path, ignoring invalid data.
pub fn read_json(path: &Path) -> Option<Map<String, Value>> {
    let raw = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    value.as_object().cloned()
}
