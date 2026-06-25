use std::collections::HashMap;
use std::path::{Path, PathBuf};

use camino::Utf8Path;
use ccb_agents::models::{
    AgentApiSpec, AgentSpec, PermissionMode, ProviderProfileSpec, QueuePolicy, RestoreMode,
    RuntimeMode, WorkspaceMode,
};
use ccb_provider_core::pathing::{session_filename_for_agent, session_filename_for_instance};
use ccb_provider_core::registry::ProviderBackendRegistry;
use ccb_provider_core::runtime_shared::{pane_title_marker, provider_start_parts};
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
    /// Optional agent spec; when missing a minimal spec is synthesised.
    pub spec: Option<&'a ccb_agents::models::AgentSpec>,
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

    /// Build the launch plan without sending it to a pane.
    ///
    /// Useful for callers that manage pane creation/respawn themselves.
    pub fn build_plan(&self, ctx: &LaunchContext) -> Result<LaunchResult, String> {
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

        build_launch_plan(ctx, &provider)
    }

    /// Build the launch plan and send the start command to the pane.
    pub fn launch(&self, ctx: &LaunchContext) -> Result<LaunchResult, String> {
        let result = self.build_plan(ctx)?;
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
        "codex" | "claude" | "gemini" | "agy" | "droid" => {
            let ccb_dir = project_root.join(".ccb");
            session_filename_for_agent(provider, agent_name)
                .ok()
                .map(|filename| ccb_dir.join(filename))
        }
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
        "codex" => build_codex_launch(ctx),
        "claude" => build_claude_launch(ctx),
        "gemini" => build_gemini_launch(ctx),
        "agy" => build_agy_launch(ctx),
        "droid" => build_droid_launch(ctx),
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
    use ccb_providers::deepseek::{
        build_session_payload, build_start_cmd, prepare_launch_context,
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
    use ccb_providers::opencode::{
        build_session_payload, build_start_cmd, prepare_launch_context,
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

fn build_codex_launch(ctx: &LaunchContext) -> Result<LaunchResult, String> {
    use ccb_providers::codex::{
        build_session_payload, build_start_cmd, prepare_launch_context, CodexStartCommand,
    };

    let runtime_dir = runtime_dir_for_agent(ctx.project_root, ctx.agent_name);
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|e| format!("failed to create runtime dir for {}: {e}", ctx.agent_name))?;

    let workspace_path = Path::new(ctx.workspace_path);
    let agent_events_path = runtime_dir.join("events.jsonl");
    let prepared_state = serde_json::json!({});
    let launch_context = prepare_launch_context(
        Path::new(ctx.project_root),
        ctx.agent_name,
        workspace_path,
        &agent_events_path,
        &runtime_dir,
        Some(&prepared_state),
    );

    let spec = ctx
        .spec
        .cloned()
        .unwrap_or_else(|| minimal_agent_spec(ctx.agent_name, "codex", ctx.startup_args));
    let runtime_dir_utf8 = Utf8Path::from_path(&runtime_dir)
        .ok_or("runtime dir path is not UTF-8")?
        .to_path_buf();
    let launch_session_id = format!("{}-{}-launch", ctx.project_id, ctx.agent_name);
    let start_cmd = build_start_cmd(
        &CodexStartCommand {
            restore: ctx.restore,
            auto_permission: ctx.auto_permission,
            provider_command_template: ctx.command_template.map(String::from),
        },
        &spec,
        &runtime_dir_utf8,
        &launch_session_id,
        Some(&launch_context),
    )
    .map_err(|e| format!("failed to build codex start command: {e}"))?;

    let pane_title_marker = pane_title_marker(ctx.project_id, ctx.agent_name);
    let session_payload = build_session_payload(
        &launch_context,
        &runtime_dir_utf8,
        workspace_path,
        ctx.pane_id,
        &pane_title_marker,
        &start_cmd,
        &launch_session_id,
    );

    let session_path = ccb_dir(ctx.project_root).join(
        session_filename_for_agent("codex", ctx.agent_name)
            .map_err(|e| format!("invalid codex session filename: {e}"))?,
    );
    std::fs::write(
        &session_path,
        serde_json::to_string(&session_payload).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("failed to write codex session file: {e}"))?;

    // Seed the isolated codex HOME with auth config from the real HOME.
    // Without auth.json, codex CLI crashes on startup (no API credentials).
    let codex_home = runtime_dir.join("home");
    let _ = std::fs::create_dir_all(&codex_home);
    if let Ok(home) = std::env::var("HOME") {
        let real_codex = std::path::Path::new(&home).join(".codex");
        for file in &["auth.json", "AGENTS.md"] {
            let src = real_codex.join(file);
            if src.exists() {
                let _ = std::fs::copy(&src, codex_home.join(file));
            }
        }
    }
    // Overwrite malformed config.toml (generated as JSON-like blob by
    // prepare_launch_context) with a valid minimal TOML so codex CLI
    // doesn't crash on startup with "invalid key-value pair".
    let config_toml = codex_home.join("config.toml");
    if config_toml.exists() {
        let content = std::fs::read_to_string(&config_toml).unwrap_or_default();
        if content.trim_start().starts_with('{') {
            // JSON-like content — replace with valid TOML.
            let _ = std::fs::write(&config_toml, "");
        }
    }

    Ok(LaunchResult {
        command: start_cmd,
        session_payload: Some(serde_json::to_value(&session_payload).map_err(|e| e.to_string())?),
        session_path: Some(session_path),
    })
}

fn build_claude_launch(ctx: &LaunchContext) -> Result<LaunchResult, String> {
    use ccb_providers::claude::launcher_runtime::resolve_claude_restore_target;
    use ccb_providers::claude::{build_claude_start_cmd, ClaudeStartCommand};
    let result = build_simple_provider_launch(
        ctx,
        "claude",
        |spec, runtime_dir, launch_session_id, prepared_state| {
            build_claude_start_cmd(
                &ClaudeStartCommand {
                    restore: ctx.restore,
                    auto_permission: ctx.auto_permission,
                    provider_command_template: ctx.command_template.map(String::from),
                },
                spec,
                runtime_dir,
                launch_session_id,
                Some(prepared_state),
            )
        },
        |spec, runtime_dir, workspace_path| {
            let workspace_utf8 = Utf8Path::from_path(workspace_path)?;
            let target =
                resolve_claude_restore_target(spec, runtime_dir, ctx.restore, Some(workspace_utf8));
            Some(target.run_cwd.to_string())
        },
    )?;

    // Seed the isolated Claude HOME with essential config from the real HOME.
    // Without this, Claude treats the isolated HOME as a first-run and shows
    // a "Press Enter to continue…" security screen that blocks the prompt.
    // The .claude.json file contains hasCompletedOnboarding + auth markers
    // that skip the first-run screen (mirrors Python prepare_claude_home_overrides).
    let runtime_home = runtime_dir_for_agent(ctx.project_root, ctx.agent_name).join("home");
    let claude_config = runtime_home.join(".claude");
    let _ = std::fs::create_dir_all(&claude_config);
    if let Ok(home) = std::env::var("HOME") {
        let real_home = std::path::Path::new(&home);
        // Copy .claude.json (HOME root) — contains onboarding completion markers.
        let claude_json_src = real_home.join(".claude.json");
        if claude_json_src.exists() {
            let _ = std::fs::copy(&claude_json_src, runtime_home.join(".claude.json"));
        }
        // Copy .claude/ config files.
        let real_claude = real_home.join(".claude");
        for file in &[".credentials.json", "settings.json"] {
            let src = real_claude.join(file);
            if src.exists() {
                let _ = std::fs::copy(&src, claude_config.join(file));
            }
        }
    }

    Ok(result)
}

fn build_gemini_launch(ctx: &LaunchContext) -> Result<LaunchResult, String> {
    use ccb_providers::providers::gemini::{build_gemini_start_cmd, GeminiStartCommand};
    build_simple_provider_launch(
        ctx,
        "gemini",
        |spec, runtime_dir, launch_session_id, prepared_state| {
            build_gemini_start_cmd(
                &GeminiStartCommand {
                    restore: ctx.restore,
                    auto_permission: ctx.auto_permission,
                    provider_command_template: ctx.command_template.map(String::from),
                },
                spec,
                runtime_dir,
                launch_session_id,
                Some(prepared_state),
            )
        },
        |_, _, _| None,
    )
}

fn build_agy_launch(ctx: &LaunchContext) -> Result<LaunchResult, String> {
    use ccb_providers::providers::agy::{build_agy_start_cmd, AgyStartCommand};
    build_simple_provider_launch(
        ctx,
        "agy",
        |spec, runtime_dir, launch_session_id, prepared_state| {
            build_agy_start_cmd(
                &AgyStartCommand {
                    restore: ctx.restore,
                    auto_permission: ctx.auto_permission,
                    provider_command_template: ctx.command_template.map(String::from),
                },
                spec,
                runtime_dir,
                launch_session_id,
                Some(prepared_state),
            )
        },
        |_, _, _| None,
    )
}

fn build_droid_launch(ctx: &LaunchContext) -> Result<LaunchResult, String> {
    use ccb_providers::droid::launcher::{
        build_start_cmd as build_droid_start_cmd, DroidStartCommand,
    };
    build_simple_provider_launch(
        ctx,
        "droid",
        |spec, runtime_dir, launch_session_id, prepared_state| {
            build_droid_start_cmd(
                &DroidStartCommand {
                    restore: ctx.restore,
                    provider_command_template: ctx.command_template.map(String::from),
                },
                spec,
                runtime_dir,
                launch_session_id,
                Some(prepared_state),
            )
        },
        |_, _, _| None,
    )
}

fn build_simple_provider_launch<'a>(
    ctx: &'a LaunchContext,
    provider: &str,
    build_start_cmd_fn: impl FnOnce(&AgentSpec, &Utf8Path, &str, &HashMap<String, String>) -> anyhow::Result<String>
        + 'a,
    resolve_run_cwd_fn: impl FnOnce(&AgentSpec, &Utf8Path, &Path) -> Option<String> + 'a,
) -> Result<LaunchResult, String> {
    let runtime_dir = runtime_dir_for_agent(ctx.project_root, ctx.agent_name);
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|e| format!("failed to create runtime dir for {}: {e}", ctx.agent_name))?;

    let workspace_path = Path::new(ctx.workspace_path);
    let agent_events_path = runtime_dir.join("events.jsonl");

    let spec = ctx
        .spec
        .cloned()
        .unwrap_or_else(|| minimal_agent_spec(ctx.agent_name, provider, ctx.startup_args));
    let runtime_dir_utf8 = Utf8Path::from_path(&runtime_dir)
        .ok_or("runtime dir path is not UTF-8")?
        .to_path_buf();
    let run_cwd = resolve_run_cwd_fn(&spec, &runtime_dir_utf8, workspace_path)
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_path.to_path_buf());
    let prepared_state = simple_prepared_state(
        ctx.project_root,
        ctx.workspace_path,
        &agent_events_path,
        run_cwd.to_string_lossy().as_ref(),
    );

    let launch_session_id = format!("{}-{}-launch", ctx.project_id, ctx.agent_name);
    let start_cmd = build_start_cmd_fn(
        &spec,
        &runtime_dir_utf8,
        &launch_session_id,
        &prepared_state,
    )
    .map_err(|e| format!("failed to build {provider} start command: {e}"))?;

    let pane_title_marker = pane_title_marker(ctx.project_id, ctx.agent_name);
    let session_payload = build_simple_session_payload(
        ctx.agent_name,
        ctx.project_root,
        &runtime_dir,
        &run_cwd,
        ctx.pane_id,
        &pane_title_marker,
        &start_cmd,
        &launch_session_id,
        &agent_events_path,
    );

    let session_path = ccb_dir(ctx.project_root).join(
        session_filename_for_agent(provider, ctx.agent_name)
            .map_err(|e| format!("invalid {provider} session filename: {e}"))?,
    );
    std::fs::write(
        &session_path,
        serde_json::to_string(&session_payload).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("failed to write {provider} session file: {e}"))?;

    Ok(LaunchResult {
        command: start_cmd,
        session_payload: Some(serde_json::to_value(&session_payload).map_err(|e| e.to_string())?),
        session_path: Some(session_path),
    })
}

