//! Mirrors Python `lib/cli/output.py`.
//!
//! CLI output utilities: exit codes, atomic file writes, message normalization.
//! 1:1 alignment with Python functions.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tempfile::NamedTempFile;

/// Successful exit code.
pub const EXIT_OK: i32 = 0;
/// General error exit code.
pub const EXIT_ERROR: i32 = 1;
/// No reply received exit code.
pub const EXIT_NO_REPLY: i32 = 2;

/// Atomically write text to a file using a temporary file + rename.
///
/// Mirrors Python `atomic_write_text`.
pub fn atomic_write_text(path: &Path, content: &str) -> Result<()> {
    ensure_parent(path)?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut tmp = NamedTempFile::new_in(dir)
        .with_context(|| format!("creating temp file in {}", dir.display()))?;
    tmp.write_all(content.as_bytes())
        .with_context(|| "writing content to temp file")?;
    tmp.persist(path)
        .with_context(|| format!("persisting temp file to {}", path.display()))?;
    Ok(())
}

/// Normalize message parts into a single string.
///
/// Joins non-empty trimmed parts with newlines. Mirrors Python `normalize_message_parts`.
pub fn normalize_message_parts(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ensure the parent directory of a path exists.
pub fn ensure_parent(path: &Path) -> Result<PathBuf> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating parent dir {}", parent.display()))?;
        }
        Ok(parent.to_path_buf())
    } else {
        Ok(PathBuf::from("."))
    }
}
