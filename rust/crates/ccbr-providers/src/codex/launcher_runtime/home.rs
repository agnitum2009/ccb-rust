//! Codex launcher home layout and override helpers.
//!
//! Mirrors Python `lib/provider_backends/codex/launcher_runtime/command_runtime/home.py`.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use camino::{Utf8Path, Utf8PathBuf};
use ccbr_provider_profiles::codex_home_config::{
    materialize_codex_home_config, repair_codex_activity_hooks,
    resolve_codex_home_layout as resolve_codex_home_layout_inner,
};
use ccbr_provider_profiles::models::{ProviderProfileSpec, ResolvedProviderProfile};

pub use ccbr_provider_profiles::codex_home_config::CodexHomeLayout;

const SESSION_NAMESPACE_MARKER: &str = ".ccbr-session-namespace.json";

/// Resolve the isolated Codex home layout for a runtime directory.
///
/// Mirrors Python `resolve_codex_home_layout` but uses the provider-state
/// mapping (`repo/.ccbr/agents/<agent>/provider-state/codex`) instead of the
/// runtime directory itself, matching the Python `state_dir_for_runtime_dir`
/// convention.
pub fn resolve_codex_home_layout(
    runtime_dir: &Utf8Path,
    profile: Option<&ResolvedProviderProfile>,
) -> CodexHomeLayout {
    if let Some(home) = profile_runtime_home(profile) {
        return CodexHomeLayout {
            codex_home: home.clone(),
            session_root: home.join("sessions"),
        };
    }

    let state_dir =
        state_dir_for_runtime_dir(runtime_dir).unwrap_or_else(|| runtime_dir.to_path_buf());

    if let Some(layout) = existing_layout_from_session_file(runtime_dir) {
        return layout;
    }

    resolve_codex_home_layout_inner(&state_dir, profile)
}

fn profile_runtime_home(profile: Option<&ResolvedProviderProfile>) -> Option<Utf8PathBuf> {
    let home = profile?.runtime_home.as_deref()?;
    let home = home.trim();
    if home.is_empty() {
        return None;
    }
    Some(Utf8PathBuf::from(home))
}

fn existing_layout_from_session_file(runtime_dir: &Utf8Path) -> Option<CodexHomeLayout> {
    let session_file = super::session_paths::session_file_for_runtime_dir(runtime_dir)?;
    let text = std::fs::read_to_string(&session_file).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let data = value.as_object()?;

    // Explicit fields take precedence and are returned without migration.
    if let Some(layout) = explicit_layout_from_fields(data) {
        return Some(layout);
    }

    // Fall back to env assignments in the persisted start command.
    if let Some(layout) = layout_from_persisted_command(data) {
        return Some(layout);
    }

    // Fall back to the persisted session log path, normalizing a legacy layout.
    layout_from_session_log_path(data)
}

fn explicit_layout_from_fields(
    data: &serde_json::Map<String, serde_json::Value>,
) -> Option<CodexHomeLayout> {
    let codex_home = data
        .get("codex_home")
        .and_then(|v| v.as_str())
        .map(Utf8PathBuf::from)?;
    let session_root = data
        .get("codex_session_root")
        .and_then(|v| v.as_str())
        .map(Utf8PathBuf::from)
        .unwrap_or_else(|| codex_home.join("sessions"));
    Some(CodexHomeLayout {
        codex_home,
        session_root,
    })
}

fn layout_from_persisted_command(
    data: &serde_json::Map<String, serde_json::Value>,
) -> Option<CodexHomeLayout> {
    for key in ["codex_start_cmd", "start_cmd"] {
        let cmd = data.get(key).and_then(|v| v.as_str())?;
        let session_root = extract_env_path(cmd, "CODEX_SESSION_ROOT");
        let codex_home = extract_env_path(cmd, "CODEX_HOME");
        if session_root.is_none() && codex_home.is_none() {
            continue;
        }
        let session_root =
            session_root.unwrap_or_else(|| codex_home.as_ref().unwrap().join("sessions"));
        let codex_home = codex_home.unwrap_or_else(|| {
            if session_root.as_str().ends_with("/sessions") {
                let parent = session_root.parent().unwrap_or(&session_root);
                Utf8PathBuf::from(parent)
            } else {
                session_root.join("home")
            }
        });
        return Some(CodexHomeLayout {
            codex_home,
            session_root,
        });
    }
    None
}

