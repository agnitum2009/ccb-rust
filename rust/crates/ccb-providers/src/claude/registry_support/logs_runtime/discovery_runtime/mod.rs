//! Mirrors Python `lib/provider_backends/claude/registry_support/logs_runtime/discovery_runtime/`.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};

use regex::Regex;
use walkdir::WalkDir;

use super::meta::read_session_meta;
use crate::claude::registry_support::pathing::path_within;

/// UUID pattern used to extract a session id from a start command.
pub fn extract_session_id_from_start_cmd(start_cmd: &str) -> Option<String> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
            .unwrap()
    });
    re.find(start_cmd).map(|m| m.as_str().to_string())
}

fn mtime_of(path: &Path) -> i64 {
    path.metadata()
        .and_then(|m| m.modified())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        })
        .unwrap_or(-1)
}

/// Find the newest log file matching a session id under `root`.
pub fn find_log_for_session_id(session_id: &str, root: &Path) -> Option<PathBuf> {
    let root = expand_tilde(root);
    if session_id.is_empty() || !root.exists() {
        return None;
    }
    let mut latest: Option<PathBuf> = None;
    let mut latest_mtime: i64 = -1;
    let sid_lower = session_id.to_lowercase();
    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name()?.to_string_lossy();
        if !name.to_lowercase().contains(&sid_lower) {
            continue;
        }
        let mtime = mtime_of(path);
        if mtime >= latest_mtime {
            latest = Some(path.to_path_buf());
            latest_mtime = mtime;
        }
    }
    latest
}

/// Scan the newest `scan_limit` log files under `root` and return the first whose
/// metadata cwd is within `work_dir`.
pub fn scan_latest_log_for_work_dir(
    work_dir: &Path,
    root: &Path,
    scan_limit: usize,
) -> (Option<PathBuf>, Option<String>) {
    let root = expand_tilde(root);
    if !root.exists() {
        return (None, None);
    }
    let candidates = candidate_logs(&root, scan_limit);
    let work_dir_str = work_dir.to_string_lossy().to_string();
    for candidate in candidates {
        let (cwd, sid, is_sidechain) = read_session_meta(&candidate);
        if is_sidechain == Some(true) {
            continue;
        }
        if let Some(cwd) = cwd {
            if path_within(&cwd, &work_dir_str) {
                return (Some(candidate), sid);
            }
        }
    }
    (None, None)
}

fn candidate_logs(root: &Path, scan_limit: usize) -> Vec<PathBuf> {
    let mut heap: BinaryHeap<Reverse<(i64, String)>> = BinaryHeap::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        if name.starts_with('.') || !name.ends_with(".jsonl") {
            continue;
        }
        let mtime = mtime_of(path);
        let item = Reverse((mtime, path.to_string_lossy().to_string()));
        if heap.len() < scan_limit {
            heap.push(item);
        } else if let Some(&Reverse((min_mtime, _))) = heap.peek() {
            if mtime > min_mtime {
                heap.pop();
                heap.push(item);
            }
        }
    }
    let mut sorted: Vec<_> = heap.into_sorted_vec();
    sorted.reverse();
    sorted
        .into_iter()
        .map(|Reverse((_, path_str))| PathBuf::from(path_str))
        .collect()
}

fn expand_tilde(path: &Path) -> PathBuf {
    if let Some(rest) = path.to_str().and_then(|s| s.strip_prefix('~')) {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use filetime::FileTime;
    use std::time::SystemTime;
    use tempfile::TempDir;

    #[test]
    fn test_extract_session_id_from_start_cmd_finds_uuid() {
        let sid = "12345678-1234-1234-1234-1234567890ab";
        assert_eq!(
            extract_session_id_from_start_cmd(&format!("claude --resume {sid}")),
            Some(sid.to_string())
        );
        assert_eq!(extract_session_id_from_start_cmd("claude fresh"), None);
    }

    #[test]
    fn test_find_log_for_session_id_returns_newest_match() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("claude-root");
        let older = root
            .join("a")
            .join("12345678-1234-1234-1234-1234567890ab.jsonl");
        let newer = root
            .join("b")
            .join("session-12345678-1234-1234-1234-1234567890ab-copy.jsonl");
        std::fs::create_dir_all(older.parent().unwrap()).unwrap();
        std::fs::create_dir_all(newer.parent().unwrap()).unwrap();
        std::fs::write(&older, "").unwrap();
        std::fs::write(&newer, "").unwrap();

        let now = SystemTime::now();
        filetime::set_file_mtime(&older, FileTime::from_system_time(now)).unwrap();
        filetime::set_file_mtime(
            &newer,
            FileTime::from_system_time(now + std::time::Duration::from_secs(20)),
        )
        .unwrap();

        let result = find_log_for_session_id("12345678-1234-1234-1234-1234567890ab", &root);
        assert_eq!(result, Some(newer));
    }

    #[test]
    fn test_scan_latest_log_for_work_dir_skips_sidechain_and_returns_matching_log() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("claude-root");
        let work_dir = tmp.path().join("repo");
        std::fs::create_dir_all(&work_dir).unwrap();
        let older = root.join("a").join("normal.jsonl");
        let sidechain = root.join("a").join("sidechain.jsonl");
        let foreign = root.join("b").join("foreign.jsonl");
        for path in [&older, &sidechain, &foreign] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }

        let now = SystemTime::now();
        filetime::set_file_mtime(
            &older,
            FileTime::from_system_time(now + std::time::Duration::from_secs(5)),
        )
        .unwrap();
        filetime::set_file_mtime(
            &sidechain,
            FileTime::from_system_time(now + std::time::Duration::from_secs(20)),
        )
        .unwrap();
        filetime::set_file_mtime(
            &foreign,
            FileTime::from_system_time(now + std::time::Duration::from_secs(10)),
        )
        .unwrap();

        std::fs::write(
            &older,
            format!(
                "{{\"cwd\":\"{}\",\"sessionId\":\"sid-normal\",\"isSidechain\":false}}\n",
                work_dir.display()
            ),
        )
        .unwrap();
        std::fs::write(
            &sidechain,
            format!(
                "{{\"cwd\":\"{}\",\"sessionId\":\"sid-side\",\"isSidechain\":true}}\n",
                work_dir.display()
            ),
        )
        .unwrap();
        std::fs::write(
            &foreign,
            format!(
                "{{\"cwd\":\"{}\",\"sessionId\":\"sid-foreign\",\"isSidechain\":false}}\n",
                tmp.path().join("other").display()
            ),
        )
        .unwrap();

        let (log_path, session_id) = scan_latest_log_for_work_dir(&work_dir, &root, 10);
        assert_eq!(log_path, Some(older));
        assert_eq!(session_id.as_deref(), Some("sid-normal"));
    }
}
