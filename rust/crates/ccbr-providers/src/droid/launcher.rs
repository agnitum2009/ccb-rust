//! Mirrors Python `lib/provider_backends/droid/launcher.py`.

use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};
use ccbr_provider_core::caller_env::{caller_context_env, export_env_clause, join_env_prefix};
use ccbr_provider_core::runtime_shared::{apply_provider_command_template, provider_start_parts};

use crate::droid::paths::managed_droid_home_for_runtime;

/// Start-command options specific to Droid.
#[derive(Debug, Clone, Default)]
pub struct DroidStartCommand {
    pub restore: bool,
    pub provider_command_template: Option<String>,
}

/// Build the shell command that launches a Droid runtime pane.
pub fn build_start_cmd(
    command: &DroidStartCommand,
    spec: &ccbr_agents::models::AgentSpec,
    runtime_dir: &Utf8Path,
    launch_session_id: &str,
    prepared_state: Option<&HashMap<String, String>>,
) -> anyhow::Result<String> {
    let mut cmd_parts = provider_start_parts("droid");
    if command.restore {
        cmd_parts.push("-r".to_string());
    }
    cmd_parts.extend(spec.startup_args.iter().cloned());

    let cmd = cmd_parts
        .iter()
        .map(|p| shell_quote(p))
        .collect::<Vec<_>>()
        .join(" ");
    let cmd = apply_provider_command_template(&cmd, command.provider_command_template.as_deref())?;

    let prepared_state = prepared_state.cloned().unwrap_or_default();
    let droid_home = droid_home(runtime_dir, &prepared_state);
    let droid_sessions_root = droid_sessions_root(&droid_home, &prepared_state);

    let mut env = HashMap::new();
    env.insert("FACTORY_HOME".to_string(), droid_home.to_string());
    env.insert(
        "FACTORY_SESSIONS_ROOT".to_string(),
        droid_sessions_root.to_string(),
    );
    env.insert(
        "DROID_SESSIONS_ROOT".to_string(),
        droid_sessions_root.to_string(),
    );

    let env_prefix = join_env_prefix(&[
        &export_env_clause(&env),
        &export_env_clause(&ccbr_provider_core::caller_env::provider_user_session_env()),
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

fn droid_home(runtime_dir: &Utf8Path, prepared_state: &HashMap<String, String>) -> Utf8PathBuf {
    prepared_state
        .get("droid_home")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(Utf8PathBuf::from)
        .unwrap_or_else(|| {
            Utf8PathBuf::from_path_buf(managed_droid_home_for_runtime(runtime_dir.as_std_path()))
                .unwrap_or_else(|_| runtime_dir.to_path_buf())
        })
}

fn droid_sessions_root(
    droid_home: &Utf8Path,
    prepared_state: &HashMap<String, String>,
) -> Utf8PathBuf {
    prepared_state
        .get("droid_sessions_root")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(Utf8PathBuf::from)
        .unwrap_or_else(|| droid_home.join("sessions"))
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
