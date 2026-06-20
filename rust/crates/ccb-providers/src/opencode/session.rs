use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccb_provider_core::contracts::ProviderSessionBinding;
use ccb_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use serde_json::Value;

pub const PROVIDER_NAME: &str = "opencode";
pub const SESSION_FILENAME: &str = ".opencode-session";

/// Build the OpenCode session binding.
pub fn build_session_binding() -> ProviderSessionBinding {
    ProviderSessionBinding {
        provider: PROVIDER_NAME.to_string(),
        session_id_attr: "opencode_session_id".to_string(),
        session_path_attr: "session_file".to_string(),
    }
}

/// An OpenCode project session loaded from disk.
#[derive(Debug, Clone)]
pub struct OpenCodeProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl OpenCodeProjectSession {
    pub fn ccb_session_id(&self) -> Option<&str> {
        self.data
            .get("ccb_session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
    }

    pub fn opencode_session_id(&self) -> Option<&str> {
        self.data
            .get("opencode_session_id")
            .or_else(|| self.data.get("opencode_storage_session_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    pub fn opencode_session_id_filter(&self) -> Option<String> {
        self.opencode_session_id().map(|s| s.to_string())
    }

    pub fn opencode_project_id(&self) -> Option<&str> {
        self.data
            .get("opencode_project_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
    }

    pub fn work_dir(&self) -> Option<&str> {
        self.data
            .get("work_dir")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
    }

    pub fn pane_id(&self) -> Option<&str> {
        self.data
            .get("pane_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
    }
}

/// Find the OpenCode session file for a work directory.
pub fn find_project_session_file(work_dir: &Path, instance: Option<&str>) -> Option<PathBuf> {
    let filename = session_filename_for_instance(SESSION_FILENAME, instance);
    find_session_file_for_work_dir(work_dir, &filename)
}

/// Load the OpenCode project session for a work directory.
pub fn load_project_session(
    work_dir: &Path,
    instance: Option<&str>,
) -> Option<OpenCodeProjectSession> {
    let session_file = find_project_session_file(work_dir, instance)?;
    let data = read_json(&session_file)?;
    if data.is_empty() {
        return None;
    }
    Some(OpenCodeProjectSession { session_file, data })
}

/// Load an OpenCode project session for an agent without falling back to the
/// primary session when the agent is named.
///
/// Mirrors Python `provider_backends.opencode.execution_runtime.helpers.load_session`.
pub fn load_session<F>(
    work_dir: &Path,
    agent_name: &str,
    primary_agent: &str,
    load_project_session_fn: F,
) -> Option<OpenCodeProjectSession>
where
    F: FnOnce(&Path, Option<&str>) -> Option<OpenCodeProjectSession>,
{
    let instance = ccb_provider_core::instance_resolution::named_agent_instance(
        agent_name,
        primary_agent,
    );
    load_project_session_fn(work_dir, instance.as_deref())
}

fn read_json(path: &Path) -> Option<HashMap<String, Value>> {
    let raw = std::fs::read_to_string(path).ok()?;
    // Strip UTF-8 BOM if present.
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let value: Value = serde_json::from_str(raw).ok()?;
    value
        .as_object()
        .cloned()
        .map(|obj| obj.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_json(dir: &Path, name: &str, content: Value) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, serde_json::to_string(&content).unwrap()).unwrap();
        path
    }

    #[test]
    fn test_session_binding_fields() {
        let binding = build_session_binding();
        assert_eq!(binding.provider, PROVIDER_NAME);
        assert_eq!(binding.session_id_attr, "opencode_session_id");
        assert_eq!(binding.session_path_attr, "session_file");
    }

    #[test]
    fn test_load_project_session() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_json(
            &work_dir,
            ".opencode-session",
            serde_json::json!({
                "opencode_session_id": "session-1",
                "opencode_project_id": "proj1",
                "work_dir": work_dir.to_string_lossy().to_string(),
                "pane_id": "%1",
            }),
        );
        let session = load_project_session(&work_dir, None).unwrap();
        assert_eq!(session.opencode_session_id(), Some("session-1"));
        assert_eq!(session.opencode_project_id(), Some("proj1"));
        assert_eq!(session.pane_id(), Some("%1"));
    }
}
