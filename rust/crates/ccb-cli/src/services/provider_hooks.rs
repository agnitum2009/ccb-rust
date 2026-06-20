//! Mirrors Python `lib/cli/services/provider_hooks.py`.
//!
//! CLI-side orchestration for installing provider finish/activity hooks into a
//! managed provider home.

use std::io::Write;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use ccb_agents::models::AgentSpec;
use ccb_provider_core::source_home::current_provider_source_home;
use ccb_provider_hooks::settings::{
    build_activity_hook_command, build_hook_command, install_workspace_activity_hooks,
    install_workspace_completion_hooks_with_profile,
};
use ccb_provider_profiles::codex_home_config::{
    materialize_codex_home_config, resolve_codex_home_layout,
};
use ccb_provider_profiles::materializer::{
    load_resolved_provider_profile, materialize_provider_profile,
};
use ccb_provider_profiles::models::ResolvedProviderProfile;
use ccb_providers::claude::launcher_runtime::binary_cache::route_claude_binary_cache;
use ccb_providers::claude::launcher_runtime::home::{
    materialize_claude_home_config, resolve_claude_home_layout,
};
use ccb_providers::droid::paths::materialize_droid_home_config;
use ccb_providers::opencode::launcher::materialize_opencode_memory_config;
use ccb_providers::providers::gemini::materialize_gemini_home_config;
use ccb_storage::paths::PathLayout;

/// Prepare the completion (and optionally activity) hooks for a provider
/// workspace.
///
/// Mirrors Python `prepare_workspace_provider_hooks`. Only `claude` and
/// `gemini` receive completion hooks; only `claude` receives activity hooks
/// when both `project_id` and `runtime_dir` are supplied.
#[allow(clippy::too_many_arguments)]
pub fn prepare_workspace_provider_hooks(
    provider: &str,
    workspace_path: &Utf8Path,
    completion_dir: &Utf8Path,
    agent_name: &str,
    home_root: Option<&Utf8Path>,
    project_id: Option<&str>,
    runtime_dir: Option<&Utf8Path>,
    resolved_profile: Option<&serde_json::Value>,
) -> Option<Utf8PathBuf> {
    let normalized = provider.trim().to_lowercase();
    if !matches!(normalized.as_str(), "claude" | "gemini") {
        return None;
    }

    let finish_hook = provider_hook_binary_path("ccb-provider-finish-hook")?;
    let command = build_hook_command(
        &normalized,
        &finish_hook,
        "",
        completion_dir,
        agent_name,
        workspace_path,
    );
    let settings_path = install_workspace_completion_hooks_with_profile(
        &normalized,
        workspace_path,
        home_root,
        &command,
        resolved_profile,
    );

    if normalized == "claude" {
        if let (Some(project_id), Some(runtime_dir)) = (project_id, runtime_dir) {
            let activity_hook = provider_hook_binary_path("ccb-provider-activity-hook")?;
            let activity_command = build_activity_hook_command(
                &normalized,
                &activity_hook,
                "",
                project_id,
                agent_name,
                runtime_dir,
                workspace_path,
            );
            return install_workspace_activity_hooks(
                &normalized,
                workspace_path,
                home_root,
                &activity_command,
            )
            .or(settings_path);
        }
    }

    settings_path
}

/// Prepare the provider workspace: materialize the profile, provider home, and
/// workspace hooks.
///
/// Mirrors Python `prepare_provider_workspace`.
#[allow(clippy::too_many_arguments)]
pub fn prepare_provider_workspace(
    layout: &PathLayout,
    spec: &AgentSpec,
    workspace_path: &Utf8Path,
    completion_dir: &Utf8Path,
    agent_name: &str,
    refresh_profile: bool,
    auto_permission: bool,
) -> anyhow::Result<ResolvedProviderProfile> {
    let runtime_dir = layout.agent_provider_runtime_dir(&spec.name, &spec.provider);

    let resolved_profile = if refresh_profile {
        materialize_provider_profile(
            layout,
            &spec.name,
            &spec.provider,
            &spec.provider_profile,
            workspace_path,
        )
        .with_context(|| format!("failed to materialize {} profile", spec.provider))?
    } else {
        match load_resolved_provider_profile(&runtime_dir) {
            Some(profile) => profile,
            None => materialize_provider_profile(
                layout,
                &spec.name,
                &spec.provider,
                &spec.provider_profile,
                workspace_path,
            )
            .with_context(|| format!("failed to materialize {} profile", spec.provider))?,
        }
    };

    _materialize_provider_home(
        layout,
        spec,
        &runtime_dir,
        &resolved_profile,
        workspace_path,
        auto_permission,
    )
    .with_context(|| format!("failed to materialize {} home", spec.provider))?;

    let profile_value = serde_json::to_value(&resolved_profile)
        .with_context(|| "failed to serialize resolved profile")?;
    prepare_workspace_provider_hooks(
        &spec.provider,
        workspace_path,
        completion_dir,
        agent_name,
        provider_hook_home_root(
            layout,
            &spec.provider,
            agent_name,
            &runtime_dir,
            Some(&resolved_profile),
        )
        .as_deref(),
        Some(layout.project_id()),
        Some(&runtime_dir),
        Some(&profile_value),
    );

    Ok(resolved_profile)
}

