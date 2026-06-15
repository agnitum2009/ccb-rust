//! Mirrors Python `lib/cli/management_runtime/install.py`.
//!
//! Install directory discovery helpers. The tarball download / extract and
//! `run_installer` flow remain TODO until the provider installer runtime is
//! wired in; the pure path logic below is sufficient to unblock `cmd_version`.

use std::path::{Path, PathBuf};

/// Resolve the active install directory for the bridge.
///
/// Mirrors Python `find_install_dir(script_root)`.
pub fn find_install_dir(script_root: &Path) -> PathBuf {
    if script_root.join("install.sh").exists() || script_root.join("install.ps1").exists() {
        return script_root.to_path_buf();
    }
    for candidate in install_dir_candidates() {
        if installed_candidate(&candidate) {
            return candidate;
        }
    }
    script_root.to_path_buf()
}

fn env_install_prefix() -> Option<PathBuf> {
    let env_prefix = std::env::var("CODEX_INSTALL_PREFIX").ok()?;
    let trimmed = env_prefix.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(expand_user(trimmed)))
}

fn default_install_dir() -> PathBuf {
    if let Some(prefix) = env_install_prefix() {
        return prefix;
    }
    if cfg!(windows) {
        return windows_install_dir_candidates()[0].clone();
    }
    home_dir().join(".local/share/codex-dual")
}

fn install_dir_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = vec![default_install_dir()];
    if cfg!(windows) {
        for candidate in windows_install_dir_candidates() {
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }
    candidates
}

#[cfg(windows)]
fn windows_install_dir_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        let base = PathBuf::from(localappdata);
        candidates.push(base.join("codex-dual"));
        candidates.push(base.join("claude-code-bridge"));
    }
    candidates.push(home_dir().join("AppData/Local/codex-dual"));
    candidates
}

#[cfg(not(windows))]
fn windows_install_dir_candidates() -> Vec<PathBuf> {
    Vec::new()
}

fn installed_candidate(candidate: &Path) -> bool {
    !candidate.as_os_str().is_empty() && candidate.join("ccb").exists()
}

/// Return true when `script_root` looks like a source repo checkout.
///
/// Mirrors Python `is_source_repo_root(script_root)`.
pub fn is_source_repo_root(script_root: &Path) -> bool {
    let root = PathBuf::from(expand_user(&script_root.to_string_lossy()));
    root.join("install.sh").exists() && root.join(".git").exists()
}

fn home_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        if !userprofile.is_empty() {
            return PathBuf::from(userprofile);
        }
    }
    PathBuf::from("/")
}

fn expand_user(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~") {
        if let Some(sep) = rest.chars().next() {
            if sep == '/' || sep == '\\' {
                return format!("{}{}", home_dir().display(), rest);
            }
        }
    }
    path.to_string()
}

// TODO: align `run_installer`, `pick_temp_base_dir`, `download_tarball`,
// `safe_extract_tar` with Python once the tarball installer runtime is wired.
