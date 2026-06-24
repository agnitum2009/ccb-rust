//! Mirrors Python `lib/provider_backends/claude/launcher_runtime/home.py`.
//!
//! Home-layout resolution for an isolated Claude runtime.

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use ccb_provider_profiles::models::ResolvedProviderProfile;

use ccb_memory::render_provider_home_memory;

use crate::claude::home_layout::{claude_layout_for_home, ClaudeHomeLayout};

/// Resolve the isolated Claude home layout for a runtime directory.
///
/// Mirrors Python `resolve_claude_home_layout`.
pub fn resolve_claude_home_layout(
    runtime_dir: &Utf8Path,
    _profile: Option<&ResolvedProviderProfile>,
) -> ClaudeHomeLayout {
    // Mirror Python: profile runtime_home is intentionally not used for Claude
    // layout resolution.
    let runtime_dir = provider_state_dir_for_runtime_dir(runtime_dir);
    let managed_home = managed_isolated_home(&runtime_dir);
    if let Some(existing) = existing_layout(&runtime_dir, &managed_home) {
        return existing;
    }

    claude_layout_for_home(managed_home)
}

fn provider_state_dir_for_runtime_dir(runtime_dir: &Utf8Path) -> Utf8PathBuf {
    let s = runtime_dir.as_str();
    let needle = "/provider-runtime/";
    if let Some(pos) = s.find(needle) {
        let prefix = &s[..=pos]; // includes the leading '/'
        let suffix = &s[pos + needle.len() - 1..]; // keep trailing '/' of original segment
        return Utf8PathBuf::from(format!("{}provider-state{}", prefix, suffix));
    }
    if let Some(prefix) = s.strip_suffix("/provider-runtime") {
        return Utf8PathBuf::from(format!("{}/provider-state", prefix));
    }
    runtime_dir.to_path_buf()
}

fn managed_isolated_home(runtime_dir: &Utf8Path) -> Utf8PathBuf {
    runtime_dir.join("home")
}

fn existing_layout(runtime_dir: &Utf8Path, managed_home: &Utf8Path) -> Option<ClaudeHomeLayout> {
    // If the managed home already has a settings file, use it.
    let managed_settings = managed_home.join(".claude").join("settings.json");
    if managed_settings.exists() {
        return Some(claude_layout_for_home(managed_home));
    }

    // Otherwise, look for a pre-existing home directory inside the runtime.
    let candidates: Vec<Utf8PathBuf> = vec![
        runtime_dir.join("home"),
        runtime_dir.join(".claude").join("home"),
    ];
    for candidate in candidates {
        if candidate.join(".claude").join("settings.json").exists() {
            return Some(claude_layout_for_home(&candidate));
        }
    }
    None
}

/// Materialize a managed Claude home directory.
///
/// Mirrors Python `materialize_claude_home_config`. This is a minimal parity
/// implementation: it creates the home layout, copies inherited settings/auth,
/// routes inherited skills/commands, and writes a simple memory bundle.
#[allow(clippy::too_many_arguments)]
pub fn materialize_claude_home_config(
    target_home: &Utf8Path,
    profile: Option<&ResolvedProviderProfile>,
    source_home: Option<&Utf8Path>,
    project_root: Option<&Utf8Path>,
    agent_name: Option<&str>,
    workspace_path: Option<&Utf8Path>,
    auto_permission: bool,
    memory_projection_event_path: Option<&Utf8Path>,
    memory_projection_marker_path: Option<&Utf8Path>,
) -> anyhow::Result<ClaudeHomeLayout> {
    let layout = claude_layout_for_home(target_home);

    std::fs::create_dir_all(&layout.home_root)?;
    std::fs::create_dir_all(&layout.claude_dir)?;
    std::fs::create_dir_all(&layout.projects_root)?;
    std::fs::create_dir_all(&layout.session_env_root)?;

    let source_root: Utf8PathBuf = source_home.map(expand_user_path).unwrap_or_else(|| {
        Utf8PathBuf::from_path_buf(ccb_provider_core::source_home::current_provider_source_home())
            .unwrap_or_else(|_| Utf8PathBuf::from("/tmp"))
    });

    let memory_result = if layout.home_root == source_root {
        ensure_trust_file(&layout.trust_path)?;
        ccb_provider_core::memory_projection::memory_projection_result(
            "skipped",
            "source_home_is_target_home",
            layout.claude_dir.join("CLAUDE.md").as_std_path(),
            None,
            None,
            None,
            None,
        )
    } else {
        materialize_claude_settings(&source_root, &layout, profile, auto_permission)?;
        materialize_claude_auth(&source_root, &layout, profile)?;
        materialize_claude_trust(&source_root, &layout, profile)?;
        route_claude_inherited_tree(
            &source_root.join(".claude").join("commands"),
            &layout.claude_dir.join("commands"),
            inherits_commands(profile),
            "claude-inherited-commands",
        )?;
        route_claude_inherited_tree(
            &source_root.join(".claude").join("skills"),
            &layout.claude_dir.join("skills"),
            inherits_skills(profile),
            "claude-inherited-skills",
        )?;
        materialize_claude_memory(
            &source_root,
            &layout,
            profile,
            project_root,
            agent_name,
            workspace_path,
        )?
    };

    ccb_provider_core::memory_projection::record_memory_projection_event(
        &memory_result,
        "claude",
        memory_projection_event_path.map(|p| p.as_std_path()),
        memory_projection_marker_path.map(|p| p.as_std_path()),
        agent_name,
    )
    .with_context(|| "failed to record claude memory projection event")?;

    Ok(layout)
}

