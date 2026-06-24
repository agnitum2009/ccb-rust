//! Codex launcher command builder.
//!
//! Mirrors Python `lib/provider_backends/codex/launcher_runtime/command_runtime/service.py`.

use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};
use ccbr_agents::models::AgentSpec;
use ccbr_agents::policy::{resolve_effective_restore_mode, should_restore_provider_history};
use ccbr_provider_core::caller_env::{caller_context_env, provider_user_session_env};
use ccbr_provider_core::runtime_shared::{apply_provider_command_template, provider_start_parts};
use ccbr_provider_profiles::codex_home_config::codex_provider_authority_fingerprint;
use ccbr_provider_profiles::models::{ProviderProfileSpec, ResolvedProviderProfile};
use ccbr_provider_profiles::provider_api_env_keys;

use super::home::prepare_codex_home_overrides;
use super::session_paths::load_resume_session_id;

/// Build the shell command that launches Codex inside a tmux pane.
///
/// Mirrors Python `build_start_cmd` in `command_runtime/service.py`.
#[allow(clippy::too_many_arguments)]
pub fn build_start_cmd(
    command: &CodexStartCommand,
    spec: &AgentSpec,
    runtime_dir: &Utf8Path,
    launch_session_id: &str,
    prepared_state: Option<&CodexLaunchContext>,
    profile: Option<&ResolvedProviderProfile>,
) -> anyhow::Result<String> {
    let launch_context = prepared_state.cloned().unwrap_or_default();
    let project_root = utf8_path_or_none(Some(&launch_context.project_root)).ok_or_else(|| {
        anyhow::anyhow!("Codex launch requires prepare_launch_context before build_start_cmd")
    })?;

    let project_root_utf8 = project_root.clone();
    let workspace_path = utf8_path_or_none(Some(&launch_context.workspace_path));
    let agent_events_path = utf8_path_or_none(Some(&launch_context.agent_events_path));
    let codex_home_overrides = prepare_codex_home_overrides(
        runtime_dir,
        profile,
        false,
        Some(&project_root_utf8),
        Some(&spec.name),
        workspace_path.as_deref(),
        agent_events_path.as_deref(),
        Some(&runtime_dir.join("codex-memory-projection.json")),
    )?;

    let codex_args = codex_args(command, spec, runtime_dir, profile);
    let env_map = env_map(
        runtime_dir,
        launch_session_id,
        spec,
        profile,
        &codex_home_overrides,
    );
    let prefix_parts = build_codex_shell_prefix(profile);
    let exports: Vec<String> = env_map
        .iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| format!("{}={}", k, shlex_quote(v)))
        .collect();
    let mut prefix_parts = prefix_parts;
    if !exports.is_empty() {
        prefix_parts.push(format!("export {}", exports.join(" ")));
    }
    let cmd = codex_args
        .iter()
        .map(|p| shlex_quote(p))
        .collect::<Vec<_>>()
        .join(" ");
    let cmd = apply_provider_command_template(&cmd, command.provider_command_template.as_deref())?;
    if prefix_parts.is_empty() {
        Ok(cmd)
    } else {
        Ok(format!("{}; {}", prefix_parts.join("; "), cmd))
    }
}

/// Build the leading `unset API_KEY` prefix when the profile does not inherit
/// API credentials from the user environment.
pub fn build_codex_shell_prefix(profile: Option<&ResolvedProviderProfile>) -> Vec<String> {
    if profile.map(|p| p.inherit_api).unwrap_or(true) {
        return Vec::new();
    }
    provider_api_env_keys("codex")
        .iter()
        .map(|k| format!("unset {}", k))
        .collect()
}

/// Prepared launch context for Codex.
#[derive(Debug, Clone, Default)]
pub struct CodexLaunchContext {
    pub agent_name: String,
    pub project_root: String,
    pub workspace_path: String,
    pub agent_events_path: String,
}

/// Command flags/behaviour for a Codex launch.
#[derive(Debug, Clone, Default)]
pub struct CodexStartCommand {
    pub restore: bool,
    pub auto_permission: bool,
    pub provider_command_template: Option<String>,
}

