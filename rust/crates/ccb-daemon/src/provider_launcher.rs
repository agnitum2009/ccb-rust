use std::path::{Path, PathBuf};

use ccb_provider_core::pathing::session_filename_for_instance;
use ccb_provider_core::registry::ProviderBackendRegistry;
use ccb_provider_core::runtime_shared::provider_start_parts;
use ccb_terminal::TmuxBackend;
use serde_json::Value;

/// Context needed to launch a provider into a tmux pane.
pub struct LaunchContext<'a> {
    pub provider: &'a str,
    pub agent_name: &'a str,
    pub project_id: &'a str,
    pub project_root: &'a str,
    pub workspace_path: &'a str,
    pub pane_id: &'a str,
    pub socket_path: &'a str,
    pub restore: bool,
    /// Optional wrapper template such as `tmux new-window {command}`.
    pub command_template: Option<&'a str>,
    /// Provider-specific startup arguments from the agent spec.
    pub startup_args: &'a [String],
    /// Whether the provider should launch with auto-approve behavior.
    pub auto_permission: bool,
}

/// Result of preparing a provider launch.
pub struct LaunchResult {
    pub command: String,
    pub session_payload: Option<Value>,
    pub session_path: Option<PathBuf>,
}

/// Launches provider CLIs into tmux panes using the provider backend registry.
pub struct ProviderLauncher {
    backend_registry: ProviderBackendRegistry,
}

impl Default for ProviderLauncher {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderLauncher {
    pub fn new() -> Self {
        Self {
            backend_registry: ccb_providers::build_default_backend_registry(),
        }
    }

    /// Build the launch plan and send the start command to the pane.
    pub fn launch(&self, ctx: &LaunchContext) -> Result<LaunchResult, String> {
        let provider = ctx.provider.trim().to_lowercase();
        if provider.is_empty() {
            return Err(format!(
                "cannot launch agent {}: no provider configured",
                ctx.agent_name
            ));
        }

        // Validate provider is known.
        let backend = self.backend_registry.get(&provider);
        let _launch_mode = backend
            .and_then(|b| b.runtime_launcher.as_ref())
            .map(|l| l.launch_mode);

        let result = build_launch_plan(ctx, &provider)?;
        launch_command_in_pane(
            ctx.pane_id,
            ctx.socket_path,
            ctx.workspace_path,
            &result.command,
        )?;
        Ok(result)
    }
}

/// Return the default session log path for a provider if it has a known
/// convention. Mirrors the session path defaults used by provider adapters.
pub fn default_session_path(
    provider: &str,
    agent_name: &str,
    project_root: &Path,
    workspace: &Path,
) -> Option<PathBuf> {
    match provider.trim().to_lowercase().as_str() {
        "opencode" => {
            let runtime_dir = project_root.join(".ccb").join("runtime").join(agent_name);
            Some(runtime_dir.join(format!("{}-session.jsonl", agent_name)))
        }
        "deepseek" => {
            let runtime_dir = project_root.join(".ccb").join("runtime").join(agent_name);
            Some(runtime_dir.join(format!("{}-session.jsonl", agent_name)))
        }
        "codex" => Some(workspace.join("codex-session.jsonl")),
        "kimi" => Some(workspace.join(session_filename_for_instance(
            ".kimi-session",
            Some(agent_name),
        ))),
        "mimo" => Some(workspace.join(session_filename_for_instance(
            ".mimo-session",
            Some(agent_name),
        ))),
        _ => None,
    }
}

fn build_launch_plan(ctx: &LaunchContext, provider: &str) -> Result<LaunchResult, String> {
    match provider {
        "opencode" => build_opencode_launch(ctx),
        "deepseek" => build_deepseek_launch(ctx),
        "kimi" => build_kimi_launch(ctx),
        "mimo" => build_mimo_launch(ctx),
        _ => build_generic_launch(ctx, provider),
    }
}

fn build_generic_launch(ctx: &LaunchContext, provider: &str) -> Result<LaunchResult, String> {
    let mut parts = provider_start_parts(provider);
    if ctx.restore && provider == "opencode" {
        parts.push("--continue".to_string());
    }

    let mut command = shlex_join(&parts);
    if let Some(template) = ctx.command_template {
        command = apply_template(template, &command)?;
    }

    Ok(LaunchResult {
        command,
        session_payload: None,
        session_path: None,
    })
}

fn build_deepseek_launch(ctx: &LaunchContext) -> Result<LaunchResult, String> {
    use ccb_providers::deepseek::{build_session_payload, build_start_cmd, prepare_launch_context};

    let runtime_dir = Path::new(ctx.project_root)
        .join(".ccb")
        .join("runtime")
        .join(ctx.agent_name);
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|e| format!("failed to create runtime dir for {}: {e}", ctx.agent_name))?;