/// Prepare the `HOME` / `CLAUDE_PROJECTS_ROOT` / `CLAUDE_PROJECT_ROOT`
/// overrides for a Claude runtime.
#[allow(clippy::too_many_arguments)]
pub fn prepare_claude_home_overrides(
    runtime_dir: &Utf8Path,
    profile: Option<&ResolvedProviderProfile>,
    refresh_home: bool,
    auto_permission: bool,
    project_root: Option<&Utf8Path>,
    agent_name: Option<&str>,
    workspace_path: Option<&Utf8Path>,
    memory_projection_event_path: Option<&Utf8Path>,
    memory_projection_marker_path: Option<&Utf8Path>,
) -> anyhow::Result<std::collections::HashMap<String, String>> {
    let layout = resolve_claude_home_layout(runtime_dir, profile);
    if refresh_home {
        materialize_claude_home_config(
            &layout.home_root,
            profile,
            None,
            project_root,
            agent_name,
            workspace_path,
            auto_permission,
            memory_projection_event_path,
            memory_projection_marker_path,
        )?;
    }

    let mut overrides = std::collections::HashMap::new();
    overrides.insert("HOME".to_string(), layout.home_root.to_string());
    overrides.insert(
        "CLAUDE_PROJECTS_ROOT".to_string(),
        layout.projects_root.to_string(),
    );
    overrides.insert(
        "CLAUDE_PROJECT_ROOT".to_string(),
        layout.projects_root.to_string(),
    );

    if std::env::var("WSL_DISTRO_NAME").is_ok() {
        overrides.insert("USERPROFILE".to_string(), layout.home_root.to_string());
        let wslenv_additions = "HOME/p:USERPROFILE/p:CLAUDE_PROJECTS_ROOT/p:CLAUDE_PROJECT_ROOT/p:\
                                ANTHROPIC_AUTH_TOKEN:ANTHROPIC_API_KEY:ANTHROPIC_BASE_URL";
        let existing = std::env::var("WSLENV").unwrap_or_default();
        let value = if existing.is_empty() {
            wslenv_additions.to_string()
        } else {
            format!("{}:{}", wslenv_additions, existing)
        };
        overrides.insert("WSLENV".to_string(), value);
    }

    Ok(overrides)
}

