use std::collections::HashMap;
use std::path::Path;

use ccbr_provider_core::caller_env::{
    caller_context_env, export_env_clause, join_env_prefix, provider_user_session_env,
};
use ccbr_provider_core::contracts::{LaunchMode, ProviderRuntimeLauncher};
use ccbr_provider_core::runtime_shared::{apply_provider_command_template, provider_start_parts};
use serde_json::Value;

pub const PROVIDER_NAME: &str = "mimo";

/// Build the Mimo runtime launcher.
pub fn build_runtime_launcher() -> ProviderRuntimeLauncher {
    ProviderRuntimeLauncher::new(PROVIDER_NAME, LaunchMode::SimpleTmux)
}

/// Launch context prepared before building the start command.
#[derive(Debug, Clone, Default)]
pub struct MimoLaunchContext {
    pub agent_name: String,
    pub project_root: String,
    pub workspace_path: String,
    pub agent_events_path: String,
    pub mimo_home: String,
    pub mimo_config_path: String,
    pub mimo_storage_root: String,
}

/// Prepare the launch context for an agent.
pub fn prepare_launch_context(
    project_root: &Path,
    agent_name: &str,
    workspace_path: &Path,
    agent_events_path: &Path,
    runtime_dir: &Path,
) -> MimoLaunchContext {
    let state_dir = runtime_dir;
    let mimo_home = state_dir.join("home");
    MimoLaunchContext {
        agent_name: agent_name.to_string(),
        project_root: project_root.to_string_lossy().to_string(),
        workspace_path: workspace_path.to_string_lossy().to_string(),
        agent_events_path: agent_events_path.to_string_lossy().to_string(),
        mimo_home: mimo_home.to_string_lossy().to_string(),
        mimo_config_path: state_dir
            .join("mimocode.json")
            .to_string_lossy()
            .to_string(),
        mimo_storage_root: mimo_home
            .join("data")
            .join("storage")
            .to_string_lossy()
            .to_string(),
    }
}

/// Build the Mimo start command.
pub fn build_start_cmd(
    restore: bool,
    startup_args: &[String],
    launch_context: &MimoLaunchContext,
    command_template: Option<&str>,
) -> String {
    let mut cmd_parts = provider_start_parts(PROVIDER_NAME);
    if restore {
        cmd_parts.push("--continue".to_string());
    }
    cmd_parts.extend(startup_args.iter().cloned());
    let cmd = shlex_join(&cmd_parts);
    let cmd = apply_provider_command_template(&cmd, command_template).unwrap_or(cmd);

    let mimo_env: HashMap<String, String> = [
        ("MIMOCODE_HOME", launch_context.mimo_home.clone()),
        ("MIMOCODE_DISABLE_AUTOUPDATE", "true".to_string()),
        ("MIMOCODE_ENABLE_ANALYSIS", "false".to_string()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();

    let env_prefix = join_env_prefix(&[
        &export_env_clause(&provider_user_session_env()),
        &export_env_clause(&mimo_env),
        &export_env_clause(&caller_context_env(
            &launch_context.agent_name,
            Path::new(&launch_context.mimo_home),
            &launch_context.agent_name,
        )),
    ]);

    if env_prefix.is_empty() {
        cmd
    } else {
        format!("{env_prefix}; {cmd}")
    }
}

/// Build the session payload written to the session file.
#[allow(clippy::too_many_arguments)]
pub fn build_session_payload(
    launch_context: &MimoLaunchContext,
    runtime_dir: &Path,
    run_cwd: &Path,
    pane_id: &str,
    pane_title_marker: &str,
    start_cmd: &str,
    launch_session_id: &str,
    session_path: &Path,
) -> HashMap<String, Value> {
    let mut payload = HashMap::new();
    payload.insert(
        "ccbr_session_id".to_string(),
        Value::String(launch_session_id.to_string()),
    );
    payload.insert(
        "mimo_session_id".to_string(),
        Value::String(launch_session_id.to_string()),
    );
    payload.insert(
        "mimo_session_path".to_string(),
        Value::String(session_path.to_string_lossy().to_string()),
    );
    payload.insert(
        "agent_name".to_string(),
        Value::String(launch_context.agent_name.clone()),
    );
    payload.insert(
        "ccbr_project_id".to_string(),
        Value::String(launch_context.project_root.clone()),
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
        "mimo_home".to_string(),
        Value::String(launch_context.mimo_home.clone()),
    );
    payload.insert(
        "mimo_storage_root".to_string(),
        Value::String(launch_context.mimo_storage_root.clone()),
    );
    payload.insert(
        "mimo_config_path".to_string(),
        Value::String(launch_context.mimo_config_path.clone()),
    );
    payload.insert(
        "agent_events_path".to_string(),
        Value::String(launch_context.agent_events_path.clone()),
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

    #[test]
    fn test_build_start_cmd_includes_mimo() {
        let ctx = MimoLaunchContext {
            mimo_home: "/home/mimo".to_string(),
            ..Default::default()
        };
        let cmd = build_start_cmd(false, &[], &ctx, None);
        assert!(cmd.contains("mimo"));
        assert!(cmd.contains("MIMOCODE_HOME"));
    }

    #[test]
    fn test_build_session_payload() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = MimoLaunchContext {
            agent_name: "agent1".to_string(),
            project_root: "/project".to_string(),
            workspace_path: "/workspace".to_string(),
            agent_events_path: "/events".to_string(),
            mimo_home: "/mimo/home".to_string(),
            mimo_config_path: "/mimo/config.json".to_string(),
            mimo_storage_root: "/mimo/storage".to_string(),
        };
        let payload = build_session_payload(
            &ctx,
            tmp.path(),
            Path::new("/run"),
            "%1",
            "CCBR-agent1-proj",
            "mimo",
            "launch-1",
            Path::new("/workspace/.mimo-session"),
        );
        assert_eq!(payload.get("agent_name").unwrap(), "agent1");
        assert_eq!(payload.get("pane_id").unwrap(), "%1");
        assert_eq!(payload.get("ccbr_session_id").unwrap(), "launch-1");
        assert_eq!(payload.get("mimo_home").unwrap(), "/mimo/home");
    }
}
