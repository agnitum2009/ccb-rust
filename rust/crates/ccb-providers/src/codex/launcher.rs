//! Codex runtime launcher.
//!
//! Mirrors Python `lib/provider_backends/codex/launcher.py`.

use std::path::Path;

use camino::Utf8Path;
use ccb_provider_core::contracts::{LaunchMode, ProviderRuntimeLauncher};
use ccb_provider_profiles::materializer::load_resolved_provider_profile;
use serde_json::Value;

use crate::codex::launcher_runtime::command::{
    build_start_cmd as build_start_cmd_impl, CodexLaunchContext, CodexStartCommand,
};
use crate::codex::launcher_runtime::home::resolve_codex_home_layout;

pub use crate::codex::launcher_runtime::command::build_codex_shell_prefix;
pub use crate::codex::launcher_runtime::home::prepare_codex_home_overrides as prepare_codex_home_overrides_for_test;
pub use crate::codex::launcher_runtime::session_paths::{
    load_resume_session_id as load_resume_session_id_for_test, session_file_for_runtime_dir,
};

/// Build the Codex runtime launcher descriptor.
pub fn build_runtime_launcher() -> ProviderRuntimeLauncher {
    ProviderRuntimeLauncher::new("codex", LaunchMode::CodexTmux)
}

/// Prepare a minimal runtime directory payload.
pub fn prepare_runtime(_runtime_dir: &Path) -> Value {
    serde_json::json!({})
}

/// Prepare the launch context for an agent.
pub fn prepare_launch_context(
    project_root: &Path,
    spec_name: &str,
    workspace_path: &Path,
    agent_events_path: &Path,
    _runtime_dir: &Path,
    prepared_state: Option<&Value>,
) -> CodexLaunchContext {
    let run_cwd = prepared_state
        .and_then(|v| v.get("run_cwd"))
        .and_then(|v| v.as_str())
        .map(Path::new)
        .unwrap_or(workspace_path);
    CodexLaunchContext {
        agent_name: spec_name.to_string(),
        project_root: project_root.to_string_lossy().to_string(),
        workspace_path: run_cwd.to_string_lossy().to_string(),
        agent_events_path: agent_events_path.to_string_lossy().to_string(),
    }
}

/// Build the shell command that launches Codex in a tmux pane.
#[allow(clippy::too_many_arguments)]
pub fn build_start_cmd(
    command: &CodexStartCommand,
    spec: &ccb_agents::models::AgentSpec,
    runtime_dir: &Utf8Path,
    launch_session_id: &str,
    prepared_state: Option<&CodexLaunchContext>,
) -> anyhow::Result<String> {
    let profile = load_resolved_provider_profile(runtime_dir);
    build_start_cmd_impl(
        command,
        spec,
        runtime_dir,
        launch_session_id,
        prepared_state,
        profile.as_ref(),
    )
}

/// Build the session payload persisted for the Codex bridge.
#[allow(clippy::too_many_arguments)]
pub fn build_session_payload(
    context: &CodexLaunchContext,
    runtime_dir: &Utf8Path,
    workspace_path: &Path,
    pane_id: &str,
    pane_title_marker: &str,
    start_cmd: &str,
    launch_session_id: &str,
) -> Value {
    let layout = resolve_codex_home_layout(runtime_dir, None);
    let mut payload = serde_json::json!({
        "ccb_session_id": launch_session_id,
        "agent_name": context.agent_name,
        "runtime_dir": runtime_dir.as_str(),
        "input_fifo": runtime_dir.join("input.fifo").as_str(),
        "output_fifo": runtime_dir.join("output.fifo").as_str(),
        "terminal": "tmux",
        "tmux_session": pane_id,
        "pane_id": pane_id,
        "pane_title_marker": pane_title_marker,
        "tmux_log": runtime_dir.join("bridge.log").as_str(),
        "bridge_log": runtime_dir.join("bridge.log").as_str(),
        "workspace_path": workspace_path.to_string_lossy(),
        "work_dir": context.workspace_path,
        "start_dir": context.project_root,
        "codex_start_cmd": start_cmd,
        "start_cmd": start_cmd,
        "codex_session_root": layout.session_root.as_str(),
    });
    if layout.codex_home.as_str() != layout.session_root.as_str() {
        payload["codex_home"] = layout.codex_home.as_str().into();
    }
    payload
}

/// Post-launch hook for Codex.
pub fn post_launch(
    _backend: &dyn std::any::Any,
    _pane_id: &str,
    _runtime_dir: &Utf8Path,
    _launch_session_id: &str,
    _prepared_state: &CodexLaunchContext,
) {
    // TODO: align with Python `post_launch`.
}
