//! Project/PID matching helpers.
//!
//! Mirrors Python `runtime_pid_cleanup.matching`.

use std::path::{Path, PathBuf};

/// Check whether `pid` belongs to a project based on cwd and cmdline hints.
///
/// Mirrors Python `runtime_pid_cleanup.matching.pid_matches_project`.
pub fn pid_matches_project(
    pid: u32,
    project_root: &Path,
    hint_paths: &[PathBuf],
    read_proc_path_fn: impl Fn(u32, &str) -> Option<PathBuf>,
    read_proc_cmdline_fn: impl Fn(u32) -> String,
    path_within_fn: impl Fn(&Path, &Path) -> bool,
    os_name: &str,
) -> bool {
    if os_name == "nt" {
        return true;
    }
    let normalized_hints = normalize_hint_roots(project_root, hint_paths);
    if let Some(cwd_path) = read_proc_path_fn(pid, "cwd") {
        for root in &normalized_hints {
            if path_within_fn(&cwd_path, root) {
                return true;
            }
        }
    }
    let cmdline = read_proc_cmdline_fn(pid);
    if !cmdline.is_empty() {
        for candidate in normalized_hints.iter().chain(hint_paths.iter()) {
            let text = candidate.to_string_lossy().trim().to_string();
            if !text.is_empty() && cmdline.contains(&text) {
                return true;
            }
        }
    }
    false
}

/// Normalize a set of hint roots for path comparisons.
///
/// Mirrors Python `runtime_pid_cleanup.matching.normalize_hint_roots`.
pub fn normalize_hint_roots(project_root: &Path, hint_paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = vec![project_root.to_path_buf()];
    for path in hint_paths {
        if let Some(parent) = path.parent() {
            candidates.push(parent.to_path_buf());
        }
    }

    let mut normalized: Vec<PathBuf> = Vec::new();
    for candidate in candidates {
        let resolved = if let Ok(resolved) = candidate.canonicalize() {
            resolved
        } else if let Ok(abs) = std::path::absolute(&candidate) {
            abs
        } else {
            candidate.clone()
        };
        if !normalized.contains(&resolved) {
            normalized.push(resolved);
        }
    }
    normalized
}

/// Check whether `path` is within `root`.
///
/// Mirrors Python `runtime_pid_cleanup.matching.path_within`.
pub fn path_within(path: &Path, root: &Path) -> bool {
    let resolved_path = if let Ok(p) = path.canonicalize() {
        p
    } else if let Ok(p) = std::path::absolute(path) {
        p
    } else {
        path.to_path_buf()
    };
    let resolved_root = if let Ok(r) = root.canonicalize() {
        r
    } else if let Ok(r) = std::path::absolute(root) {
        r
    } else {
        root.to_path_buf()
    };
    resolved_path.starts_with(&resolved_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_within_true_for_subpath() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        let child = root.join("child");
        std::fs::create_dir_all(&child).unwrap();
        assert!(path_within(&child, &root));
    }

    #[test]
    fn path_within_false_for_sibling() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        let sibling = tmp.path().join("sibling");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        assert!(!path_within(&sibling, &root));
    }

    #[test]
    fn normalize_hint_roots_includes_project_and_parents() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        let hint = project_root.join("sub").join("hint.pid");
        std::fs::create_dir_all(hint.parent().unwrap()).unwrap();

        let normalized = normalize_hint_roots(&project_root, std::slice::from_ref(&hint));
        assert!(normalized.contains(&project_root.canonicalize().unwrap()));
        assert!(normalized.contains(&hint.parent().unwrap().canonicalize().unwrap()));
    }

    #[test]
    fn pid_matches_project_on_windows_returns_true() {
        let result = pid_matches_project(
            123,
            Path::new("/project"),
            &[],
            |_pid, _entry| None,
            |_pid| String::new(),
            |_path, _root| false,
            "nt",
        );
        assert!(result);
    }

    #[test]
    fn pid_matches_project_by_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        let child = project_root.join("child");
        std::fs::create_dir_all(&child).unwrap();

        let result = pid_matches_project(
            123,
            &project_root,
            &[],
            |_pid, _entry| Some(child.clone()),
            |_pid| String::new(),
            path_within,
            "posix",
        );
        assert!(result);
    }

    #[test]
    fn pid_matches_project_by_cmdline() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        std::fs::create_dir_all(&project_root).unwrap();

        let hint = project_root.join("hint.pid");
        let result = pid_matches_project(
            123,
            &project_root,
            std::slice::from_ref(&hint),
            |_pid, _entry| None,
            |_pid| format!("some {} marker", hint.to_string_lossy()),
            path_within,
            "posix",
        );
        assert!(result);
    }

    #[test]
    fn pid_matches_project_no_match() {
        let result = pid_matches_project(
            123,
            Path::new("/project"),
            &[],
            |_pid, _entry| None,
            |_pid| String::new(),
            |_path, _root| false,
            "posix",
        );
        assert!(!result);
    }
}