fn materialize_claude_settings(
    source_home: &Utf8Path,
    layout: &ClaudeHomeLayout,
    profile: Option<&ResolvedProviderProfile>,
    auto_permission: bool,
) -> anyhow::Result<()> {
    let source_settings = source_home.join(".claude").join("settings.json");
    let mut payload = read_json_object(&source_settings);

    if !inherits_config(profile) {
        // Keep only hooks/permissions from source when config inheritance is disabled.
        let hooks = payload.get("hooks").cloned();
        let permissions = payload.get("permissions").cloned();
        payload = serde_json::Map::new();
        if let Some(h) = hooks {
            payload.insert("hooks".into(), h);
        }
        if let Some(p) = permissions {
            payload.insert("permissions".into(), p);
        }
    }

    if !inherits_api(profile) {
        // Strip API env keys from the projected env.
        if let Some(env) = payload.get_mut("env").and_then(|v| v.as_object_mut()) {
            for key in ccb_provider_profiles::provider_api_env_keys("claude") {
                env.remove(&key);
            }
        }
    }

    if auto_permission {
        let allowed = payload
            .entry("allowedTools".to_string())
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        if let Some(arr) = allowed.as_array_mut() {
            let marker = serde_json::Value::String("Bash(ccb ".into());
            if !arr.contains(&marker) {
                arr.push(marker);
            }
        }
    }

    if payload.is_empty() {
        let _ = std::fs::remove_file(&layout.settings_path);
        return Ok(());
    }

    std::fs::create_dir_all(layout.settings_path.parent().unwrap_or(&layout.claude_dir))?;
    ccb_storage::atomic::atomic_write_json(
        &layout.settings_path,
        &serde_json::Value::Object(payload),
    )?;
    Ok(())
}

fn materialize_claude_auth(
    source_home: &Utf8Path,
    layout: &ClaudeHomeLayout,
    profile: Option<&ResolvedProviderProfile>,
) -> anyhow::Result<()> {
    if !inherits_auth(profile) {
        let _ = std::fs::remove_file(&layout.auth_path);
        let _ = std::fs::remove_file(&layout.credentials_path);
        return Ok(());
    }

    let source_auth = source_home
        .join(".config")
        .join("claude-code")
        .join("auth.json");
    if source_auth.is_file() {
        std::fs::create_dir_all(layout.auth_path.parent().unwrap_or(&layout.claude_dir))?;
        std::fs::copy(&source_auth, &layout.auth_path)?;
    }

    let source_creds = source_home.join(".claude").join(".credentials.json");
    if source_creds.is_file() {
        std::fs::copy(&source_creds, &layout.credentials_path)?;
    }

    Ok(())
}

fn materialize_claude_trust(
    source_home: &Utf8Path,
    layout: &ClaudeHomeLayout,
    profile: Option<&ResolvedProviderProfile>,
) -> anyhow::Result<()> {
    if !inherits_config(profile) {
        ensure_trust_file(&layout.trust_path)?;
        return Ok(());
    }

    let source_trust = source_home.join(".claude.json");
    let mut merged = if source_trust.is_file() {
        read_json_object(&source_trust)
    } else {
        serde_json::Map::new()
    };

    let existing = read_json_object(&layout.trust_path);
    for (k, v) in existing {
        merged.entry(k).or_insert(v);
    }

    write_json_object(&layout.trust_path, &merged)?;
    ensure_trust_file(&layout.trust_path)?;
    Ok(())
}

fn route_claude_inherited_tree(
    source: &Utf8Path,
    target: &Utf8Path,
    enabled: bool,
    label: &str,
) -> anyhow::Result<()> {
    ccb_provider_core::projected_assets::route_projected_tree(
        source.as_std_path(),
        target.as_std_path(),
        enabled,
        label,
        None,
        true,
    )
    .with_context(|| format!("failed to route claude inherited tree {label}"))?;
    Ok(())
}

