use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccbr_provider_core::caller_env::{
    caller_context_env, export_env_clause, join_env_prefix, provider_user_session_env,
};
use ccbr_provider_core::contracts::{LaunchMode, ProviderRuntimeLauncher};
use ccbr_provider_core::runtime_shared::{apply_provider_command_template, provider_start_parts};
use serde_json::Value;

use super::skills::kimi_skill_dirs_for_launch;

pub const PROVIDER_NAME: &str = "kimi";

const AUTO_FLAG: &str = "--auto-approve";
const AUTO_FLAGS: &[&str] = &["--auto-approve", "--auto", "--yes", "-y", "--yolo"];

/// Build the Kimi runtime launcher.
pub fn build_runtime_launcher() -> ProviderRuntimeLauncher {
    ProviderRuntimeLauncher::new(PROVIDER_NAME, LaunchMode::SimpleTmux)
}

/// Launch context prepared before building the start command.
#[derive(Debug, Clone, Default)]
pub struct KimiLaunchContext {
    pub agent_name: String,
    pub project_root: String,
    pub workspace_path: String,
    pub agent_events_path: String,
    pub skill_dirs: Vec<PathBuf>,
}

/// Prepare the launch context for an agent.
pub fn prepare_launch_context(
    project_root: &Path,
    agent_name: &str,
    workspace_path: &Path,
    agent_events_path: &Path,
    runtime_dir: &Path,
) -> KimiLaunchContext {
    let env: HashMap<String, String> = std::env::vars().collect();
    let skill_dirs = kimi_skill_dirs_for_launch(
        Some(project_root),
        Some(workspace_path),
        &state_dir_for_agent(runtime_dir),
        Some(&env),
    );
    KimiLaunchContext {
        agent_name: agent_name.to_string(),
        project_root: project_root.to_string_lossy().to_string(),
        workspace_path: workspace_path.to_string_lossy().to_string(),
        agent_events_path: agent_events_path.to_string_lossy().to_string(),
        skill_dirs,
    }
}

/// Build the Kimi start command.
pub fn build_start_cmd(
    restore: bool,
    startup_args: &[String],
    auto_permission: bool,
    launch_context: &KimiLaunchContext,
    command_template: Option<&str>,
) -> String {
    let mut cmd_parts = provider_start_parts(PROVIDER_NAME);
    if restore {
        cmd_parts.push("--continue".to_string());
    }
    if auto_permission && !has_any(&cmd_parts, AUTO_FLAGS) && !has_any(startup_args, AUTO_FLAGS) {
        cmd_parts.push(AUTO_FLAG.to_string());
    }
    cmd_parts.extend(skill_dir_args(&launch_context.skill_dirs, &cmd_parts));
    cmd_parts.extend(startup_args.iter().cloned());
    let cmd = shlex_join(&cmd_parts);
    apply_provider_command_template(&cmd, command_template).unwrap_or(cmd)
}

/// Build the session payload written to the session file.
pub fn build_session_payload(
    launch_context: &KimiLaunchContext,
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
        "agent_events_path".to_string(),
        Value::String(launch_context.agent_events_path.clone()),
    );
    payload
}

/// Build the shell environment prefix for the launch command.
pub fn build_env_prefix(runtime_dir: &Path, launch_session_id: &str, agent_name: &str) -> String {
    join_env_prefix(&[
        &export_env_clause(&provider_user_session_env()),
        &export_env_clause(&caller_context_env(
            agent_name,
            runtime_dir,
            launch_session_id,
        )),
    ])
}

fn state_dir_for_agent(runtime_dir: &Path) -> PathBuf {
    runtime_dir.to_path_buf()
}

fn has_any(parts: &[String], flags: &[&str]) -> bool {
    let normalized: std::collections::HashSet<String> =
        parts.iter().map(|p| p.trim().to_lowercase()).collect();
    flags
        .iter()
        .any(|f| normalized.contains(f.to_lowercase().as_str()))
}

fn skill_dir_args(raw_dirs: &[PathBuf], existing_parts: &[String]) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    for path in raw_dirs {
        if !path.is_dir() {
            continue;
        }
        let value = path.to_string_lossy().to_string();
        if has_option_value(existing_parts, "--skills-dir", &value)
            || has_option_value(&args, "--skills-dir", &value)
        {
            continue;
        }
        args.push("--skills-dir".to_string());
        args.push(value);
    }
    args
}

fn has_option_value(parts: &[String], option: &str, value: &str) -> bool {
    let normalized: Vec<String> = parts.iter().map(|p| p.trim().to_string()).collect();
    for (index, part) in normalized.iter().enumerate() {
        if part == option && index + 1 < normalized.len() && normalized[index + 1] == value {
            return true;
        }
        if part == &format!("{}={}", option, value) {
            return true;
        }
    }
    false
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
    fn test_build_start_cmd_includes_kimi() {
        let ctx = KimiLaunchContext::default();
        let cmd = build_start_cmd(false, &[], false, &ctx, None);
        assert!(cmd.contains("kimi"));
    }

    #[test]
    fn test_build_start_cmd_auto_approve() {
        let ctx = KimiLaunchContext::default();
        let cmd = build_start_cmd(false, &[], true, &ctx, None);
        assert!(cmd.contains("--auto-approve"));
    }

    #[test]
    fn test_build_start_cmd_skills_dirs() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        std::fs::create_dir(&skill_dir).unwrap();
        let ctx = KimiLaunchContext {
            skill_dirs: vec![skill_dir],
            ..Default::default()
        };
        let cmd = build_start_cmd(false, &[], false, &ctx, None);
        assert!(cmd.contains("--skills-dir"));
    }

    #[test]
    fn test_build_session_payload() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = KimiLaunchContext {
            agent_name: "agent1".to_string(),
            project_root: "/project".to_string(),
            workspace_path: "/workspace".to_string(),
            agent_events_path: "/events".to_string(),
            skill_dirs: Vec::new(),
        };
        let payload = build_session_payload(
            &ctx,
            tmp.path(),
            Path::new("/run"),
            "%1",
            "CCB-agent1-proj",
            "kimi",
            "launch-1",
        );
        assert_eq!(payload.get("agent_name").unwrap(), "agent1");
        assert_eq!(payload.get("pane_id").unwrap(), "%1");
        assert_eq!(payload.get("ccbr_session_id").unwrap(), "launch-1");
    }
}
