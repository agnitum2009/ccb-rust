use camino::Utf8PathBuf;
use sha2::{Digest, Sha256};
use std::env;
use std::path::PathBuf;

/// Compute SHA256 hex digest of input bytes.
fn sha256_hex(input: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    hex::encode(hasher.finalize())
}

/// Expand a leading `~` using the HOME environment variable.
fn expand_user_path(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix('~') {
        if let Ok(home) = env::var("HOME") {
            return home + rest;
        }
    }
    raw.to_string()
}

/// True if `raw` looks like an absolute path on any supported platform.
fn is_absolute_preview(raw: &str) -> bool {
    let preview = raw.replace('\\', "/");
    preview.starts_with('/')
        || preview.starts_with("//")
        || preview.starts_with("\\\\")
        || raw.len() >= 2 && raw.as_bytes()[1] == b':' && raw.as_bytes()[0].is_ascii_alphabetic()
}

/// Make a relative path absolute against the current working directory.
fn absolutize_relative_path(raw: &str) -> String {
    if is_absolute_preview(raw) {
        return raw.to_string();
    }
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.join(raw).to_string_lossy().to_string()
}

fn normalize_path_slashes(raw: &str) -> String {
    raw.replace('\\', "/")
}

/// Convert `/mnt/X/rest` to `x:/rest`.
fn normalize_mnt_drive_mapping(value: &str) -> Option<String> {
    let rest = value.strip_prefix("/mnt/")?;
    let drive = rest.chars().next()?;
    if !drive.is_ascii_alphabetic() {
        return None;
    }
    let after_drive = &rest[drive.len_utf8()..];
    if after_drive.is_empty() {
        return Some(format!("{}:/", drive.to_lowercase()));
    }
    if after_drive.starts_with('/') {
        Some(format!("{}:{}", drive.to_lowercase(), after_drive))
    } else {
        None
    }
}

/// Convert `/X/rest` to `x:/rest` when running under MSYS/Windows.
fn normalize_msys_drive_mapping(value: &str) -> Option<String> {
    if value.len() < 2 || value.as_bytes()[0] != b'/' {
        return None;
    }
    let drive = value.chars().nth(1)?;
    if !drive.is_ascii_alphabetic() {
        return None;
    }
    if env::var("MSYSTEM").is_ok() || env::consts::OS == "windows" {
        let after = &value[1 + drive.len_utf8()..];
        if after.starts_with('/') {
            return Some(format!("{}:{}", drive.to_lowercase(), after));
        }
    }
    None
}

fn normalize_platform_drive_mapping(value: &str) -> String {
    normalize_mnt_drive_mapping(value)
        .or_else(|| normalize_msys_drive_mapping(value))
        .unwrap_or_else(|| value.to_string())
}

fn normalize_posix_path(value: &str) -> String {
    PathBuf::from(value).to_string_lossy().to_string()
}

fn normalize_work_dir_segments(value: &str) -> String {
    if let Some(rest) = value.strip_prefix("//") {
        let norm = normalize_posix_path(rest);
        return format!("//{}", norm.trim_start_matches('/'));
    }
    normalize_posix_path(value)
}

fn normalize_drive_letter_case(value: &str) -> String {
    if value.len() >= 2 && value.as_bytes()[0].is_ascii_alphabetic() && value.as_bytes()[1] == b':'
    {
        value[0..1].to_lowercase() + &value[1..]
    } else {
        value.to_string()
    }
}

/// Normalize a work_dir into a stable string for hashing and matching.
/// Mirrors Python `project.identity.normalize_work_dir`.
pub fn normalize_work_dir(value: impl AsRef<str>) -> String {
    let mut raw = value.as_ref().trim().to_string();
    if raw.is_empty() {
        return String::new();
    }
    raw = expand_user_path(&raw);
    raw = absolutize_relative_path(&raw);
    let mut normalized = normalize_path_slashes(&raw);
    normalized = normalize_platform_drive_mapping(&normalized);
    normalized = normalize_work_dir_segments(&normalized);
    normalize_drive_letter_case(&normalized)
}

