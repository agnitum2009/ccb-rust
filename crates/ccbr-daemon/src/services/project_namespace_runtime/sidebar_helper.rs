//! Mirrors Python `lib/ccbrd/services/project_namespace_runtime/sidebar_helper.py`.
//! Resolves the `ccbr-agent-sidebar` helper binary and builds respawn arguments.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const SIDEBAR_BINARY_NAME: &str = "ccbr-agent-sidebar";
pub const SIDEBAR_ENV_PATH: &str = "CCBR_AGENT_SIDEBAR_BIN";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarHelperResolution {
    pub path: Option<String>,
    pub source: String,
    pub reason: Option<String>,
}

impl SidebarHelperResolution {
    pub fn available(&self) -> bool {
        self.path.is_some()
    }
}

/// Resolve the sidebar helper binary using the same search order as Python:
/// 1. `CCBR_AGENT_SIDEBAR_BIN` environment override
/// 2. `<script_root>/bin/ccbr-agent-sidebar`
/// 3. `$CODEX_INSTALL_PREFIX/bin/ccbr-agent-sidebar`
/// 4. `PATH` lookup
pub fn resolve_sidebar_helper(
    env: Option<&HashMap<String, String>>,
    script_root: Option<&Path>,
) -> SidebarHelperResolution {
    let process_env = lazy_env_map();
    let env_map = env.unwrap_or(&process_env);

    if let Some(override_path) = clean_text(env_map.get(SIDEBAR_ENV_PATH)) {
        return resolve_explicit(Path::new(&override_path), SIDEBAR_ENV_PATH);
    }

    let root = script_root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(default_script_root);
    let root_candidate = root.join("bin").join(SIDEBAR_BINARY_NAME);
    if is_executable_file(&root_candidate) {
        return SidebarHelperResolution {
            path: Some(root_candidate.to_string_lossy().to_string()),
            source: "script_root_bin".to_string(),
            reason: None,
        };
    }

    if let Some(prefix) = clean_text(env_map.get("CODEX_INSTALL_PREFIX")) {
        let prefix_candidate = Path::new(&prefix).join("bin").join(SIDEBAR_BINARY_NAME);
        if is_executable_file(&prefix_candidate) {
            return SidebarHelperResolution {
                path: Some(prefix_candidate.to_string_lossy().to_string()),
                source: "CODEX_INSTALL_PREFIX".to_string(),
                reason: None,
            };
        }
    }

    if let Some(path_candidate) = which(SIDEBAR_BINARY_NAME) {
        return SidebarHelperResolution {
            path: Some(path_candidate),
            source: "PATH".to_string(),
            reason: None,
        };
    }

    SidebarHelperResolution {
        path: None,
        source: "missing".to_string(),
        reason: Some(format!(
            "{SIDEBAR_BINARY_NAME} not found in {SIDEBAR_ENV_PATH}, repository bin, install prefix bin, or PATH"
        )),
    }
}

/// Build the final argv used to respawn a sidebar pane.
///
/// If the first argument is `ccbr-agent-sidebar`, it is replaced with the resolved
/// absolute path. Otherwise the args are returned unchanged.
pub fn sidebar_respawn_args(
    launch_args: &[String],
    env: Option<&HashMap<String, String>>,
    script_root: Option<&Path>,
) -> Vec<String> {
    if launch_args.is_empty() || launch_args[0] != SIDEBAR_BINARY_NAME {
        return launch_args.to_vec();
    }
    let resolution = resolve_sidebar_helper(env, script_root);
    if let Some(path) = resolution.path {
        let mut args = vec![path];
        args.extend(launch_args[1..].iter().cloned());
        return args;
    }
    missing_sidebar_respawn_args(resolution.reason.as_deref())
}

/// Fallback argv shown when the sidebar helper cannot be resolved.
pub fn missing_sidebar_respawn_args(reason: Option<&str>) -> Vec<String> {
    let message = "CCBR sidebar helper unavailable";
    let detail = reason.unwrap_or("ccbr-agent-sidebar not found");
    let body = format!(
        "printf '%s\\n' '{}'; printf '%s\\n' '{}'; printf '%s\\n' 'Build or install bin/ccbr-agent-sidebar, or set CCBR_AGENT_SIDEBAR_BIN.'; while :; do sleep 3600; done",
        shell_single_quote_text(message),
        shell_single_quote_text(detail),
    );
    vec!["sh".to_string(), "-lc".to_string(), body]
}

fn resolve_explicit(path: &Path, source: &str) -> SidebarHelperResolution {
    if is_executable_file(path) {
        SidebarHelperResolution {
            path: Some(path.to_string_lossy().to_string()),
            source: source.to_string(),
            reason: None,
        }
    } else {
        SidebarHelperResolution {
            path: None,
            source: source.to_string(),
            reason: Some(format!(
                "{source} points to a missing or non-executable file: {}",
                path.display()
            )),
        }
    }
}

fn default_script_root() -> PathBuf {
    // Mirror Python's `Path(__file__).resolve().parents[4]`, which for
    // `lib/ccbrd/services/project_namespace_runtime/sidebar_helper.py` resolves
    // to the repository root. In the Rust workspace this file lives under
    // `rust/crates/ccbr-daemon/src/services/project_namespace_runtime/`, so we
    // walk up five levels to reach the repository root.
    let mut path = std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    for _ in 0..5 {
        if let Some(parent) = path.parent() {
            path = parent.to_path_buf();
        } else {
            break;
        }
    }
    path
}

fn is_executable_file(path: &Path) -> bool {
    path.is_file() && is_executable(path)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

fn clean_text(value: Option<&String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn shell_single_quote_text(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}

fn lazy_env_map() -> HashMap<String, String> {
    std::env::vars().collect()
}

fn which(name: &str) -> Option<String> {
    if let Ok(path_env) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_env) {
            let candidate = dir.join(name);
            if is_executable_file(&candidate) {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_sidebar_respawn_args_unchanged_when_not_sidebar_binary() {
        let args = vec!["echo".to_string(), "hello".to_string()];
        let resolved = sidebar_respawn_args(&args, None, None);
        assert_eq!(resolved, args);
    }

    #[test]
    fn test_sidebar_respawn_args_empty() {
        let args: Vec<String> = Vec::new();
        let resolved = sidebar_respawn_args(&args, None, None);
        assert!(resolved.is_empty());
    }

    #[test]
    fn test_missing_sidebar_respawn_args_has_fallback_shell() {
        let args = missing_sidebar_respawn_args(Some("not found"));
        assert_eq!(args[0], "sh");
        assert_eq!(args[1], "-lc");
        assert!(args[2].contains("CCBR sidebar helper unavailable"));
        assert!(args[2].contains("not found"));
    }

    #[test]
    fn test_resolve_explicit_executable() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        tmpfile.write_all(b"#!/bin/sh\n").unwrap();
        let path = tmpfile.path();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms).unwrap();
        }
        let resolution = resolve_explicit(path, "TEST_SOURCE");
        assert!(resolution.available());
        assert_eq!(resolution.source, "TEST_SOURCE");
    }

    #[test]
    fn test_resolve_explicit_missing() {
        let path = PathBuf::from("/non/existent/ccbr-agent-sidebar");
        let resolution = resolve_explicit(&path, "TEST_SOURCE");
        assert!(!resolution.available());
        assert!(resolution.reason.unwrap().contains("missing"));
    }
}