fn materialize_claude_memory(
    source_home: &Utf8Path,
    layout: &ClaudeHomeLayout,
    profile: Option<&ResolvedProviderProfile>,
    project_root: Option<&Utf8Path>,
    agent_name: Option<&str>,
    workspace_path: Option<&Utf8Path>,
) -> anyhow::Result<ccb_provider_core::memory_projection::MemoryProjectionResult> {
    let target = layout.claude_dir.join("CLAUDE.md");

    if !inherits_memory(profile) {
        let _ = std::fs::remove_file(&target);
        return Ok(
            ccb_provider_core::memory_projection::memory_projection_result(
                "skipped",
                "inherit_memory_disabled",
                target.as_std_path(),
                None,
                None,
                None,
                None,
            ),
        );
    }

    let (Some(project_root), Some(agent_name)) = (project_root, agent_name) else {
        return Ok(
            ccb_provider_core::memory_projection::memory_projection_result(
                "failed",
                "missing_project_context",
                target.as_std_path(),
                None,
                None,
                None,
                None,
            ),
        );
    };

    let source_memory = source_home.join(".claude").join("CLAUDE.md");
    let rendered = render_provider_home_memory(
        project_root.as_std_path(),
        agent_name,
        "claude",
        workspace_path.map(|p| p.as_std_path()),
        Some(source_memory.as_std_path()),
    )
    .map_err(|e| anyhow::anyhow!("failed to render claude memory bundle: {e}"))?;

    if rendered.trim().is_empty() {
        let _ = std::fs::remove_file(&target);
        return Ok(
            ccb_provider_core::memory_projection::memory_projection_result(
                "skipped",
                "no_memory_sources",
                target.as_std_path(),
                None,
                None,
                None,
                None,
            ),
        );
    }

    std::fs::create_dir_all(target.parent().unwrap_or(&layout.claude_dir))?;
    ccb_storage::atomic::atomic_write_text(&target, &rendered)?;
    let sha = ccb_provider_core::memory_projection::text_file_sha256(target.as_std_path());
    let sha_ref = if sha.is_empty() {
        None
    } else {
        Some(sha.as_str())
    };
    Ok(
        ccb_provider_core::memory_projection::memory_projection_result(
            "ok",
            "written",
            target.as_std_path(),
            sha_ref,
            Some(1),
            None,
            None,
        ),
    )
}

fn ensure_trust_file(path: &Utf8Path) -> anyhow::Result<()> {
    if path.is_file() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    ccb_storage::atomic::atomic_write_json(path, &serde_json::json!({}))?;
    Ok(())
}

fn read_json_object(path: &Utf8Path) -> serde_json::Map<String, serde_json::Value> {
    if !path.is_file() {
        return serde_json::Map::new();
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn write_json_object(
    path: &Utf8Path,
    obj: &serde_json::Map<String, serde_json::Value>,
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    ccb_storage::atomic::atomic_write_json(path, &serde_json::Value::Object(obj.clone()))?;
    Ok(())
}

fn inherits_config(profile: Option<&ResolvedProviderProfile>) -> bool {
    profile.map(|p| p.inherit_config).unwrap_or(true)
}

fn inherits_api(profile: Option<&ResolvedProviderProfile>) -> bool {
    profile.map(|p| p.inherit_api).unwrap_or(true)
}

fn inherits_auth(profile: Option<&ResolvedProviderProfile>) -> bool {
    profile.map(|p| p.inherit_auth).unwrap_or(true)
}

fn inherits_skills(profile: Option<&ResolvedProviderProfile>) -> bool {
    profile.map(|p| p.inherit_skills).unwrap_or(true)
}

fn inherits_commands(profile: Option<&ResolvedProviderProfile>) -> bool {
    profile.map(|p| p.inherit_commands).unwrap_or(true)
}

fn inherits_memory(profile: Option<&ResolvedProviderProfile>) -> bool {
    profile.map(|p| p.inherit_memory).unwrap_or(true)
}

fn expand_user_path(path: &Utf8Path) -> Utf8PathBuf {
    let s = path.as_str();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return Utf8PathBuf::from(home).join(rest);
        }
    }
    if s == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return Utf8PathBuf::from(home);
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_claude_home_layout_ignores_profile_runtime_home() {
        let runtime_dir =
            Utf8PathBuf::from("/tmp/proj/.ccbr/agents/agent1/provider-runtime/claude");
        let mut profile = ResolvedProviderProfile::new("claude", "agent1");
        profile.runtime_home = Some("/custom/runtime/home".to_string());

        let layout = resolve_claude_home_layout(&runtime_dir, Some(&profile));
        assert_eq!(
            layout.home_root,
            Utf8PathBuf::from("/tmp/proj/.ccbr/agents/agent1/provider-state/claude/home")
        );
    }

    #[test]
    fn test_resolve_claude_home_layout_maps_provider_runtime_to_state_dir() {
        let runtime_dir =
            Utf8PathBuf::from("/tmp/proj/.ccbr/agents/agent1/provider-runtime/claude");
        let layout = resolve_claude_home_layout(&runtime_dir, None);
        assert_eq!(
            layout.home_root,
            Utf8PathBuf::from("/tmp/proj/.ccbr/agents/agent1/provider-state/claude/home")
        );
    }
}
