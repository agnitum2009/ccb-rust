use std::collections::{HashMap, HashSet};
use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use serde_json::json;

use crate::codex_home_config::materialize_codex_home_config;
use crate::models::{normalize_path_text, ProviderProfileSpec, ResolvedProviderProfile};

const CODEX_KEYS: &[&str] = &[
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "OPENAI_API_BASE",
    "OPENAI_ORG_ID",
    "OPENAI_ORGANIZATION",
];
const CLAUDE_KEYS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
];
const GEMINI_KEYS: &[&str] = &[
    "GEMINI_API_KEY",
    "GEMINI_MODEL",
    "GOOGLE_API_KEY",
    "GOOGLE_API_BASE",
    "GOOGLE_GEMINI_BASE_URL",
    "GOOGLE_VERTEX_BASE_URL",
    "GOOGLE_GENAI_USE_VERTEXAI",
    "GOOGLE_GENAI_USE_GCA",
    "GOOGLE_CLOUD_PROJECT",
    "GOOGLE_CLOUD_LOCATION",
    "GOOGLE_APPLICATION_CREDENTIALS",
];

const CODEX_RUNTIME_HOME_SENTINELS: &[&str] = &[
    "sessions",
    "archived-sessions",
    "auth.json",
    "history.jsonl",
    "logs_2.sqlite",
    "state_5.sqlite",
    ".ccbr-session-namespace.json",
    "log",
    "logs",
    "shell_snapshots",
    ".tmp/plugins",
    ".tmp/plugins.sha",
];

const CODEX_SESSION_MIGRATION_SENTINELS: &[&str] = &[
    "sessions",
    "archived-sessions",
    "auth.json",
    "history.jsonl",
    "logs_2.sqlite",
    "state_5.sqlite",
    ".ccbr-session-namespace.json",
    "log",
    "logs",
    "shell_snapshots",
    ".tmp/plugins",
    ".tmp/plugins.sha",
];

/// Materialize a provider profile and write a record to the runtime directory.
pub fn materialize_provider_profile(
    layout: &ccb_storage::paths::PathLayout,
    name: &str,
    provider: &str,
    profile_spec: &ProviderProfileSpec,
    workspace_path: &Utf8Path,
) -> crate::Result<ResolvedProviderProfile> {
    materialize_provider_profile_with_source(
        layout,
        name,
        provider,
        profile_spec,
        workspace_path,
        None,
    )
}

/// Materialize a provider profile with an explicit Codex source home.
pub fn materialize_provider_profile_with_source(
    layout: &ccb_storage::paths::PathLayout,
    name: &str,
    provider: &str,
    profile_spec: &ProviderProfileSpec,
    workspace_path: &Utf8Path,
    codex_source_home: Option<&Utf8Path>,
) -> crate::Result<ResolvedProviderProfile> {
    let normalized_provider = provider.trim().to_lowercase();
    let normalized_name = name.trim().to_lowercase();
    if normalized_name.is_empty() {
        return Err(crate::ProfilesError::Validation(
            "agent name cannot be empty".into(),
        ));
    }
    if normalized_provider.is_empty() {
        return Err(crate::ProfilesError::Validation(
            "provider cannot be empty".into(),
        ));
    }
    validate_provider_runtime_home_policy(provider, profile_spec)?;

    let runtime_dir = layout.agent_provider_runtime_dir(&normalized_name, &normalized_provider);
    fs::create_dir_all(&runtime_dir)?;

    let profile_root =
        resolve_profile_root(layout, &normalized_name, &normalized_provider, profile_spec)?;

    let profile = if normalized_provider == "codex" {
        materialize_codex_profile(
            layout,
            &normalized_name,
            &normalized_provider,
            profile_spec,
            &profile_root,
            workspace_path,
            codex_source_home,
        )?
    } else if normalized_provider == "claude" {
        materialize_claude_profile(
            &normalized_name,
            &normalized_provider,
            profile_spec,
            &profile_root,
        )
    } else if normalized_provider == "gemini" {
        materialize_api_profile(
            &normalized_name,
            &normalized_provider,
            profile_spec,
            &profile_root,
        )
    } else {
        ResolvedProviderProfile {
            provider: normalized_provider,
            agent_name: normalized_name,
            mode: profile_spec.mode.clone(),
            profile_root: Some(profile_root.to_string()),
            runtime_home: None,
            env: profile_spec.env.clone(),
            inherit_api: profile_spec.inherit_api,
            inherit_auth: profile_spec.inherit_auth,
            inherit_config: profile_spec.inherit_config,
            inherit_skills: profile_spec.inherit_skills,
            inherit_commands: profile_spec.inherit_commands,
            inherit_memory: profile_spec.inherit_memory,
        }
    };

    write_profile_record(&runtime_dir, &profile)?;
    Ok(profile)
}

