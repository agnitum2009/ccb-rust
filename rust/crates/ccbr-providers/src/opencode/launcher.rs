use std::collections::HashMap;
use std::path::Path;

use ccbr_memory::{materialize_runtime_memory_bundle, runtime_memory_bundle_relative_path};
use ccbr_provider_core::contracts::{LaunchMode, ProviderRuntimeLauncher};
use ccbr_provider_core::runtime_shared::{apply_provider_command_template, provider_start_parts};
use serde_json::Value;

pub const PROVIDER_NAME: &str = "opencode";
pub const OPENCODE_CONFIG_FILENAME: &str = "opencode.json";

/// Build the OpenCode runtime launcher.
pub fn build_runtime_launcher() -> ProviderRuntimeLauncher {
    ProviderRuntimeLauncher::new(PROVIDER_NAME, LaunchMode::SimpleTmux)
}

/// Launch context prepared before building the start command.
#[derive(Debug, Clone, Default)]
pub struct OpenCodeLaunchContext {
    pub agent_name: String,
    pub project_root: String,
    pub workspace_path: String,
    pub agent_events_path: String,
    pub opencode_config_path: String,
}

/// Prepare the launch context for an agent.
pub fn prepare_launch_context(
    project_root: &Path,
    agent_name: &str,
    workspace_path: &Path,
    agent_events_path: &Path,
    runtime_dir: &Path,
) -> OpenCodeLaunchContext {
    let config_dir = runtime_dir.join("opencode");
    let _ = std::fs::create_dir_all(&config_dir);
    OpenCodeLaunchContext {
        agent_name: agent_name.to_string(),
        project_root: project_root.to_string_lossy().to_string(),
        workspace_path: workspace_path.to_string_lossy().to_string(),
        agent_events_path: agent_events_path.to_string_lossy().to_string(),
        opencode_config_path: config_dir
            .join(OPENCODE_CONFIG_FILENAME)
            .to_string_lossy()
            .to_string(),
    }
}

/// Build the OpenCode start command.
pub fn build_start_cmd(
    restore: bool,
    startup_args: &[String],
    command_template: Option<&str>,
) -> String {
    let mut cmd_parts = provider_start_parts(PROVIDER_NAME);
    if restore {
        cmd_parts.push("--continue".to_string());
    }
    cmd_parts.extend(startup_args.iter().cloned());
    let cmd = shlex_join(&cmd_parts);
    apply_provider_command_template(&cmd, command_template).unwrap_or(cmd)
}

/// Result of materializing the OpenCode memory config.
#[derive(Debug, Clone, Default)]
pub struct OpenCodeMemoryConfigResult {
    pub env: HashMap<String, String>,
}

/// Materialize the OpenCode memory config file.
///
/// Mirrors Python `materialize_opencode_memory_config`. Minimal parity
/// implementation: builds a simple memory bundle and writes `opencode.json`.
#[allow(clippy::too_many_arguments)]
pub fn materialize_opencode_memory_config(
    project_root: &Path,
    agent_name: &str,
    workspace_path: Option<&Path>,
    config_path: Option<&Path>,
    profile: Option<&ccbr_provider_profiles::models::ResolvedProviderProfile>,
    event_path: Option<&Path>,
    marker_path: Option<&Path>,
) -> OpenCodeMemoryConfigResult {
    let Some(config_path) = config_path else {
        let result = ccbr_provider_core::memory_projection::memory_projection_result(
            "failed",
            "missing_config_path",
            Path::new(""),
            None,
            None,
            None,
            None,
        );
        let _ = ccbr_provider_core::memory_projection::record_memory_projection_event(
            &result,
            "opencode",
            event_path,
            marker_path,
            Some(agent_name),
        );
        return OpenCodeMemoryConfigResult::default();
    };

    if !opencode_inherits_memory(profile) {
        let _ = std::fs::remove_file(config_path);
        let result = ccbr_provider_core::memory_projection::memory_projection_result(
            "skipped",
            "inherit_memory_disabled",
            Path::new(""),
            None,
            None,
            None,
            None,
        );
        let _ = ccbr_provider_core::memory_projection::record_memory_projection_event(
            &result,
            "opencode",
            event_path,
            marker_path,
            Some(agent_name),
        );
        return OpenCodeMemoryConfigResult::default();
    }

    let bundle_result = match materialize_runtime_memory_bundle(
        project_root,
        agent_name,
        "opencode",
        workspace_path,
        None,
    ) {
        Ok(r) => r,
        Err(err) => {
            let result = ccbr_provider_core::memory_projection::memory_projection_result(
                "failed",
                "bundle_render_failed",
                Path::new(""),
                None,
                None,
                None,
                Some(&err.to_string()),
            );
            let _ = ccbr_provider_core::memory_projection::record_memory_projection_event(
                &result,
                "opencode",
                event_path,
                marker_path,
                Some(agent_name),
            );
            return OpenCodeMemoryConfigResult::default();
        }
    };

    let bundle_text = match std::fs::read_to_string(&bundle_result.path) {
        Ok(text) => text,
        Err(err) => {
            let result = ccbr_provider_core::memory_projection::memory_projection_result(
                "failed",
                "bundle_read_failed",
                &bundle_result.path,
                None,
                None,
                None,
                Some(&err.to_string()),
            );
            let _ = ccbr_provider_core::memory_projection::record_memory_projection_event(
                &result,
                "opencode",
                event_path,
                marker_path,
                Some(agent_name),
            );
            return OpenCodeMemoryConfigResult::default();
        }
    };

    let bundle_relative = runtime_memory_bundle_relative_path(project_root, agent_name)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| bundle_result.path.to_string_lossy().to_string());

    let config_text = match serde_json::to_string_pretty(&serde_json::json!({
        "instructions": ["AGENTS.md", bundle_relative],
        "memory": {
            "instruction": bundle_text,
        }
    })) {
        Ok(t) => t,
        Err(err) => {
            let result = ccbr_provider_core::memory_projection::memory_projection_result(
                "failed",
                "config_render_failed",
                Path::new(""),
                None,
                None,
                None,
                Some(&err.to_string()),
            );
            let _ = ccbr_provider_core::memory_projection::record_memory_projection_event(
                &result,
                "opencode",
                event_path,
                marker_path,
                Some(agent_name),
            );
            return OpenCodeMemoryConfigResult::default();
        }
    };

    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let write_res = std::fs::write(config_path, config_text.as_bytes());

    if let Err(err) = write_res {
        let result = ccbr_provider_core::memory_projection::memory_projection_result(
            "failed",
            "config_write_failed",
            config_path,
            None,
            None,
            None,
            Some(&err.to_string()),
        );
        let _ = ccbr_provider_core::memory_projection::record_memory_projection_event(
            &result,
            "opencode",
            event_path,
            marker_path,
            Some(agent_name),
        );
        return OpenCodeMemoryConfigResult::default();
    }

    let sha = ccbr_provider_core::memory_projection::text_file_sha256(config_path);
    let sha_ref = if sha.is_empty() {
        None
    } else {
        Some(sha.as_str())
    };
    let result = ccbr_provider_core::memory_projection::memory_projection_result(
        "ok",
        "written",
        config_path,
        sha_ref,
        Some(1),
        None,
        None,
    );
    let _ = ccbr_provider_core::memory_projection::record_memory_projection_event(
        &result,
        "opencode",
        event_path,
        marker_path,
        Some(agent_name),
    );

    let mut env = HashMap::new();
    env.insert(
        "OPENCODE_CONFIG".into(),
        config_path.to_string_lossy().to_string(),
    );
    OpenCodeMemoryConfigResult { env }
}

