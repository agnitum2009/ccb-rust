use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccb_provider_core::contracts::{
    LaunchMode, ProviderBackend, ProviderRuntimeLauncher, ProviderSessionBinding,
};
use ccb_provider_core::manifest::ProviderManifest;
use ccb_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use ccb_provider_core::runtime_shared::provider_start_parts;
use serde_json::Value;

use crate::native_cli_support::{
    NativeCliExecutionAdapter, NativeCliExecutionConfig, NativeCliExecutionRequest, OutputKind,
};
use crate::providers::pane_backed_manifest;

pub const PROVIDER_NAME: &str = "cursor";

const SESSION_FILENAME: &str = ".cursor-session";
const SESSION_ID_ATTR: &str = "cursor_session_id";
const SESSION_PATH_ATTR: &str = "cursor_session_path";

// ---------------------------------------------------------------------------
// Manifest / backend
// ---------------------------------------------------------------------------

/// Build the Cursor provider manifest.
pub fn manifest() -> ProviderManifest {
    pane_backed_manifest(PROVIDER_NAME, false)
}

/// Build the Cursor provider backend registration.
pub fn backend() -> ProviderBackend {
    ProviderBackend {
        manifest: manifest(),
        execution_adapter: None,
        session_binding: Some(ProviderSessionBinding {
            provider: PROVIDER_NAME.to_string(),
            session_id_attr: SESSION_ID_ATTR.to_string(),
            session_path_attr: SESSION_PATH_ATTR.to_string(),
        }),
        runtime_launcher: Some(ProviderRuntimeLauncher {
            provider: PROVIDER_NAME.to_string(),
            launch_mode: LaunchMode::SimpleTmux,
        }),
    }
}

// ---------------------------------------------------------------------------
// Native CLI execution adapter
// ---------------------------------------------------------------------------

/// Build a generic native CLI execution adapter configured for Cursor.
pub fn build_execution_adapter() -> NativeCliExecutionAdapter {
    NativeCliExecutionAdapter::new(
        NativeCliExecutionConfig::new(PROVIDER_NAME, _build_command)
            .with_env_builder(_build_env)
            .with_output_kind(OutputKind::Jsonl)
            .with_reason("start_failed", "cursor_run_start_failed")
            .with_reason("failed", "cursor_run_failed")
            .with_reason("empty", "cursor_empty_reply")
            .with_reason("run_error", "cursor_run_error")
            .with_reason("complete", "cursor_run_stop")
            .with_reason("process_exit_complete", "cursor_run_exit")
            .with_reason("timeout", "cursor_run_timeout"),
    )
}

fn _build_command(request: NativeCliExecutionRequest) -> Vec<String> {
    let mut cmd = provider_start_parts(PROVIDER_NAME);
    cmd.push("--print".to_string());
    cmd.push("--output-format".to_string());
    cmd.push("stream-json".to_string());
    cmd.push("--workspace".to_string());
    cmd.push(request.work_dir.to_string_lossy().to_string());
    cmd.push("--trust".to_string());
    cmd.push(request.prompt.clone());
    cmd
}

fn _build_env(request: &NativeCliExecutionRequest) -> HashMap<String, String> {
    let cursor_home = request.state_path("cursor_home", "home");
    let _ = std::fs::create_dir_all(&cursor_home);
    let mut env = HashMap::new();
    env.insert(
        "HOME".to_string(),
        cursor_home.to_string_lossy().to_string(),
    );
    env
}

// ---------------------------------------------------------------------------
// Session helpers
// ---------------------------------------------------------------------------

/// A loaded Cursor project session.
#[derive(Debug, Clone, Default)]
pub struct CursorProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl CursorProjectSession {
    pub fn cursor_session_id(&self) -> String {
        self.data
            .get(SESSION_ID_ATTR)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn cursor_session_path(&self) -> String {
        self.data
            .get(SESSION_PATH_ATTR)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }
}

/// Find a project session file for a work directory.
pub fn find_project_session_file(work_dir: &Path, instance: Option<&str>) -> Option<PathBuf> {
    let filename = session_filename_for_instance(SESSION_FILENAME, instance);
    find_session_file_for_work_dir(work_dir, &filename)
}

/// Load a Cursor project session.
pub fn load_project_session(
    work_dir: &Path,
    instance: Option<&str>,
) -> Option<CursorProjectSession> {
    let session_file = find_project_session_file(work_dir, instance)?;
    let raw = std::fs::read_to_string(&session_file).ok()?;
    let data: HashMap<String, Value> = serde_json::from_str(&raw).ok()?;
    Some(CursorProjectSession { session_file, data })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest() {
        let m = manifest();
        assert_eq!(m.provider, PROVIDER_NAME);
        assert!(m.supports_runtime_mode(&ccb_provider_core::manifest::RuntimeMode::PaneBacked));
    }

    #[test]
    fn test_backend_has_session_binding_and_launcher() {
        let b = backend();
        assert_eq!(b.provider(), PROVIDER_NAME);
        assert!(b.session_binding.is_some());
        assert!(b.runtime_launcher.is_some());
    }

    #[test]
    fn test_build_execution_adapter_provider_name() {
        let adapter = build_execution_adapter();
        assert_eq!(adapter.provider(), PROVIDER_NAME);
    }

    #[test]
    fn test_load_project_session() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session_path = tmp.path().join(SESSION_FILENAME);
        std::fs::write(&session_path, r#"{"cursor_session_id":"s1"}"#).unwrap();

        let session = load_project_session(tmp.path(), None).unwrap();
        assert_eq!(session.cursor_session_id(), "s1");
        assert_eq!(session.session_file, session_path);
    }
}
