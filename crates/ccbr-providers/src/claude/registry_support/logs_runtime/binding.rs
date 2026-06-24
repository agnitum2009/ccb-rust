//! Mirrors Python `lib/provider_backends/claude/registry_support/logs_runtime/binding.py`.

use std::path::{Path, PathBuf};

use super::discovery::{
    extract_session_id_from_start_cmd, find_log_for_session_id, scan_latest_log_for_work_dir,
};
use super::indexing::parse_sessions_index;
use crate::claude::session::ClaudeProjectSession;

/// Decide whether a candidate log should overwrite the current binding.
pub fn should_overwrite_binding(current: Option<&Path>, candidate: &Path) -> bool {
    match current {
        None => true,
        Some(current) if !current.exists() => true,
        Some(current) => {
            let current_mtime = current.metadata().and_then(|m| m.modified()).ok();
            let candidate_mtime = candidate.metadata().and_then(|m| m.modified()).ok();
            match (current_mtime, candidate_mtime) {
                (_, None) => false,
                (None, Some(_)) => true,
                (Some(c), Some(n)) => n > c,
            }
        }
    }
}

/// Refresh the Claude log binding for a session.
/// Mirrors Python `refresh_claude_log_binding`.
pub fn refresh_claude_log_binding(
    session: &mut ClaudeProjectSession,
    root: &Path,
    scan_limit: usize,
    force_scan: bool,
) -> bool {
    let current_log = current_log_path(session);
    let (intended_log, intended_sid) = intended_log_binding(session, root);
    if binding_exists(intended_log.as_deref()) {
        return refresh_from_candidate(
            session,
            current_log.as_deref(),
            intended_log.as_deref().unwrap(),
            intended_sid.as_deref(),
        );
    }

    let index_log = indexed_log_binding(session, root);
    if binding_exists(index_log.as_deref()) {
        let updated = refresh_from_candidate(
            session,
            current_log.as_deref(),
            index_log.as_deref().unwrap(),
            index_log
                .as_deref()
                .unwrap()
                .file_stem()
                .and_then(|s| s.to_str()),
        );
        if updated || !force_scan {
            return updated;
        }
    }

    if !need_scan(force_scan, intended_log.as_deref(), index_log.as_deref()) {
        return false;
    }

    let (candidate_log, candidate_sid) = scanned_log_binding(session, root, scan_limit);
    if !binding_exists(candidate_log.as_deref()) {
        return false;
    }
    refresh_from_candidate(
        session,
        current_log.as_deref(),
        candidate_log.as_deref().unwrap(),
        candidate_sid.as_deref(),
    )
}

fn current_log_path(session: &ClaudeProjectSession) -> Option<PathBuf> {
    session
        .data
        .get("claude_session_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(expand_tilde)
}

fn start_cmd(session: &ClaudeProjectSession) -> String {
    session
        .data
        .get("claude_start_cmd")
        .or_else(|| session.data.get("start_cmd"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn intended_log_binding(
    session: &ClaudeProjectSession,
    root: &Path,
) -> (Option<PathBuf>, Option<String>) {
    let Some(intended_sid) = extract_session_id_from_start_cmd(&start_cmd(session)) else {
        return (None, None);
    };
    let log = find_log_for_session_id(&intended_sid, root);
    (log, Some(intended_sid))
}

fn indexed_log_binding(session: &ClaudeProjectSession, root: &Path) -> Option<PathBuf> {
    let work_dir = session
        .data
        .get("work_dir")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            session
                .session_file
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default()
        });
    parse_sessions_index(&work_dir, root)
}

fn scanned_log_binding(
    session: &ClaudeProjectSession,
    root: &Path,
    scan_limit: usize,
) -> (Option<PathBuf>, Option<String>) {
    let work_dir = session
        .data
        .get("work_dir")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            session
                .session_file
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default()
        });
    scan_latest_log_for_work_dir(&work_dir, root, scan_limit)
}

fn binding_exists(candidate: Option<&Path>) -> bool {
    candidate.map(Path::exists).unwrap_or(false)
}

fn need_scan(force_scan: bool, intended_log: Option<&Path>, index_log: Option<&Path>) -> bool {
    force_scan || intended_log.is_none() && index_log.is_none()
}

fn refresh_from_candidate(
    session: &mut ClaudeProjectSession,
    current_log: Option<&Path>,
    candidate_log: &Path,
    candidate_sid: Option<&str>,
) -> bool {
    if !should_update_session_binding(session, current_log, candidate_log, candidate_sid) {
        return false;
    }
    session.update_claude_binding(Some(candidate_log), candidate_sid);
    true
}

