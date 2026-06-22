//! Mirrors Python `lib/provider_backends/agy/launcher.py`.
//!
//! This is a minimal parity implementation: it builds the agy start command,
//! applies the provider command template, and exports the managed HOME plus
//! caller context. The full WSL/NTFS directory-junction credential logic is
//! deferred.

use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};
use ccb_provider_core::caller_env::{caller_context_env, export_env_clause, join_env_prefix};
use ccb_provider_core::runtime_shared::{apply_provider_command_template, provider_start_parts};

const YOLO_FLAG: &str = "--dangerously-skip-permissions";

/// Start-command options specific to AGY.
#[derive(Debug, Clone, Default)]
pub struct AgyStartCommand {
    pub restore: bool,
    pub auto_permission: bool,
    pub provider_command_template: Option<String>,
}

/// Build the shell command that launches an AGY runtime pane.
pub fn build_start_cmd(
    command: &AgyStartCommand,
    spec: &ccb_agents::models::AgentSpec,
    runtime_dir: &Utf8Path,
    launch_session_id: &str,
    _prepared_state: Option<&HashMap<String, String>>,
) -> anyhow::Result<String> {
    let managed_home = resolve_agy_home(runtime_dir);

    let mut cmd_parts = provider_start_parts("agy");
    if command.auto_permission
        && !cmd_parts.iter().any(|p| p == YOLO_FLAG)
        && !spec.startup_args.iter().any(|p| p == YOLO_FLAG)
    {
        cmd_parts.push(YOLO_FLAG.to_string());
    }
    if command.restore
        && !has_restore_arg(&cmd_parts)
        && !has_restore_arg(&spec.startup_args)
    {
        cmd_parts.push("--continue".to_string());
    }
    cmd_parts.extend(spec.startup_args.iter().cloned());

    let cmd = cmd_parts
        .iter()
        .map(|p| shell_quote(p))
        .collect::<Vec<_>>()
        .join(" ");
    let cmd = apply_provider_command_template(&cmd, command.provider_command_template.as_deref())?;

    let mut overrides = HashMap::new();
    overrides.insert("HOME".to_string(), managed_home.to_string());
    overrides.insert("USERPROFILE".to_string(), managed_home.to_string());

    if std::env::var("WSL_DISTRO_NAME").is_ok() {
        let wslenv_additions = "HOME/p:USERPROFILE/p";
        let existing = std::env::var("WSLENV").unwrap_or_default();
        let value = if existing.is_empty() {
            wslenv_additions.to_string()
        } else {
            format!("{}:{}", wslenv_additions, existing)
        };
        overrides.insert("WSLENV".to_string(), value);
    }

    let env_prefix = join_env_prefix(&[
        &export_env_clause(&overrides),
        &export_env_clause(&spec.env),
        &export_env_clause(&ccb_provider_core::caller_env::provider_user_session_env()),
        &export_env_clause(&caller_context_env(
            &spec.name,
            runtime_dir.as_std_path(),
            launch_session_id,
        )),
    ]);

    if env_prefix.is_empty() {
        Ok(cmd)
    } else {
        Ok(format!("{}; {}", env_prefix, cmd))
    }
}

fn resolve_agy_home(runtime_dir: &Utf8Path) -> Utf8PathBuf {
    crate::session_paths::state_dir_for_runtime_dir(runtime_dir)
        .map(|p| Utf8PathBuf::from_path_buf(p.join("home")).unwrap_or_else(|_| runtime_dir.to_path_buf()))
        .unwrap_or_else(|| runtime_dir.join("agy-home"))
}

fn has_restore_arg(parts: &[String]) -> bool {
    parts
        .iter()
        .any(|p| matches!(p.trim(), "--continue" | "-c" | "--conversation"))
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
