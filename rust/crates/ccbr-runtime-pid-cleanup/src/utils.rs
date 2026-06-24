//! Utility helpers for PID cleanup.
//!
//! Mirrors Python `runtime_pid_cleanup.utils`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Trait describing the runtime object used by collection helpers.
///
/// Python accepts an arbitrary object with `runtime_pid`, `pid`, and
/// `runtime_root` attributes. This trait provides the Rust equivalent.
pub trait RuntimeRef {
    /// Runtime PID if the runtime tracks one explicitly.
    fn runtime_pid(&self) -> Option<u32> {
        None
    }

    /// Fallback PID attribute.
    fn pid(&self) -> Option<u32> {
        None
    }

    /// Runtime root directory, if configured.
    fn runtime_root(&self) -> Option<&str> {
        None
    }
}

/// Coerce an arbitrary textual value into a positive PID.
///
/// Mirrors Python `runtime_pid_cleanup.utils.coerce_pid`.
pub fn coerce_pid(value: impl AsRef<str>) -> Option<u32> {
    let text = value.as_ref().trim();
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let pid = text.parse::<u32>().ok()?;
    if pid == 0 {
        return None;
    }
    Some(pid)
}

/// Resolve the runtime root directories to scan for `.pid` files.
///
/// Mirrors Python `runtime_pid_cleanup.utils.resolved_runtime_roots`.
pub fn resolved_runtime_roots(
    agent_dir: &Path,
    runtime: Option<&dyn RuntimeRef>,
    fallback_to_agent_dir: bool,
) -> Vec<PathBuf> {
    let mut runtime_root_paths: Vec<PathBuf> = Vec::new();
    if let Some(runtime) = runtime {
        if let Some(root) = runtime.runtime_root() {
            let trimmed = root.trim();
            if !trimmed.is_empty() {
                runtime_root_paths.push(PathBuf::from(trimmed));
            }
        }
    }
    if fallback_to_agent_dir || runtime_root_paths.is_empty() {
        runtime_root_paths.push(agent_dir.join("provider-runtime"));
    }

    let mut resolved: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for root in runtime_root_paths {
        let candidate = if let Ok(canon) = root.canonicalize() {
            canon
        } else if let Ok(abs) = std::path::absolute(&root) {
            abs
        } else {
            root.clone()
        };
        if seen.contains(&candidate) || !candidate.is_dir() {
            continue;
        }
        seen.insert(candidate.clone());
        resolved.push(candidate);
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TestRuntime {
        runtime_pid: Option<u32>,
        pid: Option<u32>,
        runtime_root: Option<String>,
    }

    impl RuntimeRef for TestRuntime {
        fn runtime_pid(&self) -> Option<u32> {
            self.runtime_pid
        }

        fn pid(&self) -> Option<u32> {
            self.pid
        }

        fn runtime_root(&self) -> Option<&str> {
            self.runtime_root.as_deref()
        }
    }

    #[test]
    fn coerce_pid_basic() {
        assert_eq!(coerce_pid("123"), Some(123));
        assert_eq!(coerce_pid("  42  "), Some(42));
    }

    #[test]
    fn coerce_pid_rejects_invalid() {
        assert_eq!(coerce_pid(""), None);
        assert_eq!(coerce_pid("abc"), None);
        assert_eq!(coerce_pid("12.3"), None);
        assert_eq!(coerce_pid("0"), None);
        assert_eq!(coerce_pid("-5"), None);
    }

    #[test]
    fn resolved_runtime_roots_uses_runtime_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("runtime-root");
        fs::create_dir(&root).unwrap();
        let agent_dir = tmp.path().join("agent");
        fs::create_dir(&agent_dir).unwrap();

        let runtime = TestRuntime {
            runtime_pid: None,
            pid: None,
            runtime_root: Some(root.to_string_lossy().to_string()),
        };

        let roots = resolved_runtime_roots(&agent_dir, Some(&runtime), false);
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0], root.canonicalize().unwrap());
    }

    #[test]
    fn resolved_runtime_roots_fallback_to_agent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let agent_dir = tmp.path().join("agent");
        let fallback = agent_dir.join("provider-runtime");
        fs::create_dir_all(&fallback).unwrap();

        let roots = resolved_runtime_roots(&agent_dir, None, true);
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0], fallback.canonicalize().unwrap());
    }

    #[test]
    fn resolved_runtime_roots_no_fallback_when_runtime_root_present() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("runtime-root");
        fs::create_dir(&root).unwrap();
        let agent_dir = tmp.path().join("agent");
        fs::create_dir(&agent_dir).unwrap();
        let fallback = agent_dir.join("provider-runtime");
        fs::create_dir_all(&fallback).unwrap();

        let runtime = TestRuntime {
            runtime_pid: None,
            pid: None,
            runtime_root: Some(root.to_string_lossy().to_string()),
        };

        let roots = resolved_runtime_roots(&agent_dir, Some(&runtime), false);
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0], root.canonicalize().unwrap());
    }
}