fn codex_args(
    command: &CodexStartCommand,
    spec: &AgentSpec,
    runtime_dir: &Utf8Path,
    profile: Option<&ResolvedProviderProfile>,
) -> Vec<String> {
    let mut codex_args = provider_start_parts("codex");
    codex_args.push("-c".to_string());
    codex_args.push("disable_paste_burst=true".to_string());
    if command.auto_permission {
        codex_args.extend([
            "--ask-for-approval".to_string(),
            "never".to_string(),
            "--sandbox".to_string(),
            "danger-full-access".to_string(),
            "--dangerously-bypass-hook-trust".to_string(),
        ]);
    }
    if let Some(model) = spec.model.as_deref().filter(|m| !m.is_empty()) {
        codex_args.push("-m".to_string());
        codex_args.push(model.to_string());
    }
    codex_args.extend(spec.startup_args.iter().cloned());
    let requested_restore = if command.restore {
        Some(ccbr_agents::models::RestoreMode::Provider)
    } else {
        None
    };
    let effective = resolve_effective_restore_mode(spec, None, requested_restore);
    if should_restore_provider_history(spec, effective) {
        let profile_spec = profile.map(resolved_profile_to_spec);
        let session_id = load_resume_session_id(
            spec,
            runtime_dir,
            profile,
            codex_provider_authority_fingerprint(profile_spec.as_ref()).as_deref(),
            None,
        );
        if let Some(session_id) = session_id {
            codex_args.push("resume".to_string());
            codex_args.push(session_id);
        }
    }
    codex_args
}

fn env_map(
    runtime_dir: &Utf8Path,
    launch_session_id: &str,
    spec: &AgentSpec,
    profile: Option<&ResolvedProviderProfile>,
    codex_home_overrides: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = HashMap::new();
    env.extend(provider_user_session_env());
    env.extend(inherited_api_env(profile));
    if let Some(p) = profile {
        env.extend(p.env.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    env.extend(spec.env.iter().map(|(k, v)| (k.clone(), v.clone())));

    // When a Codex API authority is configured, drop any inherited OpenAI route.
    if codex_api_authority(profile).is_some() {
        env.remove("OPENAI_BASE_URL");
        env.remove("OPENAI_API_BASE");
    }

    env.insert(
        "CODEX_RUNTIME_DIR".to_string(),
        runtime_dir.as_str().to_string(),
    );
    env.insert(
        "CODEX_INPUT_FIFO".to_string(),
        runtime_dir.join("input.fifo").as_str().to_string(),
    );
    env.insert(
        "CODEX_OUTPUT_FIFO".to_string(),
        runtime_dir.join("output.fifo").as_str().to_string(),
    );
    env.insert("CODEX_TERMINAL".to_string(), "tmux".to_string());
    env.extend(
        codex_home_overrides
            .iter()
            .map(|(k, v)| (k.clone(), v.clone())),
    );
    env.extend(caller_context_env(
        &spec.name,
        runtime_dir.as_std_path(),
        launch_session_id,
    ));
    env
}

fn inherited_api_env(profile: Option<&ResolvedProviderProfile>) -> HashMap<String, String> {
    if profile.map(|p| p.inherit_api).unwrap_or(true) {
        let keys: std::collections::HashSet<&str> = [
            "OPENAI_API_KEY",
            "OPENAI_BASE_URL",
            "OPENAI_API_BASE",
            "OPENAI_ORG_ID",
            "OPENAI_ORGANIZATION",
        ]
        .iter()
        .copied()
        .collect();
        std::env::vars()
            .filter(|(k, v)| keys.contains(k.as_str()) && !v.trim().is_empty())
            .collect()
    } else {
        HashMap::new()
    }
}

fn codex_api_authority(profile: Option<&ResolvedProviderProfile>) -> Option<String> {
    profile.and_then(|p| {
        let base_url = p
            .env
            .get("OPENAI_BASE_URL")
            .or_else(|| p.env.get("OPENAI_API_BASE"))?;
        if base_url.trim().is_empty() {
            return None;
        }
        Some(base_url.clone())
    })
}

fn utf8_path_or_none(value: Option<&str>) -> Option<Utf8PathBuf> {
    let raw = value.unwrap_or("").trim();
    if raw.is_empty() {
        return None;
    }
    Some(Utf8PathBuf::from(raw))
}

fn resolved_profile_to_spec(profile: &ResolvedProviderProfile) -> ProviderProfileSpec {
    ProviderProfileSpec {
        mode: profile.mode.trim().to_lowercase(),
        home: profile.runtime_home.clone(),
        env: profile.env.clone(),
        inherit_api: profile.inherit_api,
        inherit_auth: profile.inherit_auth,
        inherit_config: profile.inherit_config,
        inherit_skills: profile.inherit_skills,
        inherit_commands: profile.inherit_commands,
        inherit_memory: profile.inherit_memory,
    }
}

fn shlex_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let safe = s
        .chars()
        .all(|c| c.is_alphanumeric() || "_-.,/:~=@%".contains(c));
    if safe {
        return s.to_string();
    }
    let mut out = String::from("'");
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_codex_shell_prefix_inherit_api() {
        let profile = ResolvedProviderProfile::new("codex", "agent1");
        assert!(build_codex_shell_prefix(Some(&profile)).is_empty());
    }
}
