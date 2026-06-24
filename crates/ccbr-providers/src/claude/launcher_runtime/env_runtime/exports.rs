//! Mirrors Python `lib/provider_backends/claude/launcher_runtime/env_runtime/exports.py`.

use std::collections::HashMap;

use camino::Utf8PathBuf;
use ccbr_provider_profiles::models::ResolvedProviderProfile;

use crate::claude::launcher_runtime::env_runtime::overlay::collect_explicit_api_env;

/// Build the environment-variable shell prefix for launching Claude.
///
/// The result is a `; `-separated list of `unset` and `export` statements that
/// can be prepended to the provider command.
pub fn build_claude_env_prefix<F, G>(
    profile: Option<&ResolvedProviderProfile>,
    extra_env: Option<&HashMap<String, String>>,
    env: Option<&HashMap<String, String>>,
    should_drop_base_url_fn: F,
    claude_user_base_url_fn: G,
) -> String
where
    F: Fn(&str) -> bool,
    G: Fn() -> String,
{
    let api_keys = ccbr_provider_profiles::provider_api_env_keys("claude");
    let mut explicit_env = collect_explicit_api_env(profile, extra_env);
    let mut parts = unset_api_env_parts(profile, &api_keys);

    explicit_env = reconcile_base_url(
        explicit_env,
        profile,
        env.unwrap_or(&HashMap::new()),
        &mut parts,
        should_drop_base_url_fn,
        claude_user_base_url_fn,
    );

    let export_statement = ccbr_provider_core::caller_env::export_env_clause(&explicit_env);
    if !export_statement.is_empty() {
        parts.push(export_statement);
    }
    parts
        .into_iter()
        .filter(|p| !p.trim().is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Environment parts that redirect `HOME` and Claude-specific roots to a
/// profile/runtime-managed home directory.
pub fn runtime_home_env_parts(profile: Option<&ResolvedProviderProfile>) -> Vec<String> {
    let runtime_home = match profile {
        Some(p) => p.runtime_home.as_deref().filter(|s| !s.trim().is_empty()),
        None => None,
    };
    let runtime_home = match runtime_home {
        Some(h) => Utf8PathBuf::from(h),
        None => return Vec::new(),
    };
    let claude_dir = runtime_home.join(".claude");
    vec![
        "unset CODEX_HOME".to_string(),
        "unset CODEX_SESSION_ROOT".to_string(),
        format!("export HOME={}", shell_quote(runtime_home.as_str())),
        format!(
            "export CLAUDE_CONFIG_DIR={}",
            shell_quote(claude_dir.as_str())
        ),
        format!(
            "export CLAUDE_PROJECTS_ROOT={}",
            shell_quote(claude_dir.join("projects").as_str())
        ),
        format!(
            "export CLAUDE_SESSION_ENV_ROOT={}",
            shell_quote(claude_dir.join("session-env").as_str())
        ),
    ]
}

fn unset_api_env_parts(
    profile: Option<&ResolvedProviderProfile>,
    api_keys: &std::collections::HashSet<String>,
) -> Vec<String> {
    if profile.map(|p| p.inherit_api).unwrap_or(true) {
        return Vec::new();
    }
    let mut keys: Vec<_> = api_keys.iter().cloned().collect();
    keys.sort();
    keys.into_iter().map(|k| format!("unset {}", k)).collect()
}

fn reconcile_base_url<F, G>(
    mut explicit_env: HashMap<String, String>,
    profile: Option<&ResolvedProviderProfile>,
    env: &HashMap<String, String>,
    parts: &mut Vec<String>,
    should_drop_base_url_fn: F,
    claude_user_base_url_fn: G,
) -> HashMap<String, String>
where
    F: Fn(&str) -> bool,
    G: Fn() -> String,
{
    if let Some(base_url) = explicit_env.get("ANTHROPIC_BASE_URL").cloned() {
        if should_drop_base_url_fn(&base_url) {
            explicit_env.remove("ANTHROPIC_BASE_URL");
            ensure_unset(parts, "ANTHROPIC_BASE_URL");
        }
        return explicit_env;
    }

    if profile.map(|p| !p.inherit_api).unwrap_or(false) {
        return explicit_env;
    }

    let inherited = inherited_base_url_value(env, claude_user_base_url_fn);
    if inherited.is_empty() {
        return explicit_env;
    }
    if should_drop_base_url_fn(&inherited) {
        ensure_unset(parts, "ANTHROPIC_BASE_URL");
        return explicit_env;
    }
    explicit_env.insert("ANTHROPIC_BASE_URL".to_string(), inherited);
    explicit_env
}

fn inherited_base_url_value<G>(env: &HashMap<String, String>, claude_user_base_url_fn: G) -> String
where
    G: Fn() -> String,
{
    let settings_url = claude_user_base_url_fn().trim().to_string();
    if !settings_url.is_empty() {
        return settings_url;
    }
    env.get("ANTHROPIC_BASE_URL")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

fn ensure_unset(parts: &mut Vec<String>, key: &str) {
    let statement = format!("unset {}", key);
    if !parts.contains(&statement) {
        parts.push(statement);
    }
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|c| c.is_alphanumeric() || "_-./:=@".contains(c))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_claude_env_prefix_unsets_dead_local_base_url_from_env() {
        let env = {
            let mut m = HashMap::new();
            m.insert(
                "ANTHROPIC_BASE_URL".to_string(),
                "http://127.0.0.1:12345".to_string(),
            );
            m
        };
        let result = build_claude_env_prefix(None, None, Some(&env), |_| true, String::new);
        assert_eq!(result, "unset ANTHROPIC_BASE_URL");
    }

    #[test]
    fn test_build_claude_env_prefix_uses_settings_base_url_when_inheritable() {
        let result = build_claude_env_prefix(
            None,
            None,
            None,
            |_| false,
            || "https://api.example.test".to_string(),
        );
        assert_eq!(result, "export ANTHROPIC_BASE_URL=https://api.example.test");
    }

    #[test]
    fn test_build_claude_env_prefix_prefers_settings_base_url_over_ambient_env() {
        let env = {
            let mut m = HashMap::new();
            m.insert(
                "ANTHROPIC_BASE_URL".to_string(),
                "https://old-shell.example.test".to_string(),
            );
            m
        };
        let result = build_claude_env_prefix(
            None,
            None,
            Some(&env),
            |_| false,
            || "https://ccswitch.example.test".to_string(),
        );
        assert_eq!(
            result,
            "export ANTHROPIC_BASE_URL=https://ccswitch.example.test"
        );
    }
}