/// Compute a stable worktree/workspace scope id (first 12 hex chars of SHA256).
/// Mirrors Python `project.identity.compute_worktree_scope_id`.
pub fn compute_worktree_scope_id(work_dir: impl AsRef<str>) -> String {
    let norm = normalize_work_dir(work_dir);
    if norm.is_empty() {
        return String::new();
    }
    sha256_hex(norm.as_bytes())[..12].to_string()
}

/// Resolve the project root from a work_dir, considering workspace bindings and anchors.
/// Mirrors Python `project.identity.resolve_project_root`.
pub fn resolve_project_root(work_dir: impl AsRef<str>) -> Utf8PathBuf {
    // Workspace binding and anchor discovery are not yet implemented in Rust;
    // fall back to the normalized input path.
    Utf8PathBuf::from(normalize_work_dir(work_dir))
}

/// Convert `/mnt/X/rest` to `x:/rest` for project path normalization.
fn normalize_project_mnt_drive(value: &str) -> Option<String> {
    normalize_mnt_drive_mapping(value)
}

fn normalize_project_path_segments(value: &str) -> String {
    if let Some(rest) = value.strip_prefix("//") {
        let norm = normalize_posix_path(rest);
        return format!("//{}", norm.trim_start_matches('/'));
    }
    normalize_posix_path(value)
}

/// Normalize a project path the same way Python `project.ids.normalize_project_path` does.
pub fn normalize_project_path(value: impl AsRef<str>) -> String {
    let mut raw = value.as_ref().trim().to_string();
    if raw.is_empty() {
        return String::new();
    }
    if raw.starts_with('~') {
        raw = expand_user_path(&raw);
    }
    let path = PathBuf::from(&raw);
    raw = if path.exists() {
        path.canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string())
    } else {
        std::path::absolute(&path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string())
    };

    let mut normalized = normalize_path_slashes(&raw);
    if let Some(converted) = normalize_project_mnt_drive(&normalized) {
        normalized = converted;
    }
    normalized = normalize_project_path_segments(&normalized);

    if normalized.len() >= 2
        && normalized.as_bytes()[0].is_ascii_alphabetic()
        && normalized.as_bytes()[1] == b':'
    {
        normalized = normalized[0..1].to_lowercase() + &normalized[1..];
        normalized = normalized.to_lowercase();
    }
    normalized
}

/// Compute the full CCB project id (64-char SHA256 hex).
/// Mirrors Python `project.ids.compute_project_id`.
pub fn compute_project_id(project_root: impl AsRef<str>) -> String {
    let normalized = normalize_project_path(project_root);
    if normalized.is_empty() {
        panic!("project_root cannot be empty");
    }
    sha256_hex(normalized.as_bytes())
}

/// Compute the project slug used for display and tmux session names.
/// Mirrors Python `project.ids.project_slug`.
pub fn project_slug(project_root: impl AsRef<str>) -> String {
    let normalized = normalize_project_path(project_root);
    let digest = &compute_project_id(&normalized)[..8];
    let base_name = std::path::Path::new(&normalized)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let slug: String = base_name
        .to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    let slug = if slug.is_empty() {
        "project".to_string()
    } else {
        slug
    };
    format!("{}-{}", slug, digest)
}

/// Compatibility wrapper for the v2 project id.
/// Mirrors Python `project.identity.compute_ccb_project_id`.
pub fn compute_ccb_project_id(work_dir: impl AsRef<str>) -> String {
    compute_project_id(resolve_project_root(work_dir).as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_id_deterministic_and_64_chars() {
        let a = compute_project_id("/home/user/project");
        let b = compute_project_id("/home/user/project");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn test_project_slug_contains_digest() {
        let slug = project_slug("/home/user/project");
        let digest = &compute_project_id("/home/user/project")[..8];
        assert!(slug.ends_with(digest));
    }

    #[test]
    fn test_normalize_work_dir_mnt_drive() {
        let normalized = normalize_work_dir("/mnt/C/Users/demo/repo");
        assert_eq!(normalized, "c:/Users/demo/repo");
    }

    #[test]
    fn test_compute_worktree_scope_id_12_chars() {
        let id = compute_worktree_scope_id("/home/user/project");
        assert_eq!(id.len(), 12);
    }
}