/// Return the managed provider home root used for hook installation.
///
/// Mirrors Python `provider_hook_home_root`.
pub fn provider_hook_home_root(
    layout: &PathLayout,
    provider: &str,
    agent_name: &str,
    runtime_dir: &Utf8Path,
    resolved_profile: Option<&ResolvedProviderProfile>,
) -> Option<Utf8PathBuf> {
    let normalized = provider.trim().to_lowercase();
    match normalized.as_str() {
        "claude" => Some(resolve_claude_home_layout(runtime_dir, resolved_profile).home_root),
        "gemini" => Some(resolve_gemini_home_root(layout, agent_name)),
        _ => None,
    }
}

/// Resolve the Gemini managed home root.
///
/// Mirrors Python `resolve_gemini_home_root`.
pub fn resolve_gemini_home_root(layout: &PathLayout, agent_name: &str) -> Utf8PathBuf {
    layout
        .agent_provider_state_dir(agent_name, "gemini")
        .join("home")
}

fn _materialize_provider_home(
    layout: &PathLayout,
    spec: &AgentSpec,
    runtime_dir: &Utf8Path,
    resolved_profile: &ResolvedProviderProfile,
    workspace_path: &Utf8Path,
    auto_permission: bool,
) -> anyhow::Result<()> {
    let provider = spec.provider.trim().to_lowercase();
    let source_home = Utf8PathBuf::from_path_buf(current_provider_source_home())
        .unwrap_or_else(|_| Utf8PathBuf::from("/tmp"));

    match provider.as_str() {
        "claude" => {
            let home_root =
                resolve_claude_home_layout(runtime_dir, Some(resolved_profile)).home_root;
            let event_path = layout.agent_events_path(&spec.name);
            let marker_path = runtime_dir.join("claude-memory-projection.json");
            materialize_claude_home_config(
                &home_root,
                Some(resolved_profile),
                Some(&source_home),
                Some(&layout.project_root),
                Some(&spec.name),
                Some(workspace_path),
                auto_permission,
                Some(&event_path),
                Some(&marker_path),
            )?;
            let cache_root = layout
                .ensure_provider_external_cache_dir("claude", None)
                .with_context(|| "failed to ensure claude external cache dir")?;
            route_claude_binary_cache(&home_root, &cache_root, Some(&source_home))
                .with_context(|| format!("failed to route claude binary cache to {cache_root}"))?;
            _record_claude_binary_cache_drift_if_present(layout, spec, runtime_dir, &home_root)?;
        }
        "codex" => {
            let codex_home =
                resolve_codex_home_layout(runtime_dir, Some(resolved_profile)).codex_home;
            let event_path = layout.agent_events_path(&spec.name);
            let marker_path = runtime_dir.join("codex-memory-projection.json");
            materialize_codex_home_config(
                codex_home.as_std_path(),
                Some(&spec.provider_profile),
                Some(&source_home),
                Some(&layout.project_root),
                Some(&spec.name),
                Some(runtime_dir),
                Some(workspace_path),
                None,
                Some(&event_path),
                Some(&marker_path),
            )?;
        }
        "droid" => {
            let target_home = layout
                .agent_provider_state_dir(&spec.name, "droid")
                .join("home");
            materialize_droid_home_config(
                target_home.as_std_path(),
                Some(spec.provider_profile.inherit_skills),
                Some(source_home.as_std_path()),
            );
        }
        "opencode" => {
            let config_path = layout
                .agent_provider_state_dir(&spec.name, "opencode")
                .join("opencode.json");
            let _ = materialize_opencode_memory_config(
                layout.project_root.as_std_path(),
                &spec.name,
                Some(workspace_path.as_std_path()),
                Some(config_path.as_std_path()),
                Some(resolved_profile),
                Some(layout.agent_events_path(&spec.name).as_std_path()),
                Some(
                    runtime_dir
                        .join("opencode-memory-projection.json")
                        .as_std_path(),
                ),
            );
        }
        "gemini" => {
            let home_root = resolve_gemini_home_root(layout, &spec.name);
            let event_path = layout.agent_events_path(&spec.name);
            let marker_path = runtime_dir.join("gemini-memory-projection.json");
            materialize_gemini_home_config(
                home_root.as_std_path(),
                Some(resolved_profile),
                Some(source_home.as_std_path()),
                Some(layout.project_root.as_std_path()),
                Some(&spec.name),
                Some(workspace_path.as_std_path()),
                Some(event_path.as_std_path()),
                Some(marker_path.as_std_path()),
            )?;
        }
        _ => {}
    }

    Ok(())
}

