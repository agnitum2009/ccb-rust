//! Mirrors Python lib/terminal_runtime/pane_logs_runtime/paths.py
// TODO: translate from Python

use std::path::{Path, PathBuf};

/// Get log path for pane
pub fn log_path_for_pane(base_dir: &Path, pane_id: &str) -> PathBuf {
    base_dir.join(format!("{}.log", pane_id))
}

/// Get logs directory
pub fn logs_directory(base_dir: &Path) -> PathBuf {
    base_dir.to_path_buf()
}

/// Ensure logs directory exists
pub fn ensure_logs_directory(base_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(base_dir)?;
    Ok(())
}
