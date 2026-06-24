//! Mirrors Python `lib/provider_backends/claude/session_index_runtime.py`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::registry_support::pathing::{
    candidate_project_paths, normalize_project_path, project_key_for_path,
};

/// Location of a sessions index file and its project directory.
#[derive(Debug, Clone)]
pub struct SessionIndexLocation {
    pub index_path: PathBuf,
    pub project_dir: PathBuf,
}

/// Normalized candidate paths for a work directory.
pub fn candidate_paths_for_work_dir(work_dir: &Path, include_env_pwd: bool) -> HashSet<String> {
    candidate_project_paths(work_dir, include_env_pwd)
        .into_iter()
        .collect()
}

/// Resolve the index location for a work directory under `root`.
pub fn resolve_registry_index_location(
    work_dir: &Path,
    root: &Path,
) -> Option<SessionIndexLocation> {
    let root = expand_tilde(root);
    if let Some(location) =
        project_index_location(&root, &root.join(project_key_for_path(work_dir)))
    {
        return Some(location);
    }
    let resolved = work_dir
        .canonicalize()
        .unwrap_or_else(|_| work_dir.to_path_buf());
    if resolved == work_dir {
        return None;
    }
    project_index_location(&root, &root.join(project_key_for_path(&resolved)))
}

fn project_index_location(root: &Path, project_dir: &Path) -> Option<SessionIndexLocation> {
    let _ = root;
    let index_path = project_dir.join("sessions-index.json");
    if !index_path.exists() {
        return None;
    }
    Some(SessionIndexLocation {
        index_path,
        project_dir: project_dir.to_path_buf(),
    })
}

/// Load entries from a sessions index file.
pub fn load_index_entries(index_path: &Path) -> Option<Vec<serde_json::Map<String, Value>>> {
    let raw = std::fs::read_to_string(index_path).ok()?;
    let payload: Value = serde_json::from_str(&raw).ok()?;
    let entries = payload.get("entries")?.as_array()?;
    Some(
        entries
            .iter()
            .filter_map(|entry| entry.as_object().cloned())
            .collect(),
    )
}

/// Select the best session path from index entries.
pub fn select_best_session_path(
    entries: &[serde_json::Map<String, Value>],
    candidates: &HashSet<String>,
    project_dir: &Path,
) -> Option<PathBuf> {
    let mut best_path: Option<PathBuf> = None;
    let mut best_mtime: i64 = -1;
    for entry in entries {
        if let Some((session_path, mtime)) = entry_session_path(entry, candidates, project_dir) {
            if mtime > best_mtime {
                best_mtime = mtime;
                best_path = Some(session_path);
            }
        }
    }
    best_path
}

fn entry_session_path(
    entry: &serde_json::Map<String, Value>,
    candidates: &HashSet<String>,
    project_dir: &Path,
) -> Option<(PathBuf, i64)> {
    if entry.get("isSidechain").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    if !entry_matches_candidates(entry, candidates) {
        return None;
    }
    let session_path = resolve_session_path(entry, project_dir)?;
    let mtime = entry_mtime(entry, &session_path)?;
    Some((session_path, mtime))
}

fn entry_matches_candidates(
    entry: &serde_json::Map<String, Value>,
    candidates: &HashSet<String>,
) -> bool {
    let project_path = entry
        .get("projectPath")
        .and_then(Value::as_str)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    if let Some(path) = project_path {
        let normalized = normalize_project_path(path);
        return candidates.is_empty() || normalized.is_empty() || candidates.contains(&normalized);
    }
    candidates.is_empty()
}

fn resolve_session_path(
    entry: &serde_json::Map<String, Value>,
    project_dir: &Path,
) -> Option<PathBuf> {
    let full_path = entry
        .get("fullPath")
        .and_then(Value::as_str)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())?;
    let mut path = expand_tilde(Path::new(full_path));
    if !path.is_absolute() {
        path = project_dir.join(path);
    }
    if !path.exists() {
        return None;
    }
    Some(path)
}

fn entry_mtime(entry: &serde_json::Map<String, Value>, session_path: &Path) -> Option<i64> {
    if let Some(raw) = entry.get("fileMtime") {
        if let Some(n) = raw.as_i64() {
            return Some(n);
        }
        if let Some(s) = raw.as_str().map(|s| s.trim()) {
            if let Ok(n) = s.parse::<i64>() {
                return Some(n);
            }
        }
    }
    let metadata = session_path.metadata().ok()?;
    let modified = metadata.modified().ok()?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Some(secs * 1000)
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
    use std::collections::HashSet;
    use tempfile::TempDir;

    #[test]
    fn test_load_index_entries_returns_dict_entries() {
        let tmp = TempDir::new().unwrap();
        let index_path = tmp.path().join("sessions-index.json");
        std::fs::write(
            &index_path,
            serde_json::json!({
                "entries": [
                    {"fullPath": "session-1.jsonl", "fileMtime": 1000},
                    "ignored",
                    {"fullPath": "session-2.jsonl", "fileMtime": 2000},
                ]
            })
            .to_string(),
        )
        .unwrap();
        let entries = load_index_entries(&index_path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].get("fullPath").unwrap().as_str().unwrap(),
            "session-1.jsonl"
        );
    }

    #[test]
    fn test_select_best_session_path_prefers_newer_mtime() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project-key");
        std::fs::create_dir(&project_dir).unwrap();
        let older = project_dir.join("session-1.jsonl");
        let newer = project_dir.join("session-2.jsonl");
        std::fs::write(&older, "").unwrap();
        std::fs::write(&newer, "").unwrap();

        let entries = vec![
            serde_json::json!({"fullPath": "session-1.jsonl", "fileMtime": 1000})
                .as_object()
                .unwrap()
                .clone(),
            serde_json::json!({"fullPath": "session-2.jsonl", "fileMtime": 2000})
                .as_object()
                .unwrap()
                .clone(),
        ];
        let candidates: HashSet<String> = HashSet::new();
        let best = select_best_session_path(&entries, &candidates, &project_dir).unwrap();
        assert_eq!(best, newer);
    }
}