fn runtime_dir_for_agent(project_root: &str, agent_name: &str) -> PathBuf {
    Path::new(project_root)
        .join(".ccb")
        .join("runtime")
        .join(agent_name)
}

fn ccb_dir(project_root: &str) -> PathBuf {
    Path::new(project_root).join(".ccb")
}

fn simple_prepared_state(
    project_root: &str,
    workspace_path: &str,
    agent_events_path: &Path,
    run_cwd: &str,
) -> HashMap<String, String> {
    let mut state = HashMap::new();
    state.insert("project_root".to_string(), project_root.to_string());
    state.insert("workspace_path".to_string(), workspace_path.to_string());
    state.insert(
        "agent_events_path".to_string(),
        agent_events_path.to_string_lossy().to_string(),
    );
    state.insert("run_cwd".to_string(), run_cwd.to_string());
    state
}

#[allow(clippy::too_many_arguments)]
fn build_simple_session_payload(
    agent_name: &str,
    project_root: &str,
    runtime_dir: &Path,
    run_cwd: &Path,
    pane_id: &str,
    pane_title_marker: &str,
    start_cmd: &str,
    launch_session_id: &str,
    agent_events_path: &Path,
) -> HashMap<String, Value> {
    let mut payload = HashMap::new();
    payload.insert(
        "ccb_session_id".to_string(),
        Value::String(launch_session_id.to_string()),
    );
    payload.insert(
        "agent_name".to_string(),
        Value::String(agent_name.to_string()),
    );
    payload.insert(
        "ccb_project_id".to_string(),
        Value::String(project_root.to_string()),
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
        Value::String(run_cwd.to_string_lossy().to_string()),
    );
    payload.insert(
        "work_dir".to_string(),
        Value::String(run_cwd.to_string_lossy().to_string()),
    );
    payload.insert(
        "start_dir".to_string(),
        Value::String(project_root.to_string()),
    );
    payload.insert(
        "start_cmd".to_string(),
        Value::String(start_cmd.to_string()),
    );
    payload.insert(
        "agent_events_path".to_string(),
        Value::String(agent_events_path.to_string_lossy().to_string()),
    );
    payload
}

