use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccb_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use serde_json::Value;

const CLAUDE_SESSION_FILENAME: &str = ".claude-session";
const SESSION_ID_ATTR: &str = "claude_session_id";
const SESSION_PATH_ATTR: &str = "claude_session_path";

/// A loaded Claude project session.
///
/// Mirrors Python `provider_backends.claude.session_runtime.model.ClaudeProjectSession`.
#[derive(Debug, Clone, Default)]
pub struct ClaudeProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl ClaudeProjectSession {
    pub fn pane_id(&self) -> Option<&str> {
        self.data
            .get("pane_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                self.data
                    .get("tmux_session")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
            })
    }

    pub fn tmux_socket_name(&self) -> Option<&str> {
        self.data
            .get("tmux_socket_name")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    }

    pub fn tmux_socket_path(&self) -> Option<&str> {
        self.data
            .get("tmux_socket_path")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    }

    pub fn claude_session_id(&self) -> Option<&str> {
        self.data
            .get(SESSION_ID_ATTR)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    }

    pub fn claude_session_path(&self) -> Option<&str> {
        self.data
            .get(SESSION_PATH_ATTR)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    }

    pub fn claude_projects_root(&self) -> Option<PathBuf> {
        self.data
            .get("claude_projects_root")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(|s| PathBuf::from(expand_tilde(s)))
    }

    pub fn runtime_dir(&self) -> Option<PathBuf> {
        self.data
            .get("runtime_dir")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(|| self.session_file.parent().map(Path::to_path_buf))
    }

    pub fn completion_dir(&self) -> Option<PathBuf> {
        let explicit = self
            .data
            .get("completion_artifact_dir")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        if let Some(dir) = explicit {
            return Some(dir);
        }
        self.runtime_dir().map(|dir| dir.join("completion"))
    }
}

/// Find a project session file for a work directory.
/// Mirrors Python `provider_backends.claude.session_runtime.pathing.find_project_session_file`.
pub fn find_project_session_file(work_dir: &Path, instance: Option<&str>) -> Option<PathBuf> {
    let filename = session_filename_for_instance(CLAUDE_SESSION_FILENAME, instance);
    find_session_file_for_work_dir(work_dir, &filename)
}

/// Load a Claude project session.
/// Mirrors Python `provider_backends.claude.session.load_project_session`.
pub fn load_project_session(
    work_dir: &Path,
    instance: Option<&str>,
) -> Option<ClaudeProjectSession> {
    if let Some(inst) = instance {
        let session_file = find_project_session_file(work_dir, Some(inst))?;
        return load_session_from_file(session_file);
    }

    let session_file = find_project_session_file(work_dir, None)?;
    load_session_from_file(session_file)
}

fn load_session_from_file(session_file: PathBuf) -> Option<ClaudeProjectSession> {
    let raw = std::fs::read_to_string(&session_file).ok()?;
    let data: HashMap<String, Value> = serde_json::from_str(&raw).ok()?;
    Some(ClaudeProjectSession { session_file, data })
}

fn expand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    input.to_string()
}