fn should_update_session_binding(
    session: &ClaudeProjectSession,
    current_log: Option<&Path>,
    candidate_log: &Path,
    candidate_sid: Option<&str>,
) -> bool {
    if should_overwrite_binding(current_log, candidate_log) {
        return true;
    }
    let current_id = session
        .data
        .get("claude_session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    candidate_sid.map(|s| s != current_id).unwrap_or(false)
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use filetime::FileTime;
    use std::collections::HashMap;
    use std::time::SystemTime;
    use tempfile::TempDir;

    fn fake_session(
        session_path: Option<&str>,
        session_id: &str,
        work_dir: &Path,
        data: HashMap<String, serde_json::Value>,
    ) -> ClaudeProjectSession {
        let tmp = TempDir::new().unwrap();
        let session_file = tmp.path().join(".claude-session");
        std::fs::write(&session_file, "{}").unwrap();
        let mut data = data;
        if let Some(path) = session_path {
            data.insert(
                "claude_session_path".to_string(),
                serde_json::Value::String(path.to_string()),
            );
        }
        if !session_id.is_empty() {
            data.insert(
                "claude_session_id".to_string(),
                serde_json::Value::String(session_id.to_string()),
            );
        }
        data.insert(
            "work_dir".to_string(),
            serde_json::Value::String(work_dir.to_string_lossy().to_string()),
        );
        ClaudeProjectSession { session_file, data }
    }

    #[test]
    fn test_should_overwrite_binding_prefers_newer_mtime() {
        let tmp = TempDir::new().unwrap();
        let current = tmp.path().join("current.jsonl");
        let candidate = tmp.path().join("candidate.jsonl");
        std::fs::write(&current, "").unwrap();
        std::fs::write(&candidate, "").unwrap();
        let now = SystemTime::now();
        filetime::set_file_mtime(&current, FileTime::from_system_time(now)).unwrap();
        filetime::set_file_mtime(
            &candidate,
            FileTime::from_system_time(now + std::time::Duration::from_secs(20)),
        )
        .unwrap();

        assert!(should_overwrite_binding(Some(&current), &candidate));
        assert!(!should_overwrite_binding(Some(&candidate), &current));
    }

    #[test]
    fn test_refresh_prefers_intended_resume_log() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("repo");
        std::fs::create_dir(&work_dir).unwrap();
        let sid = "12345678-1234-1234-1234-1234567890ab";
        let current = tmp.path().join("current.jsonl");
        let intended = tmp.path().join(format!("{sid}.jsonl"));
        std::fs::write(&current, "").unwrap();
        std::fs::write(&intended, "").unwrap();
        let now = SystemTime::now();
        filetime::set_file_mtime(&current, FileTime::from_system_time(now)).unwrap();
        filetime::set_file_mtime(
            &intended,
            FileTime::from_system_time(now + std::time::Duration::from_secs(20)),
        )
        .unwrap();

        let mut data = HashMap::new();
        data.insert(
            "start_cmd".to_string(),
            serde_json::Value::String(format!("claude --resume {sid}")),
        );
        let mut session = fake_session(Some(current.to_str().unwrap()), "old-id", &work_dir, data);

        let updated = refresh_claude_log_binding(&mut session, tmp.path(), 10, false);
        assert!(updated);
        assert_eq!(
            session.data.get("claude_session_path").unwrap().as_str(),
            Some(intended.to_str().unwrap())
        );
    }

    #[test]
    fn test_refresh_respects_index_without_forced_scan() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("repo");
        std::fs::create_dir(&work_dir).unwrap();
        let current = tmp.path().join("current.jsonl");
        let index_log = tmp.path().join("index.jsonl");
        std::fs::write(&index_log, "").unwrap();
        std::fs::write(&current, "").unwrap();
        let now = SystemTime::now();
        filetime::set_file_mtime(
            &index_log,
            FileTime::from_system_time(now - std::time::Duration::from_secs(20)),
        )
        .unwrap();
        filetime::set_file_mtime(&current, FileTime::from_system_time(now)).unwrap();

        let mut session = fake_session(
            Some(current.to_str().unwrap()),
            "index",
            &work_dir,
            HashMap::new(),
        );
        let updated = refresh_claude_log_binding(&mut session, tmp.path(), 10, false);
        assert!(!updated);
        assert_eq!(
            session.data.get("claude_session_path").unwrap().as_str(),
            Some(current.to_str().unwrap())
        );
    }
}