    let workspace_path = Path::new(ctx.workspace_path);
    let agent_events_path = runtime_dir.join("events.jsonl");
    let launch_context = prepare_launch_context(
        Path::new(ctx.project_root),
        ctx.agent_name,
        workspace_path,
        &agent_events_path,
        &runtime_dir,
    );

    let start_cmd = build_start_cmd(ctx.restore, &[], ctx.command_template);
    let launch_session_id = format!("{}-{}-launch", ctx.project_id, ctx.agent_name);
    let pane_title_marker =
        ccb_provider_core::runtime_shared::pane_title_marker(ctx.project_id, ctx.agent_name);

    let session_payload = build_session_payload(
        &launch_context,
        &runtime_dir,
        workspace_path,
        ctx.pane_id,
        &pane_title_marker,
        &start_cmd,
        &launch_session_id,
    );

    let session_path = runtime_dir.join(format!("{}-session.jsonl", ctx.agent_name));
    std::fs::write(
        &session_path,
        serde_json::to_string(&session_payload).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("failed to write deepseek session file: {e}"))?;

    Ok(LaunchResult {
        command: start_cmd,
        session_payload: Some(serde_json::to_value(&session_payload).map_err(|e| e.to_string())?),
        session_path: Some(session_path),
    })
}

fn build_kimi_launch(ctx: &LaunchContext) -> Result<LaunchResult, String> {
    use ccb_providers::kimi::{
        build_env_prefix, build_session_payload, build_start_cmd, prepare_launch_context,
    };

    let runtime_dir = Path::new(ctx.project_root)
        .join(".ccb")
        .join("runtime")
        .join(ctx.agent_name);
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|e| format!("failed to create runtime dir for {}: {e}", ctx.agent_name))?;

    let workspace_path = Path::new(ctx.workspace_path);
    let agent_events_path = runtime_dir.join("events.jsonl");
    let launch_context = prepare_launch_context(
        Path::new(ctx.project_root),
        ctx.agent_name,
        workspace_path,
        &agent_events_path,
        &runtime_dir,
    );

    let launch_session_id = format!("{}-{}-launch", ctx.project_id, ctx.agent_name);
    let pane_title_marker =
        ccb_provider_core::runtime_shared::pane_title_marker(ctx.project_id, ctx.agent_name);

    let env_prefix = build_env_prefix(&runtime_dir, &launch_session_id, ctx.agent_name);
    let start_cmd = build_start_cmd(
        ctx.restore,
        ctx.startup_args,
        ctx.auto_permission,
        &launch_context,
        ctx.command_template,
    );
    let command = if env_prefix.is_empty() {
        start_cmd
    } else {
        format!("{env_prefix}; {start_cmd}")
    };

    let session_payload = build_session_payload(
        &launch_context,
        &runtime_dir,
        workspace_path,
        ctx.pane_id,
        &pane_title_marker,
        &command,
        &launch_session_id,
    );

    let session_filename = ccb_provider_core::pathing::session_filename_for_instance(
        ".kimi-session",
        Some(ctx.agent_name),
    );
    let session_path = workspace_path.join(session_filename);
    std::fs::write(
        &session_path,
        serde_json::to_string(&session_payload).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("failed to write kimi session file: {e}"))?;

    Ok(LaunchResult {
        command,
        session_payload: Some(serde_json::to_value(&session_payload).map_err(|e| e.to_string())?),
        session_path: Some(session_path),
    })
}

fn build_mimo_launch(ctx: &LaunchContext) -> Result<LaunchResult, String> {
    use ccb_providers::mimo::{build_session_payload, build_start_cmd, prepare_launch_context};

    let runtime_dir = Path::new(ctx.project_root)
        .join(".ccb")
        .join("runtime")
        .join(ctx.agent_name);
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|e| format!("failed to create runtime dir for {}: {e}", ctx.agent_name))?;

    let workspace_path = Path::new(ctx.workspace_path);
    let agent_events_path = runtime_dir.join("events.jsonl");
    let launch_context = prepare_launch_context(
        Path::new(ctx.project_root),
        ctx.agent_name,
        workspace_path,
        &agent_events_path,
        &runtime_dir,
    );

    let launch_session_id = format!("{}-{}-launch", ctx.project_id, ctx.agent_name);
    let pane_title_marker =
        ccb_provider_core::runtime_shared::pane_title_marker(ctx.project_id, ctx.agent_name);

    let start_cmd = build_start_cmd(
        ctx.restore,
        ctx.startup_args,
        &launch_context,
        ctx.command_template,
    );

    let session_filename = ccb_provider_core::pathing::session_filename_for_instance(
        ".mimo-session",
        Some(ctx.agent_name),
    );
    let session_path = workspace_path.join(session_filename);

    let session_payload = build_session_payload(
        &launch_context,
        &runtime_dir,
        workspace_path,
        ctx.pane_id,
        &pane_title_marker,
        &start_cmd,
        &launch_session_id,
        &session_path,
    );
    std::fs::write(
        &session_path,
        serde_json::to_string(&session_payload).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("failed to write mimo session file: {e}"))?;

    Ok(LaunchResult {
        command: start_cmd,
        session_payload: Some(serde_json::to_value(&session_payload).map_err(|e| e.to_string())?),
        session_path: Some(session_path),
    })
}

