//! Mirrors Python `lib/provider_backends/claude/home_layout.py`.

use camino::{Utf8Path, Utf8PathBuf};

/// Paths that make up an isolated Claude "home" directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeHomeLayout {
    pub home_root: Utf8PathBuf,
    pub claude_dir: Utf8PathBuf,
    pub projects_root: Utf8PathBuf,
    pub session_env_root: Utf8PathBuf,
    pub trust_path: Utf8PathBuf,
    pub settings_path: Utf8PathBuf,
    pub auth_path: Utf8PathBuf,
    pub credentials_path: Utf8PathBuf,
}

/// Build a layout from a home-root path.
pub fn claude_layout_for_home(home_root: impl AsRef<Utf8Path>) -> ClaudeHomeLayout {
    let root = expand_user_path(home_root.as_ref());
    let claude_dir = root.join(".claude");
    ClaudeHomeLayout {
        home_root: root.clone(),
        projects_root: claude_dir.join("projects"),
        session_env_root: claude_dir.join("session-env"),
        trust_path: root.join(".claude.json"),
        settings_path: claude_dir.join("settings.json"),
        auth_path: root.join(".config").join("claude-code").join("auth.json"),
        credentials_path: claude_dir.join(".credentials.json"),
        claude_dir,
    }
}

/// Resolve the current user's real Claude home root from environment or `HOME`.
pub fn current_claude_home_root() -> Utf8PathBuf {
    if let Some(root) = env_projects_root().and_then(home_root_from_projects_root) {
        return root;
    }
    expand_user_path(Utf8Path::new("~"))
}

/// Current Claude projects root.
pub fn current_claude_projects_root() -> Utf8PathBuf {
    match env_projects_root() {
        Some(root) => root,
        None => claude_layout_for_home(current_claude_home_root()).projects_root,
    }
}

/// Current Claude session-env root.
pub fn current_claude_session_env_root() -> Utf8PathBuf {
    claude_layout_for_home(current_claude_home_root()).session_env_root
}

/// Try to recover a layout from session data fields.
pub fn claude_layout_from_session_data(
    data: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<ClaudeHomeLayout> {
    let data = data?;
    let home_root = data
        .get("claude_home")
        .and_then(path_or_none)
        .or_else(|| {
            data.get("claude_projects_root")
                .and_then(path_or_none)
                .and_then(home_root_from_projects_root)
        })
        .or_else(|| {
            data.get("claude_session_env_root")
                .and_then(path_or_none)
                .and_then(home_root_from_session_env_root)
        })
        .or_else(|| {
            data.get("claude_session_path")
                .and_then(path_or_none)
                .and_then(home_root_from_session_path)
        })?;
    Some(claude_layout_for_home(home_root))
}

fn env_projects_root() -> Option<Utf8PathBuf> {
    let raw = std::env::var("CLAUDE_PROJECTS_ROOT")
        .or_else(|_| std::env::var("CLAUDE_PROJECT_ROOT"))
        .unwrap_or_default();
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    Some(expand_user_path(Utf8Path::new(raw)))
}

fn home_root_from_projects_root(projects_root: Utf8PathBuf) -> Option<Utf8PathBuf> {
    if projects_root.file_name()? != "projects" {
        return None;
    }
    let parent = projects_root.parent()?;
    if parent.file_name()? == ".claude" {
        Some(parent.parent()?.to_path_buf())
    } else {
        None
    }
}

fn home_root_from_session_env_root(session_env_root: Utf8PathBuf) -> Option<Utf8PathBuf> {
    if session_env_root.file_name()? != "session-env" {
        return None;
    }
    let parent = session_env_root.parent()?;
    if parent.file_name()? == ".claude" {
        Some(parent.parent()?.to_path_buf())
    } else {
        None
    }
}

fn home_root_from_session_path(session_path: Utf8PathBuf) -> Option<Utf8PathBuf> {
    let mut current = Some(session_path.as_path());
    while let Some(parent) = current {
        if parent.file_name() == Some(".claude") {
            return parent.parent().map(|p| p.to_path_buf());
        }
        current = parent.parent();
    }
    None
}

fn path_or_none(value: &serde_json::Value) -> Option<Utf8PathBuf> {
    let raw = value.as_str()?.trim();
    if raw.is_empty() {
        return None;
    }
    Some(expand_user_path(Utf8Path::new(raw)))
}

fn expand_user_path(path: &Utf8Path) -> Utf8PathBuf {
    let s = path.as_str();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return Utf8PathBuf::from(home).join(rest);
        }
    }
    if s == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return Utf8PathBuf::from(home);
        }
    }
    path.to_path_buf()
}
