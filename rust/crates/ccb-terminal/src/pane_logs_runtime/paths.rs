//! Mirrors Python lib/terminal_runtime/pane_logs_runtime/paths.py
// TODO: translate from Python

use std::path::PathBuf;

/// Get log path for pane
pub fn log_path_for_pane(base_dir: &PathBuf, pane_id: &str) -> PathBuf {
    base_dir.join(format!("{}.log", pane_id))
}

/// Get logs directory
pub fn logs_directory(base_dir: &PathBuf) -> PathBuf {
    base_dir.clone()
}

/// Ensure logs directory exists
pub fn ensure_logs_directory(base_dir: &PathBuf) -> std::io::Result<()> {
    std::fs::create_dir_all(base_dir)?;
    Ok(())
}