fn build_opencode_launch(ctx: &LaunchContext) -> Result<LaunchResult, String> {
    use ccb_providers::opencode::{build_session_payload, build_start_cmd, prepare_launch_context};

    let runtime_dir = Path::new(ctx.project_root)
        .join(".ccb")
        .join("runtime")
        .join(ctx.agent_name);
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|e| format!("failed to create runtime dir for {}: {e}", ctx.agent_name))?;

    let workspace_path = Path::new(ctx.workspace_path);
    let agent_events_path = runtime_dir.join("events.jsonl");
    let launch_context = prepare_launch_context(
        Path::new(ctx.project_root),
        ctx.agent_name,
        workspace_path,
        &agent_events_path,
        &runtime_dir,
    );

    let start_cmd = build_start_cmd(ctx.restore, &[], ctx.command_template);
    let launch_session_id = format!("{}-{}-launch", ctx.project_id, ctx.agent_name);
    let pane_title_marker =
        ccb_provider_core::runtime_shared::pane_title_marker(ctx.project_id, ctx.agent_name);

    let session_payload = build_session_payload(
        &launch_context,
        &runtime_dir,
        workspace_path,
        ctx.pane_id,
        &pane_title_marker,
        &start_cmd,
        &launch_session_id,
    );

    let session_path = runtime_dir.join(format!("{}-session.jsonl", ctx.agent_name));
    std::fs::write(
        &session_path,
        serde_json::to_string(&session_payload).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("failed to write opencode session file: {e}"))?;

    Ok(LaunchResult {
        command: start_cmd,
        session_payload: Some(serde_json::to_value(&session_payload).map_err(|e| e.to_string())?),
        session_path: Some(session_path),
    })
}

fn launch_command_in_pane(
    pane_id: &str,
    socket_path: &str,
    cwd: &str,
    command: &str,
) -> Result<(), String> {
    if command.trim().is_empty() {
        return Err("cannot launch empty command in pane".to_string());
    }

    let backend = TmuxBackend::new(None, Some(socket_path.to_string()));
    backend
        .tmux_run(
            &["respawn-pane", "-k", "-t", pane_id, "-c", cwd, command],
            true,
            false,
            None,
            None,
        )
        .map_err(|e| format!("failed to respawn pane {pane_id} with command: {e}"))?;
    Ok(())
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

fn apply_template(template: &str, command: &str) -> Result<String, String> {
    ccb_provider_core::runtime_shared::apply_provider_command_template(command, Some(template))
        .map_err(|e| format!("invalid command template: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_session_path_codex() {
        let ws = Path::new("/tmp/ws");
        let path = default_session_path("codex", "codex", ws, ws).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/ws/codex-session.jsonl"));
    }

    #[test]
    fn test_default_session_path_unknown_provider() {
        let ws = Path::new("/tmp/ws");
        assert!(default_session_path("unknown", "agent", ws, ws).is_none());
    }

    #[test]
    fn test_default_session_path_kimi() {
        let ws = Path::new("/tmp/ws");
        let path = default_session_path("kimi", "reviewer", ws, ws).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/ws/.kimi-reviewer-session"));
    }

    #[test]
    fn test_default_session_path_mimo() {
        let ws = Path::new("/tmp/ws");
        let path = default_session_path("mimo", "mimo", ws, ws).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/ws/.mimo-mimo-session"));
    }

    #[test]
    fn test_shlex_join_quotes_whitespace() {
        let parts = vec!["echo".to_string(), "hello world".to_string()];
        assert_eq!(shlex_join(&parts), "echo 'hello world'");
    }

    #[test]
    fn test_apply_template_ok() {
        let out = apply_template("tmux new-window {command}", "claude").unwrap();
        assert_eq!(out, "tmux new-window claude");
    }

    #[test]
    fn test_provider_launcher_rejects_empty_provider() {
        let launcher = ProviderLauncher::new();
        let ctx = LaunchContext {
            provider: "",
            agent_name: "agent1",
            project_id: "proj",
            project_root: "/tmp",
            workspace_path: "/tmp",
            pane_id: "%1",
            socket_path: "/tmp/tmux.sock",
            restore: false,
            command_template: None,
            startup_args: &[],
            auto_permission: false,
        };
        assert!(launcher.launch(&ctx).is_err());
    }
}
