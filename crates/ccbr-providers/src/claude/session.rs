use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccbr_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use serde_json::{Map, Value};

use crate::claude::registry_runtime::state::RegistrySession;

const CLAUDE_SESSION_FILENAME: &str = ".claude-session";
const SESSION_ID_ATTR: &str = "claude_session_id";
const SESSION_PATH_ATTR: &str = "claude_session_path";

fn now_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

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

    pub fn work_dir(&self) -> PathBuf {
        self.data
            .get("work_dir")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                self.session_file
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            })
    }

    pub fn start_cmd(&self) -> String {
        self.data
            .get("claude_start_cmd")
            .or_else(|| self.data.get("start_cmd"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string()
    }

    /// Update the claude session binding and persist the session file.
    /// Mirrors Python `provider_backends.claude.session_runtime.lifecycle_runtime.update_claude_binding`.
    pub fn update_claude_binding(
        &mut self,
        session_path: Option<&Path>,
        session_id: Option<&str>,
    ) -> bool {
        let old_path = self
            .data
            .get(SESSION_PATH_ATTR)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let old_id = self
            .data
            .get(SESSION_ID_ATTR)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let new_path = session_path
            .map(|p| {
                if let Some(s) = p.to_str() {
                    expand_tilde(s)
                } else {
                    p.to_string_lossy().to_string()
                }
            })
            .filter(|s| !s.is_empty());
        let new_id = session_id
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                new_path.as_ref().and_then(|p| {
                    Path::new(p)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                })
            });

        let path_changed = new_path
            .as_ref()
            .map(|p| self.data.get(SESSION_PATH_ATTR) != Some(&Value::String(p.clone())))
            .unwrap_or(false);
        let id_changed = new_id
            .as_ref()
            .map(|id| self.data.get(SESSION_ID_ATTR) != Some(&Value::String(id.clone())))
            .unwrap_or(false);
        if !path_changed && !id_changed {
            return false;
        }

        if let Some(ref path) = new_path {
            self.data
                .insert(SESSION_PATH_ATTR.to_string(), Value::String(path.clone()));
        }
        if let Some(ref id) = new_id {
            self.data
                .insert(SESSION_ID_ATTR.to_string(), Value::String(id.clone()));
        }
        if !old_id.is_empty() && new_id.as_ref().map(|id| id != &old_id).unwrap_or(true) {
            self.data.insert(
                "old_claude_session_id".to_string(),
                Value::String(old_id.clone()),
            );
        }
        if !old_path.is_empty() && new_path.as_ref().map(|p| p != &old_path).unwrap_or(true) {
            self.data.insert(
                "old_claude_session_path".to_string(),
                Value::String(old_path.clone()),
            );
        }
        if (!old_path.is_empty() || !old_id.is_empty()) && (new_path.is_some() || new_id.is_some())
        {
            self.data
                .insert("old_updated_at".to_string(), Value::String(now_str()));
        }
        self.data
            .insert("updated_at".to_string(), Value::String(now_str()));
        if self.data.get("active") == Some(&Value::Bool(false)) {
            self.data.insert("active".to_string(), Value::Bool(true));
        }

        let mut payload: Map<String, Value> = self.data.clone().into_iter().collect();
        crate::claude::registry_support::pathing::ensure_claude_session_work_dir_fields(
            &mut payload,
            &self.session_file,
        );
        self.data = payload.into_iter().collect();

        let content = serde_json::to_string_pretty(&self.data).unwrap_or_default() + "\n";
        let _ = ccbr_provider_sessions::safe_write_session(&self.session_file, &content);
        true
    }
}

/// Find a project session file for a work directory.
/// Mirrors Python `provider_backends.claude.session_runtime.pathing.find_project_session_file`.
pub fn find_project_session_file(work_dir: &Path, instance: Option<&str>) -> Option<PathBuf> {
    let filename = session_filename_for_instance(CLAUDE_SESSION_FILENAME, instance);
    find_session_file_for_work_dir(work_dir, &filename)
}

/// Load a Claude project session for an agent without falling back to the
/// primary session when the agent is named.
///
/// Mirrors Python `provider_backends.claude.execution_runtime.start.load_session`.
pub fn load_session<F>(
    load_project_session_fn: F,
    work_dir: &Path,
    agent_name: &str,
) -> Option<ClaudeProjectSession>
where
    F: FnOnce(&Path, Option<&str>) -> Option<ClaudeProjectSession>,
{
    let instance =
        ccbr_provider_core::instance_resolution::named_agent_instance(agent_name, "claude");
    load_project_session_fn(work_dir, instance.as_deref())
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

impl RegistrySession for ClaudeProjectSession {
    fn session_file(&self) -> Option<&Path> {
        Some(&self.session_file)
    }

    fn ensure_pane(&self) -> Result<String, String> {
        self.pane_id()
            .map(|s| s.to_string())
            .ok_or_else(|| "no pane identity available".to_string())
    }
}

fn expand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    input.to_string()
}
