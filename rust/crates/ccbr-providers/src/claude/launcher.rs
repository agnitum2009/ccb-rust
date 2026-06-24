//! Mirrors Python `lib/provider_backends/claude/launcher.py`.

use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};
use ccbr_provider_core::caller_env::{caller_context_env, export_env_clause, join_env_prefix};
use ccbr_provider_core::runtime_shared::{apply_provider_command_template, provider_start_parts};
use ccbr_provider_profiles::models::ResolvedProviderProfile;

use crate::claude::launcher_runtime::{
    build_claude_env_prefix, prepare_claude_home_overrides, resolve_claude_restore_target,
    write_claude_settings_overlay,
};

/// Start-command options specific to Claude.
#[derive(Debug, Clone, Default)]
pub struct ClaudeStartCommand {
    pub restore: bool,
    pub auto_permission: bool,
    pub provider_command_template: Option<String>,
}

/// Build the shell command that launches a Claude runtime pane.
///
/// Mirrors Python `lib/provider_backends/claude/launcher_runtime/service.py::build_start_cmd`.
pub fn build_start_cmd(
    command: &ClaudeStartCommand,
    spec: &ccbr_agents::models::AgentSpec,
    runtime_dir: &Utf8Path,
    launch_session_id: &str,
    prepared_state: Option<&HashMap<String, String>>,
) -> anyhow::Result<String> {
    let prepared_state = prepared_state.cloned().unwrap_or_default();
    let project_root = path_or_none(prepared_state.get("project_root")).ok_or_else(|| {
        anyhow::anyhow!("Claude launch requires prepare_launch_context before build_start_cmd")
    })?;
    let agent_events_path = path_or_none(prepared_state.get("agent_events_path"));
    let workspace_path = path_or_none(prepared_state.get("workspace_path"));

    let root_user = is_root_user();
    let profile = load_resolved_provider_profile(runtime_dir);
    let restore_target = resolve_claude_restore_target(
        spec,
        runtime_dir,
        command.restore,
        workspace_path.as_deref(),
    );

    let home_overrides = prepare_claude_home_overrides(
        runtime_dir,
        profile.as_ref(),
        false,
        command.auto_permission,
        Some(&project_root),
        Some(&spec.name),
        Some(&restore_target.run_cwd),
        agent_events_path.as_deref(),
        Some(&runtime_dir.join("claude-memory-projection.json")),
    )?;

    let mut settings_path = write_claude_settings_overlay(runtime_dir, profile.as_ref());
    if command.auto_permission {
        settings_path = Some(ensure_skip_prompt_settings(
            runtime_dir,
            settings_path.as_deref(),
        )?);
    }

    let env_prefix = join_env_prefix(&[
        &build_claude_env_prefix(
            profile.as_ref(),
            Some(&spec.env),
            None,
            |value| {
                crate::claude::launcher_runtime::env::should_drop_claude_base_url(
                    value,
                    crate::claude::launcher_runtime::env::local_tcp_listener_available,
                )
            },
            || {
                crate::claude::launcher_runtime::env::claude_user_base_url(
                    &crate::claude::home_layout::current_claude_home_root()
                        .join(".claude")
                        .join("settings.json"),
                )
            },
        ),
        &export_env_clause(&ccbr_provider_core::caller_env::provider_user_session_env()),
        &export_env_clause(&home_overrides),
        &if root_user {
            export_env_clause(&{
                let mut m = HashMap::new();
                m.insert("IS_SANDBOX".to_string(), "1".to_string());
                m
            })
        } else {
            String::new()
        },
        &export_env_clause(&caller_context_env(
            &spec.name,
            runtime_dir.as_std_path(),
            launch_session_id,
        )),
    ]);

    let mut cmd_parts = provider_start_parts("claude");
    let skip_permissions = "--dangerously-skip-permissions";
    if root_user
        && !cmd_parts.iter().any(|p| p == skip_permissions)
        && !spec.startup_args.iter().any(|p| p == skip_permissions)
    {
        cmd_parts.push(skip_permissions.to_string());
    }
    cmd_parts.extend([
        "--setting-sources".to_string(),
        "user,project,local".to_string(),
    ]);

    if let Some(settings_path) = settings_path {
        let inline = match std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        {
            Some(value) => serde_json::to_string(&value).ok(),
            None => None,
        };
        cmd_parts.push("--settings".to_string());
        cmd_parts.push(inline.unwrap_or_else(|| settings_path.to_string()));
    }

    if command.auto_permission {
        cmd_parts.extend([
            "--permission-mode".to_string(),
            "bypassPermissions".to_string(),
        ]);
    }

    if restore_target.has_history {
        cmd_parts.push("--continue".to_string());
    }

    cmd_parts.extend(spec.startup_args.iter().cloned());

    let cmd = cmd_parts
        .iter()
        .map(|p| shell_quote(p))
        .collect::<Vec<_>>()
        .join(" ");
    let cmd = apply_provider_command_template(&cmd, command.provider_command_template.as_deref())?;

    if env_prefix.is_empty() {
        Ok(cmd)
    } else {
        Ok(format!("{}; {}", env_prefix, cmd))
    }
}

fn ensure_skip_prompt_settings(
    runtime_dir: &Utf8Path,
    existing_path: Option<&Utf8Path>,
) -> anyhow::Result<Utf8PathBuf> {
    let path = existing_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| runtime_dir.join("claude-settings.json"));
    let mut payload = if path.is_file() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    } else {
        serde_json::Map::new()
    };
    payload.insert(
        "skipDangerousModePermissionPrompt".to_string(),
        serde_json::Value::Bool(true),
    );
    std::fs::create_dir_all(path.parent().unwrap_or(runtime_dir))?;
    ccbr_storage::atomic::atomic_write_json(&path, &serde_json::Value::Object(payload))?;
    Ok(path)
}

fn load_resolved_provider_profile(runtime_dir: &Utf8Path) -> Option<ResolvedProviderProfile> {
    let path = runtime_dir.join("provider-profile.json");
    if !path.is_file() {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let record = value.as_object().cloned()?;
    ccbr_provider_profiles::models::ResolvedProviderProfile::from_record(&record).ok()
}

fn is_root_user() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn path_or_none(value: Option<&String>) -> Option<Utf8PathBuf> {
    let raw = value.map(|s| s.trim()).filter(|s| !s.is_empty())?;
    Some(Utf8PathBuf::from(raw))
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|c| c.is_alphanumeric() || "_-.,/:~=@%".contains(c))
    {
        return value.to_string();
    }
    let mut out = String::from("'");
    for ch in value.chars() {
        if ch == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}