fn layout_from_session_log_path(
    data: &serde_json::Map<String, serde_json::Value>,
) -> Option<CodexHomeLayout> {
    let log_path = data
        .get("codex_session_path")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)?;
    let session_root = session_root_from_log_path(&log_path)?;
    let codex_home = if session_root.as_str().ends_with("/sessions") {
        let parent = session_root.parent().unwrap_or(&session_root);
        Utf8PathBuf::from(parent)
    } else {
        session_root.join("home")
    };

    // Normalize to provider-state layout and migrate a legacy root if needed.
    if let Some((target_home, target_root)) = normalize_legacy_layout(&codex_home, &session_root) {
        migrate_legacy_session_root(&session_root, &target_root);
        Some(CodexHomeLayout {
            codex_home: target_home,
            session_root: target_root,
        })
    } else {
        Some(CodexHomeLayout {
            codex_home,
            session_root,
        })
    }
}

fn extract_env_path(command: &str, name: &str) -> Option<Utf8PathBuf> {
    let pattern = format!(
        r#"(?:^|[\s;])(?:export\s+)?{}=('[^']*'|"[^"]*"|[^\s;]+)"#,
        regex::escape(name)
    );
    let re = regex::Regex::new(&pattern).ok()?;
    let value = re.captures(command)?.get(1)?.as_str();
    let value = value
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .unwrap_or(value);
    let value = value
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(value);
    if value.is_empty() {
        return None;
    }
    Some(Utf8PathBuf::from(value))
}

fn session_root_from_log_path(log_path: &std::path::Path) -> Option<Utf8PathBuf> {
    let mut current = Some(log_path);
    while let Some(p) = current {
        if p.file_name().map(|n| n == "sessions").unwrap_or(false) {
            return Utf8PathBuf::from_path_buf(p.to_path_buf()).ok();
        }
        current = p.parent();
    }
    None
}

fn normalize_legacy_layout(
    codex_home: &Utf8Path,
    session_root: &Utf8Path,
) -> Option<(Utf8PathBuf, Utf8PathBuf)> {
    if session_root.as_str().ends_with("/sessions") {
        let parent = session_root.parent()?;
        if parent.file_name() != Some("home") {
            let target_home = parent.join("home");
            let target_root = target_home.join("sessions");
            if target_home != *codex_home || target_root != *session_root {
                return Some((target_home, target_root));
            }
        }
    }
    None
}

fn migrate_legacy_session_root(source_root: &Utf8Path, target_root: &Utf8Path) {
    if source_root == target_root || !source_root.is_dir() {
        return;
    }
    let _ = fs::create_dir_all(target_root);
    let _ = move_dir_contents(source_root, target_root);
}

fn move_dir_contents(source: &Utf8Path, target: &Utf8Path) -> std::io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = target.as_std_path().join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&to)?;
            move_dir_contents(
                &Utf8PathBuf::from_path_buf(from.clone()).unwrap_or_else(|_| target.to_path_buf()),
                &Utf8PathBuf::from_path_buf(to.clone()).unwrap_or_else(|_| target.to_path_buf()),
            )?;
            fs::remove_dir(&from)?;
        } else {
            fs::rename(&from, &to)?;
        }
    }
    Ok(())
}

/// Prepare the `CODEX_HOME` / `CODEX_SESSION_ROOT` overrides and ensure the
/// managed home exists.
#[allow(clippy::too_many_arguments)]
pub fn prepare_codex_home_overrides(
    runtime_dir: &Utf8Path,
    profile: Option<&ResolvedProviderProfile>,
    refresh_home: bool,
    project_root: Option<&Utf8Path>,
    agent_name: Option<&str>,
    workspace_path: Option<&Utf8Path>,
    memory_projection_event_path: Option<&Utf8Path>,
    memory_projection_marker_path: Option<&Utf8Path>,
) -> anyhow::Result<HashMap<String, String>> {
    let layout = resolve_codex_home_layout(runtime_dir, profile);

    fs::create_dir_all(&layout.codex_home)?;
    fs::create_dir_all(&layout.session_root)?;

    let marker_ready = session_namespace_marker_exists(&layout.codex_home);
    if refresh_home {
        let source_home = system_codex_home();
        let profile_spec = profile.map(resolved_profile_to_spec).unwrap_or_default();
        materialize_codex_home_config(
            layout.codex_home.as_std_path(),
            Some(&profile_spec),
            Some(&source_home),
            project_root,
            agent_name,
            Some(runtime_dir),
            workspace_path,
            None,
            memory_projection_event_path,
            memory_projection_marker_path,
        )?;
        ensure_session_namespace_marker(&layout.codex_home)?;
    } else if !marker_ready && is_dir_empty(&layout.session_root)? {
        write_session_namespace_marker(&layout.codex_home, "");
    }

    if !refresh_home {
        repair_codex_activity_hooks(
            layout.codex_home.as_std_path(),
            project_root,
            agent_name,
            Some(runtime_dir),
            workspace_path,
        )?;
    }

    let mut overrides = HashMap::new();
    overrides.insert(
        "CODEX_HOME".to_string(),
        layout
            .codex_home
            .as_std_path()
            .to_string_lossy()
            .to_string(),
    );
    overrides.insert(
        "CODEX_SESSION_ROOT".to_string(),
        layout
            .session_root
            .as_std_path()
            .to_string_lossy()
            .to_string(),
    );

    if std::env::var("WSL_DISTRO_NAME").is_ok() {
        overrides.insert(
            "USERPROFILE".to_string(),
            layout
                .codex_home
                .as_std_path()
                .to_string_lossy()
                .to_string(),
        );
        let wslenv_additions = "CODEX_HOME/p:CODEX_SESSION_ROOT/p:USERPROFILE/p";
        let existing = std::env::var("WSLENV").unwrap_or_default();
        overrides.insert(
            "WSLENV".to_string(),
            if existing.is_empty() {
                wslenv_additions.to_string()
            } else {
                format!("{}:{}", wslenv_additions, existing)
            },
        );
    }

    Ok(overrides)
}