fn opencode_inherits_memory(
    profile: Option<&ccbr_provider_profiles::models::ResolvedProviderProfile>,
) -> bool {
    profile.map(|p| p.inherit_memory).unwrap_or(true)
}

/// Build the session payload written to the session file.
pub fn build_session_payload(
    launch_context: &OpenCodeLaunchContext,
    runtime_dir: &Path,
    run_cwd: &Path,
    pane_id: &str,
    pane_title_marker: &str,
    start_cmd: &str,
    launch_session_id: &str,
) -> HashMap<String, Value> {
    let mut payload = HashMap::new();
    payload.insert(
        "ccbr_session_id".to_string(),
        Value::String(launch_session_id.to_string()),
    );
    payload.insert(
        "agent_name".to_string(),
        Value::String(launch_context.agent_name.clone()),
    );
    payload.insert(
        "runtime_dir".to_string(),
        Value::String(runtime_dir.to_string_lossy().to_string()),
    );
    payload.insert(
        "completion_artifact_dir".to_string(),
        Value::String(runtime_dir.join("completion").to_string_lossy().to_string()),
    );
    payload.insert("terminal".to_string(), Value::String("tmux".to_string()));
    payload.insert(
        "tmux_session".to_string(),
        Value::String(pane_id.to_string()),
    );
    payload.insert("pane_id".to_string(), Value::String(pane_id.to_string()));
    payload.insert(
        "pane_title_marker".to_string(),
        Value::String(pane_title_marker.to_string()),
    );
    payload.insert(
        "workspace_path".to_string(),
        Value::String(launch_context.workspace_path.clone()),
    );
    payload.insert(
        "work_dir".to_string(),
        Value::String(run_cwd.to_string_lossy().to_string()),
    );
    payload.insert(
        "start_dir".to_string(),
        Value::String(launch_context.project_root.clone()),
    );
    payload.insert(
        "start_cmd".to_string(),
        Value::String(start_cmd.to_string()),
    );
    payload.insert(
        "opencode_config_path".to_string(),
        Value::String(launch_context.opencode_config_path.clone()),
    );
    payload
}

fn shlex_join(parts: &[String]) -> String {
    parts
        .iter()
        .map(|p| {
            if p.is_empty()
                || p.chars()
                    .any(|c| c.is_whitespace() || c == '\'' || c == '"')
            {
                format!("'{}'", p.replace('\'', "'\"'\"'"))
            } else {
                p.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_build_start_cmd() {
        let cmd = build_start_cmd(false, &[], None);
        assert!(cmd.contains("opencode"));
        let cmd_restore = build_start_cmd(true, &[], None);
        assert!(cmd_restore.contains("--continue"));
    }

    #[test]
    fn test_build_session_payload() {
        let tmp = TempDir::new().unwrap();
        let ctx = OpenCodeLaunchContext {
            agent_name: "agent1".to_string(),
            project_root: "/project".to_string(),
            workspace_path: "/workspace".to_string(),
            agent_events_path: "/events".to_string(),
            opencode_config_path: "/config/opencode.json".to_string(),
        };
        let payload = build_session_payload(
            &ctx,
            tmp.path(),
            Path::new("/run"),
            "%1",
            "CCBR-agent1-proj",
            "opencode",
            "launch-1",
        );
        assert_eq!(payload.get("agent_name").unwrap(), "agent1");
        assert_eq!(payload.get("pane_id").unwrap(), "%1");
        assert_eq!(payload.get("ccbr_session_id").unwrap(), "launch-1");
    }
}