/// Load a previously materialized provider profile record.
pub fn load_resolved_provider_profile(runtime_dir: &Utf8Path) -> Option<ResolvedProviderProfile> {
    let path = runtime_dir.join("provider-profile.json");
    if !path.is_file() {
        return None;
    }
    let text = fs::read_to_string(&path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let obj = value.as_object()?;
    ResolvedProviderProfile::from_record(obj).ok()
}

/// Return the set of recognized API env keys for a provider.
pub fn provider_api_env_keys(provider: &str) -> HashSet<String> {
    let normalized = provider.trim().to_lowercase();
    let keys: &[&str] = match normalized.as_str() {
        "codex" => CODEX_KEYS,
        "claude" => CLAUDE_KEYS,
        "gemini" => GEMINI_KEYS,
        _ => &[],
    };
    keys.iter().map(|k| (*k).into()).collect()
}

/// Validate that `home` is only set for codex profiles.
pub fn validate_provider_runtime_home_policy(
    provider: &str,
    profile_spec: &ProviderProfileSpec,
) -> crate::Result<()> {
    let normalized = provider.trim().to_lowercase();
    if normalized == "codex" || profile_spec.home.is_none() {
        return Ok(());
    }
    Err(crate::ProfilesError::Validation(
        "provider_profile.home is supported only for codex runtime_home overrides".into(),
    ))
}

/// Validate that effective runtime homes are unique across specs.
pub fn validate_provider_runtime_home_uniqueness<'a>(
    layout: &ccb_storage::paths::PathLayout,
    specs: impl Iterator<Item = (&'a str, &'a str, &'a ProviderProfileSpec)>,
) -> crate::Result<()> {
    let mut seen: HashMap<(String, String), String> = HashMap::new();
    for (name, provider, profile_spec) in specs {
        validate_provider_runtime_home_policy(provider, profile_spec)?;
        let home = effective_provider_runtime_home(layout, name, provider, profile_spec)?;
        let key = (
            provider.trim().to_lowercase(),
            normalize_runtime_home(&home),
        );
        if let Some(prior) = seen.get(&key) {
            return Err(crate::ProfilesError::Validation(format!(
                "duplicate effective {}_home for agents {} and {}: {}",
                key.0, prior, name, key.1
            )));
        }
        seen.insert(key, name.to_string());
    }
    Ok(())
}

fn effective_provider_runtime_home(
    layout: &ccb_storage::paths::PathLayout,
    name: &str,
    provider: &str,
    profile_spec: &ProviderProfileSpec,
) -> crate::Result<Utf8PathBuf> {
    let normalized = provider.trim().to_lowercase();
    if normalized == "codex" && codex_profile_uses_explicit_runtime_home(profile_spec) {
        return resolve_profile_root(layout, name, provider, profile_spec);
    }
    Ok(layout.agent_provider_state_dir(name, provider).join("home"))
}

fn normalize_runtime_home(path: &Utf8Path) -> String {
    let expanded = Utf8PathBuf::from(expand_tilde(path.as_str()));
    expanded
        .canonicalize_utf8()
        .unwrap_or(expanded.clone())
        .to_string()
}

fn materialize_codex_profile(
    layout: &ccb_storage::paths::PathLayout,
    name: &str,
    provider: &str,
    profile_spec: &ProviderProfileSpec,
    profile_root: &Utf8Path,
    workspace_path: &Utf8Path,
    codex_source_home: Option<&Utf8Path>,
) -> crate::Result<ResolvedProviderProfile> {
    let runtime_home = effective_provider_runtime_home(layout, name, provider, profile_spec)?;
    let uses_explicit_home = codex_profile_uses_explicit_runtime_home(profile_spec);

    if !uses_explicit_home {
        let migrated = migrate_legacy_codex_profile_runtime_home(
            layout,
            name,
            provider,
            profile_root,
            &runtime_home,
        )?;
        if migrated {
            discard_migrated_codex_projection(&runtime_home)?;
        }
    }

    materialize_codex_home_config(
        runtime_home.as_std_path(),
        Some(profile_spec),
        codex_source_home,
        Some(&layout.project_root),
        Some(name),
        Some(&layout.agent_provider_runtime_dir(name, provider)),
        Some(workspace_path),
        Some(&layout.shared_cache_dir()),
        None,
        None,
    )?;

    Ok(ResolvedProviderProfile {
        provider: provider.into(),
        agent_name: name.into(),
        mode: profile_spec.mode.clone(),
        profile_root: if uses_explicit_home {
            Some(profile_root.to_string())
        } else {
            None
        },
        runtime_home: Some(runtime_home.to_string()),
        env: profile_spec.env.clone(),
        inherit_api: profile_spec.inherit_api,
        inherit_auth: profile_spec.inherit_auth,
        inherit_config: profile_spec.inherit_config,
        inherit_skills: profile_spec.inherit_skills,
        inherit_commands: profile_spec.inherit_commands,
        inherit_memory: profile_spec.inherit_memory,
    })
}

fn materialize_api_profile(
    name: &str,
    provider: &str,
    profile_spec: &ProviderProfileSpec,
    profile_root: &Utf8Path,
) -> ResolvedProviderProfile {
    let api_keys = provider_api_env_keys(provider);
    let env: HashMap<String, String> = profile_spec
        .env
        .iter()
        .filter(|(k, _)| api_keys.contains(*k) || profile_spec.mode != "inherit")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    ResolvedProviderProfile {
        provider: provider.into(),
        agent_name: name.into(),
        mode: profile_spec.mode.clone(),
        profile_root: Some(profile_root.to_string()),
        runtime_home: None,
        env,
        inherit_api: profile_spec.inherit_api,
        inherit_auth: profile_spec.inherit_auth,
        inherit_config: profile_spec.inherit_config,
        inherit_skills: profile_spec.inherit_skills,
        inherit_commands: profile_spec.inherit_commands,
        inherit_memory: profile_spec.inherit_memory,
    }
}

fn materialize_claude_profile(
    name: &str,
    provider: &str,
    profile_spec: &ProviderProfileSpec,
    profile_root: &Utf8Path,
) -> ResolvedProviderProfile {
    materialize_api_profile(name, provider, profile_spec, profile_root)
}

fn resolve_profile_root(
    layout: &ccb_storage::paths::PathLayout,
    name: &str,
    provider: &str,
    profile_spec: &ProviderProfileSpec,
) -> crate::Result<Utf8PathBuf> {
    if let Some(home) = &profile_spec.home {
        let raw = Utf8PathBuf::from(expand_tilde(home.trim()));
        let resolved = if raw.is_absolute() {
            raw.canonicalize_utf8().unwrap_or(raw)
        } else {
            let joined = layout.project_root.join(&raw);
            joined
                .canonicalize_utf8()
                .unwrap_or_else(|_| layout.project_root.join(raw))
        };
        return Ok(resolved);
    }
    Ok(layout
        .provider_profiles_dir()
        .join(name)
        .join(provider)
        .canonicalize_utf8()
        .unwrap_or_else(|_| layout.provider_profiles_dir().join(name).join(provider)))
}

fn codex_profile_uses_explicit_runtime_home(profile_spec: &ProviderProfileSpec) -> bool {
    profile_spec.home.is_some()
}

fn discard_migrated_codex_projection(runtime_home: &Utf8Path) -> crate::Result<()> {
    let tmp_plugins = runtime_home.join(".tmp/plugins");
    if tmp_plugins.is_symlink() || tmp_plugins.is_file() {
        let _ = fs::remove_file(&tmp_plugins);
    } else if tmp_plugins.is_dir() {
        let _ = fs::remove_dir_all(&tmp_plugins);
    }
    let _ = fs::remove_file(runtime_home.join(".tmp/plugins.sha"));
    let _ = fs::remove_file(runtime_home.join("AGENTS.md"));
    Ok(())
}

fn migrate_legacy_codex_profile_runtime_home(
    layout: &ccb_storage::paths::PathLayout,
    name: &str,
    provider: &str,
    source_home: &Utf8Path,
    target_home: &Utf8Path,
) -> crate::Result<bool> {
    if same_path(source_home, target_home) || !looks_like_legacy_codex_runtime_home(source_home) {
        return Ok(false);
    }
    if source_home.is_symlink() || !is_within(source_home, &layout.provider_profiles_dir()) {
        record_codex_profile_migration_event(
            layout,
            name,
            provider,
            "skipped",
            "legacy_home_out_of_bounds_or_symlink",
            source_home,
            target_home,
        )?;
        return Ok(false);
    }
    if !is_within(
        target_home,
        &layout.agent_provider_state_dir(name, provider),
    ) {
        record_codex_profile_migration_event(
            layout,
            name,
            provider,
            "skipped",
            "target_home_out_of_bounds",
            source_home,
            target_home,
        )?;
        return Ok(false);
    }
    if agent_runtime_blocks_legacy_migration(layout, name) {
        record_codex_profile_migration_event(
            layout,
            name,
            provider,
            "skipped",
            "agent_runtime_active",
            source_home,
            target_home,
        )?;
        return Ok(false);
    }
    if session_migration_material_contains_symlink(source_home) {
        record_codex_profile_migration_event(
            layout,
            name,
            provider,
            "skipped",
            "legacy_home_contains_symlink",
            source_home,
            target_home,
        )?;
        return Ok(false);
    }
    let prepared = prepare_legacy_codex_session_authority(layout, name, source_home, target_home)?;
    if prepared == MigrationResult::Abort {
        record_codex_profile_migration_event(
            layout,
            name,
            provider,
            "skipped",
            "session_authority_preflight_failed",
            source_home,
            target_home,
        )?;
        return Ok(false);
    }

    fs::create_dir_all(target_home)?;
    merge_legacy_codex_session_material(source_home, target_home)?;
    if let MigrationResult::Session(file, payload) = prepared {
        ccb_storage::atomic::atomic_write_json(&file, &payload)?;
    }
    remove_empty_parents(source_home, &layout.provider_profiles_dir())?;
    record_codex_profile_migration_event(
        layout,
        name,
        provider,
        "migrated",
        "legacy_profile_runtime_home_migrated",
        source_home,
        target_home,
    )?;
    Ok(true)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MigrationResult {
    None,
    Abort,
    Session(Utf8PathBuf, serde_json::Value),
}

fn looks_like_legacy_codex_runtime_home(path: &Utf8Path) -> bool {
    if !path.exists() || !path.is_dir() || path.is_symlink() {
        return false;
    }
    CODEX_RUNTIME_HOME_SENTINELS
        .iter()
        .any(|relative| path.join(relative).exists() || path.join(relative).is_symlink())
}

fn merge_legacy_codex_session_material(source: &Utf8Path, target: &Utf8Path) -> crate::Result<()> {
    fs::create_dir_all(target)?;
    for relative in CODEX_SESSION_MIGRATION_SENTINELS {
        let child = source.join(relative);
        if !child.exists() || child.is_symlink() {
            continue;
        }
        let destination = target.join(relative);
        if child.is_dir() {
            merge_tree(&child, &destination)?;
        } else {
            fs::create_dir_all(destination.parent().unwrap_or(destination.as_ref()))?;
            fs::rename(&child, &destination)?;
        }
    }
    remove_empty_parents(source, source)?;
    Ok(())
}

fn merge_tree(source: &Utf8Path, target: &Utf8Path) -> crate::Result<()> {
    if !source.is_dir() || source.is_symlink() {
        return Ok(());
    }
    fs::create_dir_all(target)?;
    let mut entries: Vec<_> = fs::read_dir(source)?
        .filter_map(|e| e.ok())
        .filter_map(|e| Utf8PathBuf::from_path_buf(e.path()).ok())
        .collect();
    entries.sort();
    for child in entries {
        let destination = target.join(child.file_name().unwrap_or(""));
        if child.is_symlink() {
            continue;
        }
        if !destination.exists() && !destination.is_symlink() {
            fs::create_dir_all(destination.parent().unwrap_or(destination.as_ref()))?;
            fs::rename(&child, &destination)?;
        } else if child.is_dir() && destination.is_dir() && !destination.is_symlink() {
            merge_tree(&child, &destination)?;
        }
    }
    let _ = fs::remove_dir(source);
    Ok(())
}

fn prepare_legacy_codex_session_authority(
    layout: &ccb_storage::paths::PathLayout,
    name: &str,
    source_home: &Utf8Path,
    target_home: &Utf8Path,
) -> crate::Result<MigrationResult> {
    if !has_legacy_codex_session_material(source_home) {
        return Ok(MigrationResult::None);
    }
    let session_file = layout.ccb_dir().join(
        ccb_provider_core::pathing::session_filename_for_agent("codex", name)
            .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?,
    );
    if !session_file.is_file() {
        return Ok(MigrationResult::Abort);
    }
    let text = fs::read_to_string(&session_file)?;
    let mut payload: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Ok(MigrationResult::Abort),
    };
    let obj = match payload.as_object_mut() {
        Some(o) => o,
        None => return Ok(MigrationResult::Abort),
    };
    let source_sessions = source_home.join("sessions");
    let target_sessions = target_home.join("sessions");
    let mut changed = false;
    changed |= rewrite_path_field(obj, "codex_home", source_home, target_home);
    changed |= rewrite_path_field(
        obj,
        "codex_session_root",
        &source_sessions,
        &target_sessions,
    );
    changed |= rewrite_nested_path_field(
        obj,
        "codex_session_path",
        &source_sessions,
        &target_sessions,
    );
    for key in ["start_cmd", "codex_start_cmd"] {
        changed |= rewrite_command_field(obj, key, source_home, target_home);
    }
    if changed {
        Ok(MigrationResult::Session(session_file, payload))
    } else {
        Ok(MigrationResult::Abort)
    }
}

fn has_legacy_codex_session_material(source_home: &Utf8Path) -> bool {
    CODEX_SESSION_MIGRATION_SENTINELS
        .iter()
        .any(|relative| source_home.join(relative).exists())
}

fn session_migration_material_contains_symlink(source_home: &Utf8Path) -> bool {
    for relative in CODEX_SESSION_MIGRATION_SENTINELS {
        let root = source_home.join(relative);
        if !root.exists() && !root.is_symlink() {
            continue;
        }
        if tree_contains_symlink(&root) {
            return true;
        }
    }
    false
}

fn tree_contains_symlink(root: &Utf8Path) -> bool {
    if root.is_symlink() {
        return true;
    }
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return true,
    };
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => return true,
        };
        let path = match Utf8PathBuf::from_path_buf(entry.path()) {
            Ok(p) => p,
            Err(_) => return true,
        };
        if tree_contains_symlink(&path) {
            return true;
        }
    }
    false
}

