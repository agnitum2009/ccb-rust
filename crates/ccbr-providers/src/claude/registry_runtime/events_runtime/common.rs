//! Mirrors Python `lib/provider_backends/claude/registry_runtime/events_runtime/common.py`.

use std::path::Path;

use crate::claude::registry::{find_claude_session_file, load_claude_session};
use crate::claude::registry_runtime::session_updates::update_session_file_direct;
use crate::claude::registry_runtime::state::{ClaudeRuntimeRegistry, SessionEntry};
use crate::claude::session::ClaudeProjectSession;

/// Try to update the session binding on a session object.
pub fn safe_update_binding(
    session: Option<&mut ClaudeProjectSession>,
    session_path: &Path,
    session_id: &str,
) -> bool {
    if let Some(session) = session {
        session.update_claude_binding(Some(session_path), Some(session_id));
        true
    } else {
        false
    }
}

/// Update the on-disk session file for a work directory.
pub fn update_session_file<W>(
    _registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    work_dir: &Path,
    session_path: &Path,
    session_id: &str,
) {
    if let Some(session_file) = find_claude_session_file(work_dir) {
        let _ = update_session_file_direct(&session_file, session_path, session_id);
    }
}

/// Load the session object for a registry entry, caching it on the entry.
pub fn load_session_for_entry<'a, W>(
    _registry: &'a ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    entry: &'a mut SessionEntry<ClaudeProjectSession>,
) -> Option<&'a ClaudeProjectSession> {
    if entry.session.is_some() {
        return entry.session.as_ref();
    }
    let session = load_claude_session(&entry.work_dir)?;
    entry.session_file = Some(session.session_file.clone());
    entry.session = Some(session);
    entry.session.as_ref()
}

/// Return registry keys watched under a project key.
pub fn watcher_keys<W>(
    registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    project_key: &str,
) -> Vec<String> {
    let state = registry.state.lock().unwrap();
    state
        .watchers
        .get(project_key)
        .map(|watcher| watcher.keys.iter().cloned().collect())
        .unwrap_or_default()
}

/// Ensure a watcher entry exists for a project key.
pub fn ensure_watcher<W: Default>(
    registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    project_key: &str,
) {
    let mut state = registry.state.lock().unwrap();
    state.watchers.entry(project_key.to_string()).or_default();
}