/// Map a `provider-runtime/<provider>` directory to the corresponding
/// `provider-state/<provider>` directory.
///
/// Mirrors Python `lib/provider_backends/codex/launcher_runtime/session_paths.state_dir_for_runtime_dir`.
pub fn state_dir_for_runtime_dir(runtime_dir: &Utf8Path) -> Option<Utf8PathBuf> {
    let s = runtime_dir.as_str();
    let needle = "/provider-runtime/";
    if let Some(pos) = s.find(needle) {
        let prefix = &s[..=pos];
        let suffix = &s[pos + needle.len() - 1..];
        return Some(Utf8PathBuf::from(format!(
            "{}provider-state{}",
            prefix, suffix
        )));
    }
    if let Some(prefix) = s.strip_suffix("/provider-runtime") {
        return Some(Utf8PathBuf::from(format!("{}/provider-state", prefix)));
    }
    None
}

fn system_codex_home() -> Utf8PathBuf {
    std::env::var("CODEX_HOME")
        .map(Utf8PathBuf::from)
        .unwrap_or_else(|_| {
            Utf8PathBuf::from_path_buf(
                std::env::var("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("/tmp"))
                    .join(".codex"),
            )
            .unwrap_or_else(|_| Utf8PathBuf::from("/tmp/.codex"))
        })
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

fn session_namespace_marker_exists(codex_home: &Utf8Path) -> bool {
    codex_home.join(SESSION_NAMESPACE_MARKER).is_file()
}

fn ensure_session_namespace_marker(codex_home: &Utf8Path) -> std::io::Result<()> {
    write_session_namespace_marker(codex_home, "");
    Ok(())
}

fn write_session_namespace_marker(codex_home: &Utf8Path, fingerprint: &str) {
    let marker_path = codex_home.join(SESSION_NAMESPACE_MARKER);
    let _ = fs::create_dir_all(marker_path.parent().unwrap_or(codex_home));
    let payload = serde_json::json!({
        "provider": "codex",
        "provider_authority_fingerprint": fingerprint,
        "memory_projection_sha256": "",
        "updated_at": "",
        "version": 1,
    });
    let _ = fs::write(
        &marker_path,
        serde_json::to_string_pretty(&payload).unwrap_or_default(),
    );
}

fn is_dir_empty(path: &Utf8Path) -> std::io::Result<bool> {
    match fs::read_dir(path) {
        Ok(mut entries) => Ok(entries.next().is_none()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_dir_for_runtime_dir() {
        let runtime = Utf8PathBuf::from("/repo/.ccbr/agents/agent1/provider-runtime/codex");
        assert_eq!(
            state_dir_for_runtime_dir(&runtime),
            Some(Utf8PathBuf::from(
                "/repo/.ccbr/agents/agent1/provider-state/codex"
            ))
        );
    }

    #[test]
    fn test_state_dir_for_runtime_dir_root() {
        let runtime = Utf8PathBuf::from("/repo/.ccbr/agents/agent1/provider-runtime");
        assert_eq!(
            state_dir_for_runtime_dir(&runtime),
            Some(Utf8PathBuf::from(
                "/repo/.ccbr/agents/agent1/provider-state"
            ))
        );
    }
}