fn agent_runtime_blocks_legacy_migration(
    layout: &ccb_storage::paths::PathLayout,
    name: &str,
) -> bool {
    let runtime_path = layout.agent_runtime_path(name);
    if !runtime_path.is_file() {
        return false;
    }
    let text = match fs::read_to_string(&runtime_path) {
        Ok(t) => t,
        Err(_) => return true,
    };
    let payload: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return true,
    };
    let obj = match payload.as_object() {
        Some(o) => o,
        None => return true,
    };
    let state = obj
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if state.is_empty() || state == "stopped" || state == "failed" {
        return false;
    }
    if ["pid", "runtime_pid"]
        .iter()
        .filter_map(|k| obj.get(*k).and_then(|v| v.as_i64()))
        .any(pid_alive)
    {
        return true;
    }
    matches!(state.as_str(), "starting" | "busy" | "stopping")
}

fn pid_alive(pid: i64) -> bool {
    if pid <= 0 {
        return false;
    }
    #[cfg(unix)]
    {
        use std::process::Command;
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn record_codex_profile_migration_event(
    layout: &ccb_storage::paths::PathLayout,
    name: &str,
    provider: &str,
    status: &str,
    reason: &str,
    source_home: &Utf8Path,
    target_home: &Utf8Path,
) -> crate::Result<()> {
    let path = layout.agent_events_path(name);
    fs::create_dir_all(path.parent().unwrap_or(path.as_ref()))?;
    let payload = json!({
        "record_type": "agent_event",
        "event_type": "codex_profile_migration",
        "provider": provider,
        "agent_name": name,
        "status": status,
        "reason": reason,
        "source_home": source_home.to_string(),
        "target_home": target_home.to_string(),
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true).replace("+00:00", "Z"),
    });
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    use std::io::Write;
    writeln!(file, "{}", serde_json::to_string(&payload)?)?;
    Ok(())
}

fn rewrite_path_field(
    payload: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    source: &Utf8Path,
    target: &Utf8Path,
) -> bool {
    let current = match payload.get(key).and_then(|v| v.as_str()) {
        Some(s) => match normalize_path_text(Some(s)) {
            Some(p) => Utf8PathBuf::from(p),
            None => return false,
        },
        None => return false,
    };
    if !same_path(&current, source) {
        return false;
    }
    payload.insert(key.into(), json!(target.to_string()));
    true
}

fn rewrite_nested_path_field(
    payload: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    source: &Utf8Path,
    target: &Utf8Path,
) -> bool {
    let current = match payload.get(key).and_then(|v| v.as_str()) {
        Some(s) => match normalize_path_text(Some(s)) {
            Some(p) => Utf8PathBuf::from(p),
            None => return false,
        },
        None => return false,
    };
    match replace_path_prefix(&current, source, target) {
        Some(replacement) => {
            payload.insert(key.into(), json!(replacement.to_string()));
            true
        }
        None => false,
    }
}

fn rewrite_command_field(
    payload: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    source_home: &Utf8Path,
    target_home: &Utf8Path,
) -> bool {
    let current = match payload.get(key).and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return false,
    };
    let source_sessions = source_home.join("sessions");
    let target_sessions = target_home.join("sessions");
    let mut updated = replace_path_text(&current, &source_sessions, &target_sessions);
    updated = replace_path_text(&updated, source_home, target_home);
    if updated == current {
        return false;
    }
    payload.insert(key.into(), json!(updated));
    true
}

