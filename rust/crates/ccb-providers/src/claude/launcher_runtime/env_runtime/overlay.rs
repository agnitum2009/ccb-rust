//! Mirrors Python `lib/provider_backends/claude/launcher_runtime/env_runtime/overlay.py`.

use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};
use ccb_provider_profiles::models::ResolvedProviderProfile;

/// Write a sanitized copy of the agent's profile `settings.json` into the
/// runtime directory for Claude's `--settings` argument.
pub fn write_claude_settings_overlay(
    runtime_dir: &Utf8Path,
    profile: Option<&ResolvedProviderProfile>,
) -> Option<Utf8PathBuf> {
    let payload = read_agent_settings_payload(profile)?;
    let sanitized = sanitized_settings_overlay(&payload);
    if sanitized.is_empty() {
        return None;
    }
    let path = runtime_dir.join("claude-settings.json");
    let text = serde_json::to_string_pretty(&sanitized).ok()?;
    std::fs::write(&path, text).ok()?;
    Some(path)
}

/// Read the user's `~/.claude/settings.json` `ANTHROPIC_BASE_URL` value.
pub fn claude_user_base_url(user_settings_path: &Utf8Path) -> String {
    let payload = read_user_settings_payload(user_settings_path);
    let env = payload
        .as_ref()
        .and_then(|p| p.get("env"))
        .and_then(|v| v.as_object());
    env.and_then(|o| o.get("ANTHROPIC_BASE_URL"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub fn read_user_settings_payload(
    user_settings_path: &Utf8Path,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    read_settings_payload(user_settings_path)
}

pub fn read_settings_payload(
    path: &Utf8Path,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.as_object().cloned()
}

pub fn read_agent_settings_payload(
    profile: Option<&ResolvedProviderProfile>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let path = agent_settings_path(profile)?;
    read_settings_payload(&path)
}

pub fn agent_settings_path(profile: Option<&ResolvedProviderProfile>) -> Option<Utf8PathBuf> {
    let profile_root = profile?.profile_root.as_deref()?;
    let profile_root = profile_root.trim();
    if profile_root.is_empty() {
        return None;
    }
    let path = Utf8PathBuf::from(profile_root).join("settings.json");
    if !path.is_file() {
        return None;
    }
    Some(path)
}

pub fn sanitized_settings_overlay(
    payload: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    payload
        .iter()
        .filter(|(k, _)| k.as_str() != "env")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// API environment keys that are eligible for explicit export/unset handling.
pub fn collect_explicit_api_env(
    profile: Option<&ResolvedProviderProfile>,
    extra_env: Option<&HashMap<String, String>>,
) -> HashMap<String, String> {
    let api_keys = ccb_provider_profiles::provider_api_env_keys("claude");
    let mut explicit = HashMap::new();
    if let Some(profile) = profile {
        explicit.extend(filtered_api_env(&profile.env, &api_keys));
    }
    if let Some(extra) = extra_env {
        explicit.extend(filtered_api_env(extra, &api_keys));
    }
    explicit
}

fn filtered_api_env(
    env_map: &HashMap<String, String>,
    api_keys: &std::collections::HashSet<String>,
) -> HashMap<String, String> {
    env_map
        .iter()
        .filter(|(k, _)| api_keys.contains(k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}
