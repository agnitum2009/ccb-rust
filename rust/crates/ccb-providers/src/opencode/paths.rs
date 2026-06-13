use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use regex::Regex;

/// Default OpenCode storage root candidates.
pub fn default_opencode_storage_root() -> Option<PathBuf> {
    first_existing_path(&storage_root_candidates())
}

/// Default OpenCode log root candidates.
pub fn default_opencode_log_root() -> Option<PathBuf> {
    first_existing_path(&log_root_candidates())
}

fn storage_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(env_root) = std::env::var("OPENCODE_STORAGE_ROOT") {
        let trimmed = env_root.trim();
        if !trimmed.is_empty() {
            candidates.push(PathBuf::from(shellexpand::tilde(trimmed)));
            return candidates;
        }
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        let trimmed = xdg.trim();
        if !trimmed.is_empty() {
            candidates.push(PathBuf::from(trimmed).join("opencode").join("storage"));
        }
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(
            home.join(".local")
                .join("share")
                .join("opencode")
                .join("storage"),
        );
    }
    candidates
}

fn log_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(env_root) = std::env::var("OPENCODE_LOG_ROOT") {
        let trimmed = env_root.trim();
        if !trimmed.is_empty() {
            candidates.push(PathBuf::from(shellexpand::tilde(trimmed)));
            return candidates;
        }
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        let trimmed = xdg.trim();
        if !trimmed.is_empty() {
            candidates.push(PathBuf::from(trimmed).join("opencode").join("log"));
        }
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(
            home.join(".local")
                .join("share")
                .join("opencode")
                .join("log"),
        );
        candidates.push(home.join(".opencode").join("log"));
    }
    candidates
}

fn first_existing_path(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .or_else(|| candidates.first().cloned())
}

/// Normalize a path for matching across platforms.
pub fn normalize_path_for_match(value: &str) -> String {
    let s = value.trim();
    let expanded = shellexpand::tilde(s);
    let normalized = std::fs::canonicalize(Path::new(&*expanded))
        .unwrap_or_else(|_| Path::new(&*expanded).to_path_buf());
    normalized
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}

/// Check whether `child` is the same as or under `parent`.
pub fn path_is_same_or_parent(parent: &str, child: &str) -> bool {
    let parent = normalize_path_for_match(parent);
    let child = normalize_path_for_match(child);
    if parent == child {
        return true;
    }
    if parent.is_empty() || child.is_empty() {
        return false;
    }
    if !child.starts_with(&parent) {
        return false;
    }
    child == parent || child[parent.len()..].starts_with('/')
}

/// Check whether two paths match, optionally allowing parent matches.
pub fn path_matches(expected: &str, actual: &str, allow_parent: bool) -> bool {
    if allow_parent {
        path_is_same_or_parent(expected, actual)
    } else {
        normalize_path_for_match(expected) == normalize_path_for_match(actual)
    }
}

/// Check whether a string value looks like a truthy environment variable.
pub fn env_truthy(name: &str) -> bool {
    let raw = std::env::var(name).unwrap_or_default();
    matches!(
        raw.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Best-effort WSL detection.
pub fn is_wsl() -> bool {
    if std::env::var("WSL_INTEROP").is_ok() || std::env::var("WSL_DISTRO_NAME").is_ok() {
        return true;
    }
    std::fs::read_to_string("/proc/version")
        .unwrap_or_default()
        .to_lowercase()
        .contains("microsoft")
}

/// REQ_ID regex pattern used to extract request ids from OpenCode text.
/// Mirrors the Python `opencode_runtime.paths_runtime.project_id_runtime.patterns.REQ_ID_RE`
/// while also keeping the `[ccb:req]` alias used by the ported helper tests.
pub fn req_id_re() -> Regex {
    Regex::new(r"(?:\[?ccb:?req\]?|req-)\s*([0-9a-fA-F]{32}|\d{8}-\d{6}-\d{3}-\d+-\d+)").unwrap()
}

/// Compute the OpenCode project id for a work directory.
pub fn compute_opencode_project_id(work_dir: &Path) -> String {
    let cwd = normalize_work_dir(work_dir);
    let (git_root, git_dir) = find_git_dir(&cwd);
    if let Some(cached) = git_dir.as_ref().and_then(|p| read_cached_project_id(p)) {
        return cached;
    }
    root_commit_project_id(git_root.as_deref(), &cwd)
}

fn normalize_work_dir(work_dir: &Path) -> PathBuf {
    PathBuf::from(shellexpand::tilde(&work_dir.to_string_lossy()))
}

fn find_git_dir(start: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    let mut current = Some(start);
    while let Some(dir) = current {
        let git_entry = dir.join(".git");
        if git_entry.exists() {
            if let Some(resolved) = resolve_git_entry(dir, &git_entry) {
                return resolved;
            }
        }
        current = dir.parent();
    }
    (None, None)
}

fn resolve_git_entry(
    candidate: &Path,
    git_entry: &Path,
) -> Option<(Option<PathBuf>, Option<PathBuf>)> {
    if git_entry.is_dir() {
        return Some((Some(candidate.to_path_buf()), Some(git_entry.to_path_buf())));
    }
    let raw = std::fs::read_to_string(git_entry).ok()?;
    let prefix = "gitdir:";
    if raw.trim().to_lowercase().starts_with(prefix) {
        let gitdir = raw[prefix.len()..].trim();
        let gitdir_path = PathBuf::from(gitdir);
        let resolved = if gitdir_path.is_relative() {
            candidate
                .join(&gitdir_path)
                .canonicalize()
                .unwrap_or(gitdir_path)
        } else {
            gitdir_path
        };
        return Some((Some(candidate.to_path_buf()), Some(resolved)));
    }
    None
}

fn read_cached_project_id(git_dir: &Path) -> Option<String> {
    let cache_path = git_dir.join("opencode");
    if !cache_path.exists() {
        return None;
    }
    let cached = std::fs::read_to_string(cache_path).ok()?;
    let trimmed = cached.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

fn root_commit_project_id(git_root: Option<&Path>, cwd: &Path) -> String {
    let work_dir = git_root.unwrap_or(cwd);
    let output = std::process::Command::new("git")
        .arg("rev-list")
        .arg("--max-parents=0")
        .arg("--all")
        .current_dir(work_dir)
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let roots: BTreeSet<_> = stdout
                .lines()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            let first = roots.into_iter().next().map(|s| s.to_string());
            first.unwrap_or_else(|| "global".to_string())
        }
        _ => "global".to_string(),
    }
}

// Minimal shell expansion helper.
mod shellexpand {
    use std::env;

    pub fn tilde(input: &str) -> String {
        if let Some(rest) = input.strip_prefix('~') {
            if let Ok(home) = env::var("HOME") {
                return home + rest;
            }
        }
        input.to_string()
    }
}

// Minimal dirs::home_dir replacement.
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}
