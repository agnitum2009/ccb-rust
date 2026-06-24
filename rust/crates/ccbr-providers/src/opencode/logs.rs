use std::path::{Path, PathBuf};

/// Find the most recently modified OpenCode log file under `root`.
pub fn latest_opencode_log_file(root: &Path) -> Option<PathBuf> {
    if !root.exists() {
        return None;
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("log") && path.is_file() {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    if paths.is_empty() {
        return None;
    }
    paths.sort_by(|a, b| {
        let a_mtime = a.metadata().and_then(|m| m.modified()).ok();
        let b_mtime = b.metadata().and_then(|m| m.modified()).ok();
        b_mtime.cmp(&a_mtime)
    });
    Some(paths.swap_remove(0))
}

/// Check whether a log line indicates a cancel for the given session id.
pub fn is_cancel_log_line(line: &str, session_id: &str) -> bool {
    if line.is_empty() {
        return false;
    }
    let normalized = session_id.trim();
    if normalized.is_empty() {
        return false;
    }
    line.contains(&format!("sessionID={normalized} cancel"))
        || line.contains(&format!("path=/session/{normalized}/abort"))
}

/// Parse a log timestamp into seconds since epoch.
pub fn parse_opencode_log_epoch_s(line: &str) -> Option<f64> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let timestamp = parts[1];
    let dt = chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%dT%H:%M:%S").ok()?;
    Some(dt.and_utc().timestamp() as f64)
}
