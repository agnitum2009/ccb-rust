use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccbr_provider_core::contracts::{
    LaunchMode, ProviderBackend, ProviderRuntimeLauncher, ProviderSessionBinding,
};
use ccbr_provider_core::manifest::ProviderManifest;
use ccbr_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use ccbr_provider_core::runtime_shared::provider_start_parts;
use serde_json::Value;

use crate::native_cli_support::{
    observe_stdout_output, NativeCliExecutionAdapter, NativeCliExecutionConfig,
    NativeCliExecutionRequest, OutputKind,
};
use crate::providers::pane_backed_manifest;

pub const PROVIDER_NAME: &str = "crush";

const SESSION_FILENAME: &str = ".crush-session";
const SESSION_ID_ATTR: &str = "crush_session_id";
const SESSION_PATH_ATTR: &str = "crush_session_path";

// ---------------------------------------------------------------------------
// Manifest / backend
// ---------------------------------------------------------------------------

/// Build the Crush provider manifest.
pub fn manifest() -> ProviderManifest {
    pane_backed_manifest(PROVIDER_NAME, false)
}

/// Build the Crush provider backend registration.
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

/// Build a generic native CLI execution adapter configured for Crush.
pub fn build_execution_adapter() -> NativeCliExecutionAdapter {
    NativeCliExecutionAdapter::new(
        NativeCliExecutionConfig::new(PROVIDER_NAME, _build_command)
            .with_observer(observe_stdout_output)
            .with_output_kind(OutputKind::Stdout)
            .with_reason("start_failed", "crush_run_start_failed")
            .with_reason("failed", "crush_run_failed")
            .with_reason("empty", "crush_empty_reply")
            .with_reason("run_error", "crush_run_error")
            .with_reason("complete", "crush_run_stop")
            .with_reason("process_exit_complete", "crush_run_exit")
            .with_reason("timeout", "crush_run_timeout"),
    )
}

fn _build_command(request: NativeCliExecutionRequest) -> Vec<String> {
    let data_dir = request.state_path("crush_data_dir", "data");
    let _ = std::fs::create_dir_all(&data_dir);
    let mut cmd = provider_start_parts(PROVIDER_NAME);
    cmd.push("--data-dir".to_string());
    cmd.push(data_dir.to_string_lossy().to_string());
    cmd.push("--cwd".to_string());
    cmd.push(request.work_dir.to_string_lossy().to_string());
    cmd.push("run".to_string());
    cmd.push("--quiet".to_string());
    cmd.push(request.prompt.clone());
    cmd
}

// ---------------------------------------------------------------------------
// Session helpers
// ---------------------------------------------------------------------------

/// A loaded Crush project session.
#[derive(Debug, Clone, Default)]
pub struct CrushProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl CrushProjectSession {
    pub fn crush_session_id(&self) -> String {
        self.data
            .get(SESSION_ID_ATTR)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn crush_session_path(&self) -> String {
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

/// Load a Crush project session.
pub fn load_project_session(
    work_dir: &Path,
    instance: Option<&str>,
) -> Option<CrushProjectSession> {
    let session_file = find_project_session_file(work_dir, instance)?;
    let raw = std::fs::read_to_string(&session_file).ok()?;
    let data: HashMap<String, Value> = serde_json::from_str(&raw).ok()?;
    Some(CrushProjectSession { session_file, data })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest() {
        let m = manifest();
        assert_eq!(m.provider, PROVIDER_NAME);
        assert!(m.supports_runtime_mode(&ccbr_provider_core::manifest::RuntimeMode::PaneBacked));
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
        std::fs::write(&session_path, r#"{"crush_session_id":"s1"}"#).unwrap();

        let session = load_project_session(tmp.path(), None).unwrap();
        assert_eq!(session.crush_session_id(), "s1");
        assert_eq!(session.session_file, session_path);
    }
}
