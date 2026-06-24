//! Mirrors Python `lib/provider_backends/claude/registry_support/pathing.py`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

const CCB_PROJECT_CONFIG_DIRNAME: &str = ".ccbr";

/// Normalize a project path the same way Python
/// `registry_support.pathing_runtime.normalization.normalize_project_path` does.
pub fn normalize_project_path(value: impl AsRef<Path>) -> String {
    let raw = value.as_ref();
    let expanded = expand_tilde(raw);
    let absolute = if expanded.exists() {
        std::fs::canonicalize(&expanded)
            .unwrap_or_else(|_| AbsolutePath::to_absolute(expanded.as_path()))
    } else {
        AbsolutePath::to_absolute(expanded.as_path())
    };
    let mut normalized = absolute.to_string_lossy().replace('\\', "/");
    normalized = normalized.trim_end_matches('/').to_string();
    #[cfg(target_os = "windows")]
    {
        normalized = normalized.to_lowercase();
    }
    normalized
}

fn expand_tilde(path: &Path) -> PathBuf {
    if let Some(rest) = path.to_str().and_then(|s| s.strip_prefix('~')) {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    path.to_path_buf()
}

trait AbsolutePath {
    fn to_absolute(&self) -> PathBuf;
}

impl AbsolutePath for Path {
    fn to_absolute(&self) -> PathBuf {
        if self.is_absolute() {
            self.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(self))
                .unwrap_or_else(|_| self.to_path_buf())
        }
    }
}

/// Return true when `child` is the same as, or a descendant of, `parent`.
pub fn path_within(child: impl AsRef<Path>, parent: impl AsRef<Path>) -> bool {
    let normalized_child = normalize_project_path(child);
    let normalized_parent = normalize_project_path(parent);
    if normalized_child.is_empty() || normalized_parent.is_empty() {
        return false;
    }
    if normalized_child == normalized_parent {
        return true;
    }
    normalized_child.starts_with(&format!("{}/", normalized_parent))
}

/// Derive a project key string from a path by replacing non-alphanumeric characters.
pub fn project_key_for_path(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

/// Candidate work directories for a work dir (PWD, work_dir, resolved work_dir).
pub fn candidate_work_dirs(work_dir: &Path, include_env_pwd: bool) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if include_env_pwd {
        if let Ok(env_pwd) = std::env::var("PWD") {
            candidates.push(PathBuf::from(env_pwd));
        }
    }
    candidates.push(work_dir.to_path_buf());
    if let Ok(resolved) = work_dir.canonicalize() {
        candidates.push(resolved);
    }
    candidates
}

/// Normalized candidate project path strings for a work directory.
pub fn candidate_project_paths(work_dir: &Path, include_env_pwd: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for candidate in candidate_work_dirs(work_dir, include_env_pwd) {
        let normalized = normalize_project_path(&candidate);
        if normalized.is_empty() || seen.contains(&normalized) {
            continue;
        }
        seen.insert(normalized.clone());
        out.push(normalized);
    }
    out
}

/// Candidate project directories under `root` for a work directory.
pub fn candidate_project_dirs(root: &Path, work_dir: &Path, include_env_pwd: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for candidate in candidate_work_dirs(work_dir, include_env_pwd) {
        let key = project_key_for_path(&candidate);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key.clone());
        out.push(root.join(key));
    }
    out
}

/// Infer the work directory from a session file path.
pub fn infer_work_dir_from_session_file(session_file: &Path) -> PathBuf {
    let parent = session_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    if parent.file_name().and_then(|n| n.to_str()) == Some(CCB_PROJECT_CONFIG_DIRNAME) {
        parent.parent().map(Path::to_path_buf).unwrap_or(parent)
    } else {
        parent
    }
}

/// Ensure a session payload has `work_dir`, `work_dir_norm`, and `ccb_project_id`.
/// Returns the work directory path if available.
pub fn ensure_claude_session_work_dir_fields(
    payload: &mut Map<String, Value>,
    session_file: &Path,
) -> Option<PathBuf> {
    let work_dir = work_dir_text(payload)
        .map(PathBuf::from)
        .unwrap_or_else(|| infer_work_dir_from_session_file(session_file));
    if work_dir_text(payload).is_none() {
        payload.insert(
            "work_dir".to_string(),
            Value::String(work_dir.to_string_lossy().to_string()),
        );
    }
    assign_work_dir_norm(payload, &work_dir);
    assign_project_id(payload, &work_dir);
    Some(work_dir)
}

fn work_dir_text(payload: &Map<String, Value>) -> Option<String> {
    payload
        .get("work_dir")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn assign_work_dir_norm(payload: &mut Map<String, Value>, work_dir: &Path) {
    if payload
        .get("work_dir_norm")
        .and_then(Value::as_str)
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        return;
    }
    payload.insert(
        "work_dir_norm".to_string(),
        Value::String(normalize_project_path(work_dir)),
    );
}

fn assign_project_id(payload: &mut Map<String, Value>, work_dir: &Path) {
    if payload
        .get("ccb_project_id")
        .and_then(Value::as_str)
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        return;
    }
    if let Ok(id) = compute_ccb_project_id(work_dir) {
        payload.insert("ccb_project_id".to_string(), Value::String(id));
    }
}

fn compute_ccb_project_id(work_dir: &Path) -> Result<String, String> {
    let normalized = normalize_project_path(work_dir);
    if normalized.is_empty() {
        return Err("empty work_dir".to_string());
    }
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(normalized.as_bytes());
    Ok(format!("{:x}", digest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_project_key_for_path_replaces_non_alphanumeric() {
        assert_eq!(
            project_key_for_path(Path::new("/home/user/demo_project")),
            "-home-user-demo-project"
        );
    }

    #[test]
    fn test_path_within_detects_descendant() {
        assert!(path_within("/a/b/c", "/a/b"));
        assert!(path_within("/a/b", "/a/b"));
        assert!(!path_within("/a/bc", "/a/b"));
        assert!(!path_within("/x/y", "/a/b"));
    }

    #[test]
    fn test_normalize_project_path_returns_absolute_with_slashes() {
        let normalized = normalize_project_path("/home/user/project");
        assert_eq!(normalized, "/home/user/project");
    }

    #[test]
    fn test_candidate_project_paths_dedupes() {
        let work_dir = std::env::current_dir().unwrap();
        let paths = candidate_project_paths(&work_dir, true);
        let set: HashSet<_> = paths.iter().cloned().collect();
        assert_eq!(set.len(), paths.len());
        assert!(paths.contains(&normalize_project_path(&work_dir)));
    }

    #[test]
    fn test_infer_work_dir_from_session_file_skips_ccb_dir() {
        let session = Path::new("/project/.ccbr/.claude-session");
        assert_eq!(
            infer_work_dir_from_session_file(session),
            PathBuf::from("/project")
        );
    }

    #[test]
    fn test_ensure_work_dir_fields_populates_missing_values() {
        let mut payload = Map::new();
        let session_file = Path::new("/project/.ccbr/.claude-session");
        let work_dir = ensure_claude_session_work_dir_fields(&mut payload, session_file).unwrap();
        assert_eq!(work_dir, PathBuf::from("/project"));
        assert!(payload.contains_key("work_dir"));
        assert!(payload.contains_key("work_dir_norm"));
        assert!(payload.contains_key("ccb_project_id"));
    }
}