fn replace_path_text(text: &str, source: &Utf8Path, target: &Utf8Path) -> String {
    let source_text = source.as_str();
    if source_text.is_empty() {
        return text.to_string();
    }
    let target_text = target.as_str();
    let mut result = String::new();
    let mut index = 0;
    while let Some(pos) = text[index..].find(source_text) {
        let match_start = index + pos;
        let match_end = match_start + source_text.len();
        if path_text_match_has_boundary(text, match_start, match_end) {
            result.push_str(&text[index..match_start]);
            result.push_str(target_text);
            index = match_end;
        } else {
            result.push_str(&text[index..match_end]);
            index = match_end;
        }
    }
    result.push_str(&text[index..]);
    result
}

fn path_text_match_has_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text.chars().nth(start.saturating_sub(1)).unwrap_or('\0');
    let after = text.chars().nth(end).unwrap_or('\0');
    path_text_left_boundary(before) && path_text_right_boundary(after)
}

fn path_text_left_boundary(ch: char) -> bool {
    ch == '\0' || ch.is_whitespace() || "= :;,'\"([{".contains(ch)
}

fn path_text_right_boundary(ch: char) -> bool {
    ch == '\0' || ch.is_whitespace() || ch == '/' || " :;,'\")]}".contains(ch)
}

