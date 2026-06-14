use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccb_provider_core::contracts::ProviderSessionBinding;
use ccb_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use serde_json::Value;

pub const PROVIDER_NAME: &str = "mimo";
pub const SESSION_FILENAME: &str = ".mimo-session";

/// Build the Mimo session binding.
pub fn build_session_binding() -> ProviderSessionBinding {
    ProviderSessionBinding {
        provider: PROVIDER_NAME.to_string(),
        session_id_attr: "mimo_session_id".to_string(),
        session_path_attr: "mimo_session_path".to_string(),
    }
}

/// A Mimo project session loaded from disk.
#[derive(Debug, Clone)]
pub struct MimoProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl MimoProjectSession {
    pub fn mimo_session_id(&self) -> String {
        self.data
            .get("mimo_session_id")
            .or_else(|| self.data.get("mimocode_session_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    pub fn mimo_session_path(&self) -> String {
        self.session_file.to_string_lossy().to_string()
    }

    pub fn mimo_project_id(&self) -> String {
        self.data
            .get("mimo_project_id")
            .or_else(|| self.data.get("mimocode_project_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    pub fn mimo_storage_root(&self) -> String {
        self.data
            .get("mimo_storage_root")
            .or_else(|| self.data.get("mimocode_storage_root"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    pub fn mimo_home(&self) -> String {
        self.data
            .get("mimo_home")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    pub fn mimo_config_path(&self) -> String {
        self.data
            .get("mimo_config_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    pub fn completion_artifact_dir(&self) -> String {
        self.data
            .get("completion_artifact_dir")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    pub fn pane_id(&self) -> Option<&str> {
        self.data
            .get("pane_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    pub fn work_dir(&self) -> Option<&str> {
        self.data
            .get("work_dir")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }
}

/// Find the Mimo session file for a work directory.
pub fn find_project_session_file(work_dir: &Path, instance: Option<&str>) -> Option<PathBuf> {
    let filename = session_filename_for_instance(SESSION_FILENAME, instance);
    find_session_file_for_work_dir(work_dir, &filename)
}

/// Load the Mimo project session for a work directory.
pub fn load_project_session(work_dir: &Path, instance: Option<&str>) -> Option<MimoProjectSession> {
    let session_file = find_project_session_file(work_dir, instance)?;
    let data = read_json(&session_file)?;
    if data.is_empty() {
        return None;
    }
    Some(MimoProjectSession { session_file, data })
}

fn read_json(path: &Path) -> Option<HashMap<String, Value>> {
    let raw = std::fs::read_to_string(path).ok()?;
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let value: Value = serde_json::from_str(raw).ok()?;
    value
        .as_object()
        .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
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
        assert_eq!(binding.session_id_attr, "mimo_session_id");
        assert_eq!(binding.session_path_attr, "mimo_session_path");
    }

    #[test]
    fn test_load_project_session() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_json(
            &work_dir,
            ".mimo-session",
            serde_json::json!({
                "mimo_session_id": "session-1",
                "pane_id": "%1",
                "work_dir": work_dir.to_string_lossy().to_string(),
            }),
        );
        let session = load_project_session(&work_dir, None).unwrap();
        assert_eq!(session.mimo_session_id(), "session-1");
        assert_eq!(session.pane_id(), Some("%1"));
    }
}
