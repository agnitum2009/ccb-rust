use std::env;
use std::path::{Path, PathBuf};

/// Determine the provider source home directory.
///
/// Resolution order:
/// 1. `CCBR_SOURCE_HOME` environment variable.
/// 2. `HOME` environment variable, unless it looks like a CCBR provider home.
/// 3. `USERPROFILE` environment variable (Windows fallback).
/// 4. `HOME` as a last resort.
pub fn current_provider_source_home() -> PathBuf {
    if let Some(path) = env_path("CCBR_SOURCE_HOME") {
        return path;
    }

    if let Some(path) = env_path("HOME") {
        if !looks_like_ccbr_provider_home(&path) {
            return path;
        }
    }

    if let Some(path) = passwd_home() {
        return path;
    }

    if let Some(path) = env_path("USERPROFILE") {
        return path;
    }

    env_path("HOME").unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(unix)]
fn passwd_home() -> Option<PathBuf> {
    unsafe {
        let uid = libc::getuid();
        let pw = libc::getpwuid(uid);
        if pw.is_null() {
            return None;
        }
        let dir = std::ffi::CStr::from_ptr((*pw).pw_dir);
        let path = dir.to_str().ok()?.to_string();
        if path.trim().is_empty() {
            return None;
        }
        Some(PathBuf::from(path))
    }
}

#[cfg(not(unix))]
fn passwd_home() -> Option<PathBuf> {
    None
}

fn env_path(name: &str) -> Option<PathBuf> {
    let raw = env::var(name).unwrap_or_default();
    if raw.trim().is_empty() {
        return None;
    }
    let expanded = if let Some(rest) = raw.strip_prefix('~') {
        if let Ok(home) = env::var("HOME") {
            format!("{home}{rest}")
        } else {
            raw
        }
    } else {
        raw
    };
    let path = PathBuf::from(expanded);
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

fn looks_like_ccbr_provider_home(path: &Path) -> bool {
    let parts: Vec<String> = path
        .iter()
        .map(|c| c.to_string_lossy().to_string())
        .collect();
    if parts.len() < 5 {
        return false;
    }
    for index in 0..=parts.len() - 5 {
        if parts[index] != "agents" {
            continue;
        }
        if parts[index + 2] == "provider-state" && parts[index + 4] == "home" {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_ccbr_provider_home() {
        assert!(looks_like_ccbr_provider_home(&PathBuf::from(
            "/x/agents/foo/provider-state/bar/home"
        )));
        assert!(!looks_like_ccbr_provider_home(&PathBuf::from("/home/user")));
        assert!(!looks_like_ccbr_provider_home(&PathBuf::from(
            "/agents/a/provider-state/b"
        )));
    }

    #[test]
    fn test_current_provider_source_home_returns_path() {
        let home = current_provider_source_home();
        assert!(!home.as_os_str().is_empty());
    }
}