fn replace_path_prefix(
    path: &Utf8Path,
    source: &Utf8Path,
    target: &Utf8Path,
) -> Option<Utf8PathBuf> {
    let source_resolved = source.canonicalize_utf8().unwrap_or(source.to_path_buf());
    let path_resolved = path.canonicalize_utf8().unwrap_or(path.to_path_buf());
    let relative = path_resolved.strip_prefix(&source_resolved).ok()?;
    Some(target.join(relative))
}

fn is_within(path: &Utf8Path, root: &Utf8Path) -> bool {
    let path_resolved = path.canonicalize_utf8().unwrap_or(path.to_path_buf());
    let root_resolved = root.canonicalize_utf8().unwrap_or(root.to_path_buf());
    path_resolved.starts_with(&root_resolved)
}

fn remove_empty_parents(path: &Utf8Path, stop_at: &Utf8Path) -> crate::Result<()> {
    let stop = stop_at.canonicalize_utf8().unwrap_or(stop_at.to_path_buf());
    let mut current = path.to_path_buf();
    loop {
        if same_path(&current, &stop) {
            return Ok(());
        }
        match fs::remove_dir(&current) {
            Ok(_) => {}
            Err(_) => return Ok(()),
        }
        current = match current.parent() {
            Some(p) => p.into(),
            None => return Ok(()),
        };
    }
}