fn minimal_agent_spec(name: &str, provider: &str, startup_args: &[String]) -> AgentSpec {
    AgentSpec {
        name: name.to_string(),
        provider: provider.to_string(),
        target: name.to_string(),
        workspace_mode: WorkspaceMode::Inplace,
        workspace_root: None,
        runtime_mode: RuntimeMode::PaneBacked,
        restore_default: RestoreMode::Fresh,
        permission_default: PermissionMode::Manual,
        queue_policy: QueuePolicy::SerialPerAgent,
        workspace_path: None,
        workspace_group: None,
        provider_command_template: None,
        model: None,
        startup_args: startup_args.to_vec(),
        env: HashMap::new(),
        api: AgentApiSpec::default(),
        provider_profile: ProviderProfileSpec::default(),
        branch_template: None,
        labels: Vec::new(),
        description: None,
        role: None,
        watch_paths: Vec::new(),
    }
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
        let root = Path::new("/tmp/proj");
        let ws = Path::new("/tmp/ws");
        let path = default_session_path("codex", "codex", root, ws).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/proj/.ccb/.codex-codex-session"));
    }

    #[test]
    fn test_default_session_path_opencode() {
        let root = Path::new("/tmp/proj");
        let ws = Path::new("/tmp/ws");
        let path = default_session_path("opencode", "opencode", root, ws).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/tmp/proj/.ccb/runtime/opencode/opencode-session.jsonl")
        );
    }

    #[test]
    fn test_default_session_path_deepseek() {
        let root = Path::new("/tmp/proj");
        let ws = Path::new("/tmp/ws");
        let path = default_session_path("deepseek", "deepseek", root, ws).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/tmp/proj/.ccb/runtime/deepseek/deepseek-session.jsonl")
        );
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
    fn test_default_session_path_claude() {
        let root = Path::new("/tmp/proj");
        let ws = Path::new("/tmp/ws");
        let path = default_session_path("claude", "reviewer", root, ws).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/tmp/proj/.ccb/.claude-reviewer-session")
        );
    }

    #[test]
    fn test_default_session_path_gemini() {
        let root = Path::new("/tmp/proj");
        let ws = Path::new("/tmp/ws");
        let path = default_session_path("gemini", "gemini", root, ws).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/tmp/proj/.ccb/.gemini-gemini-session")
        );
    }

    #[test]
    fn test_default_session_path_agy() {
        let root = Path::new("/tmp/proj");
        let ws = Path::new("/tmp/ws");
        let path = default_session_path("agy", "agy", root, ws).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/proj/.ccb/.agy-agy-session"));
    }

    #[test]
    fn test_default_session_path_droid() {
        let root = Path::new("/tmp/proj");
        let ws = Path::new("/tmp/ws");
        let path = default_session_path("droid", "droid", root, ws).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/proj/.ccb/.droid-droid-session"));
    }

    #[test]
    fn test_shlex_join_quotes_whitespace() {
        let parts = vec!["echo".to_string(), "hello world".to_string()];
        assert_eq!(shlex_join(&parts), "echo 'hello world'");
    }

    #[test]
    fn test_shlex_join_quotes_single_quote() {
        let parts = vec!["echo".to_string(), "it's".to_string()];
        assert_eq!(shlex_join(&parts), "echo 'it'\"'\"'s'");
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
            spec: None,
        };
        assert!(launcher.launch(&ctx).is_err());
    }
}