fn _record_claude_binary_cache_drift_if_present(
    layout: &PathLayout,
    spec: &AgentSpec,
    runtime_dir: &Utf8Path,
    home_root: &Utf8Path,
) -> anyhow::Result<()> {
    let versions_dir = home_root
        .join(".local")
        .join("share")
        .join("claude")
        .join("versions");

    if _claude_versions_dir_points_to_shared_cache(layout, &versions_dir) {
        return Ok(());
    }

    let Some(signature) = _claude_versions_cache_signature(&versions_dir) else {
        return Ok(());
    };

    let marker_path = runtime_dir.join("claude-binary-cache-drift.json");
    if _same_cached_signature(&marker_path, &signature) {
        return Ok(());
    }

    let version_names: Vec<String> = signature
        .get("version_names")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let event = serde_json::json!({
        "record_type": "agent_event",
        "event_type": "claude_binary_cache_drift",
        "provider": "claude",
        "agent_name": spec.name,
        "status": "notice",
        "reason": signature.get("reason").and_then(|v| v.as_str()).unwrap_or(""),
        "versions_dir": versions_dir.as_str(),
        "version_count": version_names.len(),
        "version_names": version_names,
        "created_at": chrono::Utc::now()
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            .replace("+00:00", "Z"),
    });

    let events_path = layout.agent_events_path(&spec.name);
    if let Some(parent) = events_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&events_path)?;
    writeln!(file, "{}", serde_json::to_string(&event)?)?;

    if let Some(parent) = marker_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    ccb_storage::atomic::atomic_write_json(&marker_path, &signature)?;

    Ok(())
}

fn _claude_versions_cache_signature(
    versions_dir: &Utf8Path,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    if versions_dir.is_symlink() {
        let mut signature = serde_json::Map::new();
        signature.insert("reason".into(), "versions_dir_symlink".into());
        signature.insert("versions_dir".into(), versions_dir.as_str().into());
        signature.insert("version_names".into(), serde_json::Value::Array(Vec::new()));
        return Some(signature);
    }

    if !versions_dir.is_dir() {
        return None;
    }

    let mut version_names = Vec::new();
    let entries = std::fs::read_dir(versions_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name.ends_with(".ccb-projection.json") {
            continue;
        }
        version_names.push(name.into_owned());
    }

    if version_names.is_empty() {
        return None;
    }

    version_names.sort();
    let mut signature = serde_json::Map::new();
    signature.insert("reason".into(), "per_agent_versions_cache_present".into());
    signature.insert("versions_dir".into(), versions_dir.as_str().into());
    signature.insert(
        "version_names".into(),
        serde_json::Value::Array(version_names.into_iter().map(Into::into).collect()),
    );
    Some(signature)
}

fn _same_cached_signature(
    marker_path: &Utf8Path,
    signature: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    let text = match std::fs::read_to_string(marker_path) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let existing: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return false,
    };
    existing == serde_json::Value::Object(signature.clone())
}

fn _claude_versions_dir_points_to_shared_cache(
    layout: &PathLayout,
    versions_dir: &Utf8Path,
) -> bool {
    if !versions_dir.is_symlink() {
        return false;
    }
    let Ok(resolved) = std::fs::canonicalize(versions_dir.as_std_path()) else {
        return false;
    };
    let Ok(external) = layout.provider_external_cache_dir("claude") else {
        return false;
    };
    let Ok(shared) = layout.provider_shared_cache_dir("claude") else {
        return false;
    };
    let Ok(external_resolved) = std::fs::canonicalize(external.as_std_path()) else {
        return false;
    };
    let Ok(shared_resolved) = std::fs::canonicalize(shared.as_std_path()) else {
        return false;
    };
    resolved == external_resolved || resolved == shared_resolved
}

/// Locate a native provider hook binary.
///
/// Search order:
/// 1. `CCB_HOOK_BIN_DIR` environment variable.
/// 2. Directory containing the currently running executable.
/// 3. `PATH` environment variable.
fn provider_hook_binary_path(name: &str) -> Option<Utf8PathBuf> {
    if let Ok(dir) = std::env::var("CCB_HOOK_BIN_DIR") {
        let candidate = Utf8PathBuf::from(dir).join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Utf8PathBuf::from_path_buf(candidate).ok();
            }
        }
    }

    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Utf8PathBuf::from_path_buf(candidate).ok();
            }
        }
    }

    None
}