fn write_profile_record(
    runtime_dir: &Utf8Path,
    profile: &ResolvedProviderProfile,
) -> crate::Result<Utf8PathBuf> {
    let path = runtime_dir.join("provider-profile.json");
    ccb_storage::atomic::atomic_write_json(&path, &profile.to_record())?;
    Ok(path)
}

fn same_path(left: &Utf8Path, right: &Utf8Path) -> bool {
    let left = Utf8PathBuf::from(expand_tilde(left.as_str()));
    let right = Utf8PathBuf::from(expand_tilde(right.as_str()));
    left.canonicalize_utf8()
        .unwrap_or(left.clone())
        .eq(&right.canonicalize_utf8().unwrap_or(right))
}

fn expand_tilde(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn utf8(p: &std::path::Path) -> &Utf8Path {
        Utf8Path::from_path(p).unwrap()
    }

    fn layout(tmp: &TempDir) -> ccb_storage::paths::PathLayout {
        ccb_storage::paths::PathLayout::new(utf8(tmp.path()))
    }

    #[test]
    fn test_provider_api_env_keys_codex() {
        let keys = provider_api_env_keys("codex");
        assert!(keys.contains("OPENAI_API_KEY"));
        assert!(keys.contains("OPENAI_BASE_URL"));
        assert!(!keys.contains("ANTHROPIC_API_KEY"));
    }

    #[test]
    fn test_validate_provider_runtime_home_policy_codex_with_home_ok() {
        let spec = ProviderProfileSpec {
            home: Some("/tmp/home".into()),
            ..Default::default()
        };
        assert!(validate_provider_runtime_home_policy("codex", &spec).is_ok());
    }

    #[test]
    fn test_validate_provider_runtime_home_policy_claude_with_home_err() {
        let spec = ProviderProfileSpec {
            home: Some("/tmp/home".into()),
            ..Default::default()
        };
        assert!(validate_provider_runtime_home_policy("claude", &spec).is_err());
    }

    #[test]
    fn test_validate_provider_runtime_home_uniqueness_detects_duplicate() {
        let tmp = TempDir::new().unwrap();
        let layout = layout(&tmp);
        let spec = ProviderProfileSpec::default();
        let specs = vec![("agent1", "codex", &spec), ("agent2", "codex", &spec)];
        assert!(validate_provider_runtime_home_uniqueness(&layout, specs.into_iter()).is_ok());
    }

    #[test]
    fn test_resolve_profile_root_default() {
        let tmp = TempDir::new().unwrap();
        let layout = layout(&tmp);
        let spec = ProviderProfileSpec::default();
        let root = resolve_profile_root(&layout, "agent1", "codex", &spec).unwrap();
        assert!(root.as_str().contains("provider-profiles/agent1/codex"));
    }

    #[test]
    fn test_replace_path_text() {
        let source = Utf8Path::new("/old/home");
        let target = Utf8Path::new("/new/home");
        let text = "CODEX_HOME=/old/home UNCHANGED=/old/home-suffix";
        let updated = replace_path_text(text, source, target);
        assert!(updated.contains("CODEX_HOME=/new/home"));
        assert!(updated.contains("UNCHANGED=/old/home-suffix"));
    }

    #[test]
    fn test_path_text_match_has_boundary() {
        assert!(path_text_match_has_boundary("CODEX_HOME=/old/home", 11, 20));
        assert!(!path_text_match_has_boundary("/old/home-suffix", 0, 9));
    }
}
