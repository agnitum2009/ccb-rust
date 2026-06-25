use std::collections::HashMap;
use std::fs;
use std::path::Path;

use camino::{Utf8Path, Utf8PathBuf};
use ccb_memory::render_provider_home_memory;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::models::{ProviderProfileSpec, ResolvedProviderProfile};

const CODEX_CUSTOM_PROVIDER_ID: &str = "custom";
const MANAGED_CODEX_DISABLED_FEATURES: &[&str] = &["external_migration"];
const CODEX_ACTIVITY_HOOK_TIMEOUT_S: i64 = 5;

const CODEX_ACTIVITY_HOOK_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "Stop",
];

const CODEX_OWNED_SKILL_NAMES: &[&str] = &["ask"];
const CODEX_LEGACY_OWNED_SKILL_NAMES: &[&str] = &["ccb_config", "ccb-config"];

const CODEX_SKILLS_PROJECTION_LABEL: &str = "codex-inherited-skills";
const CODEX_COMMANDS_PROJECTION_LABEL: &str = "codex-inherited-commands";
const CODEX_PLUGIN_PROJECTION_LABEL: &str = "codex-plugin-bundle";

const CODEX_PLUGIN_REQUIRED_RELATIVE_PATHS: &[&str] = &[
    ".agents/plugins/marketplace.json",
    ".agents/skills",
    "plugins",
];

/// Paths that make up a managed Codex home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexHomeLayout {
    pub codex_home: Utf8PathBuf,
    pub session_root: Utf8PathBuf,
}

/// Resolve the isolated Codex home layout for a runtime directory.
///
/// Mirrors Python `resolve_codex_home_layout`.
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

    if let Some(existing) = existing_codex_layout(runtime_dir) {
        return existing;
    }

    let isolated_home = managed_isolated_codex_home(runtime_dir);
    CodexHomeLayout {
        session_root: isolated_home.join("sessions"),
        codex_home: isolated_home,
    }
}

fn profile_runtime_home(profile: Option<&ResolvedProviderProfile>) -> Option<Utf8PathBuf> {
    let home = profile?.runtime_home.as_deref()?;
    let home = home.trim();
    if home.is_empty() {
        return None;
    }
    Some(Utf8PathBuf::from(home))
}

fn managed_isolated_codex_home(runtime_dir: &Utf8Path) -> Utf8PathBuf {
    runtime_dir.join("home")
}

fn existing_codex_layout(runtime_dir: &Utf8Path) -> Option<CodexHomeLayout> {
    let candidates = [
        runtime_dir.join("home"),
        runtime_dir.join(".codex").join("home"),
    ];
    for candidate in candidates {
        if candidate.join("config.toml").exists() {
            return Some(CodexHomeLayout {
                codex_home: candidate.clone(),
                session_root: candidate.join("sessions"),
            });
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct CodexApiAuthority {
    pub provider_id: String,
    pub base_url: String,
    pub wire_api: String,
    pub requires_openai_auth: bool,
}

/// Materialize a managed Codex home directory.
#[allow(clippy::too_many_arguments)]
pub fn materialize_codex_home_config(
    target_home: impl AsRef<Path>,
    profile: Option<&ProviderProfileSpec>,
    source_home: Option<&Utf8Path>,
    project_root: Option<&Utf8Path>,
    agent_name: Option<&str>,
    runtime_dir: Option<&Utf8Path>,
    workspace_path: Option<&Utf8Path>,
    shared_cache_root: Option<&Utf8Path>,
    memory_projection_event_path: Option<&Utf8Path>,
    memory_projection_marker_path: Option<&Utf8Path>,
) -> crate::Result<Utf8PathBuf> {
    let target_home = Utf8PathBuf::from_path_buf(target_home.as_ref().to_path_buf())
        .map_err(|p| crate::ProfilesError::Validation(format!("non-utf8 path: {:?}", p)))?;
    let source_home: Utf8PathBuf = match source_home {
        Some(p) => p.to_path_buf(),
        None => Utf8PathBuf::from_path_buf(
            ccb_provider_core::source_home::current_provider_source_home(),
        )
        .unwrap_or_else(|_| Utf8PathBuf::from("/tmp/.codex")),
    };

    fs::create_dir_all(&target_home)?;
    fs::create_dir_all(target_home.join("sessions"))?;

    let target_config = target_home.join("config.toml");
    let source_config = source_home.join("config.toml");
    let authority = codex_api_authority(profile);

    if let Some(ref authority) = authority {
        write_codex_api_authority_config(
            &target_config,
            authority,
            &source_config,
            project_root,
            workspace_path,
        )?;
    } else if inherits_config(profile)
        && inherits_api(profile)
        && source_config_valid(&source_config)
    {
        if source_config.is_file() {
            let payload = read_source_config_payload(&source_config);
            if !payload.is_empty() {
                write_managed_codex_config(&target_config, &payload, project_root, workspace_path)?;
            } else {
                sync_file(&source_config, &target_config)?;
                append_managed_codex_feature_overrides(&target_config)?;
                append_managed_codex_project_trust(&target_config, project_root, workspace_path)?;
            }
        } else {
            write_managed_config_stub(&target_config, project_root, workspace_path)?;
        }
    } else {
        write_managed_config_stub(&target_config, project_root, workspace_path)?;
    }

    materialize_auth_file(
        &source_home.join("auth.json"),
        &target_home.join("auth.json"),
        profile,
        authority.as_ref(),
    )?;

    copy_inherited_tree(
        &source_home.join("skills"),
        &target_home.join("skills"),
        inherits_skills(profile),
        CODEX_SKILLS_PROJECTION_LABEL,
    )?;

    project_role_skills_to_home(
        project_root,
        agent_name,
        "codex",
        &target_home.join("skills"),
    )?;

    route_inherited_tree(
        &source_home.join("commands"),
        &target_home.join("commands"),
        inherits_commands(profile),
        CODEX_COMMANDS_PROJECTION_LABEL,
    )?;

    sync_codex_plugin_projection(&source_home, &target_home, project_root, shared_cache_root)?;

    let memory_result = materialize_codex_memory(
        &source_home,
        &target_home,
        profile,
        project_root,
        agent_name,
        workspace_path,
    )?;

    ccb_provider_core::memory_projection::record_memory_projection_event(
        &memory_result,
        "codex",
        memory_projection_event_path.map(|p| p.as_std_path()),
        memory_projection_marker_path.map(|p| p.as_std_path()),
        agent_name,
    )
    .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;

    install_codex_activity_hooks(
        &target_home,
        &target_config,
        project_root,
        agent_name,
        runtime_dir,
        workspace_path,
    )?;

    Ok(target_config)
}

/// Re-install activity hooks for an existing managed Codex home.
pub fn repair_codex_activity_hooks(
    target_home: impl AsRef<Path>,
    project_root: Option<&Utf8Path>,
    agent_name: Option<&str>,
    runtime_dir: Option<&Utf8Path>,
    workspace_path: Option<&Utf8Path>,
) -> crate::Result<()> {
    let target_home = Utf8PathBuf::from_path_buf(target_home.as_ref().to_path_buf())
        .map_err(|p| crate::ProfilesError::Validation(format!("non-utf8 path: {:?}", p)))?;
    install_codex_activity_hooks(
        &target_home,
        &target_home.join("config.toml"),
        project_root,
        agent_name,
        runtime_dir,
        workspace_path,
    )
}

pub fn codex_api_authority(profile: Option<&ProviderProfileSpec>) -> Option<CodexApiAuthority> {
    if profile.is_none() || inherits_api(profile) {
        return None;
    }
    let env = profile_env(profile);
    let base_url = env
        .get("OPENAI_BASE_URL")
        .or_else(|| env.get("OPENAI_API_BASE"))
        .cloned()
        .unwrap_or_default();
    if base_url.is_empty() {
        return None;
    }
    Some(CodexApiAuthority {
        provider_id: CODEX_CUSTOM_PROVIDER_ID.into(),
        base_url,
        wire_api: "responses".into(),
        requires_openai_auth: false,
    })
}

pub fn codex_provider_authority_fingerprint(
    profile: Option<&ProviderProfileSpec>,
) -> Option<String> {
    let authority = codex_api_authority(profile)?;
    let payload = json!({
        "provider_id": authority.provider_id,
        "base_url": authority.base_url,
        "wire_api": authority.wire_api,
        "requires_openai_auth": authority.requires_openai_auth,
    });
    let encoded = serde_json::to_string(&payload).ok()?;
    let hash = Sha256::digest(encoded.as_bytes());
    Some(hex::encode(&hash[..8]))
}

fn inherits_api(profile: Option<&ProviderProfileSpec>) -> bool {
    profile.map(|p| p.inherit_api).unwrap_or(true)
}

fn inherits_auth(profile: Option<&ProviderProfileSpec>) -> bool {
    profile.map(|p| p.inherit_auth).unwrap_or(true)
}

fn inherits_config(profile: Option<&ProviderProfileSpec>) -> bool {
    profile.map(|p| p.inherit_config).unwrap_or(true)
}

fn inherits_skills(profile: Option<&ProviderProfileSpec>) -> bool {
    profile.map(|p| p.inherit_skills).unwrap_or(true)
}

fn inherits_commands(profile: Option<&ProviderProfileSpec>) -> bool {
    profile.map(|p| p.inherit_commands).unwrap_or(true)
}

fn inherits_memory(profile: Option<&ProviderProfileSpec>) -> bool {
    profile.map(|p| p.inherit_memory).unwrap_or(true)
}

fn profile_env(profile: Option<&ProviderProfileSpec>) -> HashMap<String, String> {
    profile
        .map(|p| {
            p.env
                .iter()
                .filter_map(|(k, v)| {
                    let trimmed = v.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some((k.clone(), trimmed.to_string()))
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn explicit_api_key(profile: Option<&ProviderProfileSpec>) -> String {
    profile_env(profile)
        .get("OPENAI_API_KEY")
        .cloned()
        .unwrap_or_default()
}

fn write_codex_api_authority_config(
    target: &Utf8Path,
    authority: &CodexApiAuthority,
    source_config: &Utf8Path,
    project_root: Option<&Utf8Path>,
    workspace_path: Option<&Utf8Path>,
) -> crate::Result<()> {
    fs::create_dir_all(target.parent().unwrap_or(target.as_ref()))?;
    let mut payload = managed_codex_config_payload(source_config, authority);
    trust_managed_codex_project_paths(&mut payload, project_root, workspace_path);
    let text = render_toml_document(&payload)?;
    ccb_storage::atomic::atomic_write_text(target, &text)?;
    Ok(())
}

fn write_managed_config_stub(
    target: &Utf8Path,
    project_root: Option<&Utf8Path>,
    workspace_path: Option<&Utf8Path>,
) -> crate::Result<()> {
    fs::create_dir_all(target.parent().unwrap_or(target.as_ref()))?;
    let mut table = toml::map::Map::new();
    trust_managed_codex_project_paths(&mut table, project_root, workspace_path);
    let text = if table.is_empty() {
        "# ccb agent-local codex config\n".into()
    } else {
        render_toml_document(&table)?
    };
    ccb_storage::atomic::atomic_write_text(target, &text)?;
    Ok(())
}

fn write_managed_codex_config(
    target: &Utf8Path,
    payload: &toml::map::Map<String, toml::Value>,
    project_root: Option<&Utf8Path>,
    workspace_path: Option<&Utf8Path>,
) -> crate::Result<()> {
    fs::create_dir_all(target.parent().unwrap_or(target.as_ref()))?;
    let mut sanitized = payload.clone();
    disable_interactive_migration_features(&mut sanitized);
    strip_unmanaged_hook_config(&mut sanitized);
    trust_managed_codex_project_paths(&mut sanitized, project_root, workspace_path);
    let text = render_toml_document(&sanitized)?;
    ccb_storage::atomic::atomic_write_text(target, &text)?;
    Ok(())
}

fn managed_codex_config_payload(
    source_config: &Utf8Path,
    authority: &CodexApiAuthority,
) -> toml::map::Map<String, toml::Value> {
    let mut payload = toml::map::Map::new();
    payload.insert(
        "model_provider".into(),
        toml::Value::String(authority.provider_id.clone()),
    );

    let inherited = strip_route_authority(&read_source_config_payload(source_config));
    for (key, value) in inherited {
        payload.insert(key, value);
    }

    let mut provider = toml::map::Map::new();
    provider.insert(
        "name".into(),
        toml::Value::String(authority.provider_id.clone()),
    );
    provider.insert(
        "wire_api".into(),
        toml::Value::String(authority.wire_api.clone()),
    );
    provider.insert(
        "requires_openai_auth".into(),
        toml::Value::Boolean(authority.requires_openai_auth),
    );
    provider.insert(
        "base_url".into(),
        toml::Value::String(authority.base_url.clone()),
    );

    let mut providers = toml::map::Map::new();
    providers.insert(authority.provider_id.clone(), toml::Value::Table(provider));
    payload.insert("model_providers".into(), toml::Value::Table(providers));

    disable_interactive_migration_features(&mut payload);
    payload
}

fn strip_route_authority(
    payload: &toml::map::Map<String, toml::Value>,
) -> toml::map::Map<String, toml::Value> {
    let mut cleaned = toml::map::Map::new();
    for (key, value) in payload {
        if key == "model_provider" || key == "model_providers" {
            continue;
        }
        cleaned.insert(key.clone(), value.clone());
    }
    cleaned
}

fn disable_interactive_migration_features(payload: &mut toml::map::Map<String, toml::Value>) {
    let mut features = match payload.get("features") {
        Some(toml::Value::Table(t)) => t.clone(),
        _ => toml::map::Map::new(),
    };
    for name in MANAGED_CODEX_DISABLED_FEATURES {
        features.insert((*name).into(), toml::Value::Boolean(false));
    }
    payload.insert("features".into(), toml::Value::Table(features));
}

fn strip_unmanaged_hook_config(payload: &mut toml::map::Map<String, toml::Value>) {
    payload.remove("hooks");
}

fn append_managed_codex_feature_overrides(target: &Utf8Path) -> crate::Result<()> {
    if !target.is_file() {
        return Ok(());
    }
    let text = safe_read_text(target);
    ccb_storage::atomic::atomic_write_text(
        target,
        &merge_managed_codex_feature_overrides(&text)?,
    )?;
    Ok(())
}

fn append_managed_codex_project_trust(
    target: &Utf8Path,
    project_root: Option<&Utf8Path>,
    workspace_path: Option<&Utf8Path>,
) -> crate::Result<()> {
    if !target.is_file() {
        return Ok(());
    }
    let text = safe_read_text(target);
    ccb_storage::atomic::atomic_write_text(
        target,
        &merge_managed_codex_project_trust(&text, project_root, workspace_path)?,
    )?;
    Ok(())
}

fn merge_managed_codex_feature_overrides(text: &str) -> crate::Result<String> {
    let lines: Vec<&str> = text.lines().collect();
    let override_lines: Vec<String> = MANAGED_CODEX_DISABLED_FEATURES
        .iter()
        .map(|name| format!("{} = false", name))
        .collect();

    match find_toml_table_index(&lines, "features") {
        None => {
            let mut merged = vec![text.trim_end().to_string()];
            merged.push(String::new());
            merged.push("[features]".into());
            merged.extend(override_lines);
            Ok(merged.join("\n") + "\n")
        }
        Some(idx) => {
            let section_end = toml_table_end(&lines, idx + 1);
            let disabled: std::collections::HashSet<_> =
                MANAGED_CODEX_DISABLED_FEATURES.iter().copied().collect();
            let mut section_lines: Vec<&str> = lines[idx + 1..section_end]
                .iter()
                .filter(|line| {
                    toml_key_name(line)
                        .map(|name| !disabled.contains(name))
                        .unwrap_or(true)
                })
                .copied()
                .collect();
            let mut insert_at = section_lines.len();
            while insert_at > 0 && section_lines[insert_at - 1].trim().is_empty() {
                insert_at -= 1;
            }
            for line in &override_lines {
                section_lines.insert(insert_at, line.as_str());
                insert_at += 1;
            }
            let mut merged: Vec<&str> = Vec::new();
            merged.extend(&lines[..idx + 1]);
            merged.extend(section_lines);
            merged.extend(&lines[section_end..]);
            Ok(merged.join("\n").trim_end().to_string() + "\n")
        }
    }
}

fn merge_managed_codex_project_trust(
    text: &str,
    project_root: Option<&Utf8Path>,
    workspace_path: Option<&Utf8Path>,
) -> crate::Result<String> {
    let paths = managed_codex_trusted_paths(project_root, workspace_path);
    if paths.is_empty() {
        return Ok(text.to_string());
    }
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    for path in paths {
        lines = merge_managed_codex_single_project_trust(lines, &path);
    }
    Ok(lines.join("\n").trim_end().to_string() + "\n")
}

fn merge_managed_codex_single_project_trust(lines: Vec<String>, project_path: &str) -> Vec<String> {
    let escaped = serde_json::Value::String(project_path.into()).to_string();
    let header = format!("[projects.{}]", escaped);
    let needle = format!("[projects.{}]", escaped);
    let project_index = lines.iter().position(|line| {
        let without_comment = line.split('#').next().unwrap_or("").trim();
        without_comment == needle
    });

    match project_index {
        None => {
            let mut result = lines;
            result.push(String::new());
            result.push(header);
            result.push("trust_level = \"trusted\"".into());
            result
        }
        Some(idx) => {
            let section_end = toml_table_end_str(&lines, idx + 1);
            let mut section_lines: Vec<String> = lines[idx + 1..section_end]
                .iter()
                .filter(|line| {
                    toml_key_name(line)
                        .map(|name| name != "trust_level")
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            let mut insert_at = section_lines.len();
            while insert_at > 0 && section_lines[insert_at - 1].trim().is_empty() {
                insert_at -= 1;
            }
            section_lines.insert(insert_at, "trust_level = \"trusted\"".into());
            let mut result: Vec<String> = Vec::new();
            result.extend(lines[..idx + 1].iter().cloned());
            result.extend(section_lines);
            result.extend(lines[section_end..].iter().cloned());
            result
        }
    }
}

fn trust_managed_codex_project_paths(
    payload: &mut toml::map::Map<String, toml::Value>,
    project_root: Option<&Utf8Path>,
    workspace_path: Option<&Utf8Path>,
) {
    let paths = managed_codex_trusted_paths(project_root, workspace_path);
    if paths.is_empty() {
        return;
    }
    let mut projects = match payload.get_mut("projects") {
        Some(toml::Value::Table(t)) => t.clone(),
        _ => toml::map::Map::new(),
    };
    for path in paths {
        let mut project = match projects.get(&path) {
            Some(toml::Value::Table(t)) => t.clone(),
            _ => toml::map::Map::new(),
        };
        project.insert("trust_level".into(), toml::Value::String("trusted".into()));
        projects.insert(path, toml::Value::Table(project));
    }
    payload.insert("projects".into(), toml::Value::Table(projects));
}

fn managed_codex_trusted_paths(
    project_root: Option<&Utf8Path>,
    workspace_path: Option<&Utf8Path>,
) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in [project_root, workspace_path].into_iter().flatten() {
        if let Some(trusted) = trusted_project_path(path) {
            if seen.insert(trusted.clone()) {
                normalized.push(trusted);
            }
        }
    }
    normalized
}

fn trusted_project_path(path: &Utf8Path) -> Option<String> {
    let expanded = Utf8PathBuf::from(expand_tilde(path.as_str()));
    let resolved = expanded.canonicalize_utf8().unwrap_or(expanded);
    if resolved.as_str().is_empty() {
        None
    } else {
        Some(resolved.to_string())
    }
}

fn find_toml_table_index(lines: &[&str], table_name: &str) -> Option<usize> {
    let needle = format!("[{}]", table_name);
    lines.iter().position(|line| {
        let without_comment = line.split('#').next().unwrap_or("").trim();
        without_comment == needle
    })
}

fn toml_table_end(lines: &[&str], start: usize) -> usize {
    let re = regex::Regex::new(r"^\s*\[{1,2}[^\]]+\]{1,2}\s*(?:#.*)?$").unwrap();
    for (idx, line) in lines.iter().enumerate().skip(start) {
        if re.is_match(line) {
            return idx;
        }
    }
    lines.len()
}

fn toml_table_end_str(lines: &[String], start: usize) -> usize {
    let re = regex::Regex::new(r"^\s*\[{1,2}[^\]]+\]{1,2}\s*(?:#.*)?$").unwrap();
    for (idx, line) in lines.iter().enumerate().skip(start) {
        if re.is_match(line) {
            return idx;
        }
    }
    lines.len()
}

fn toml_key_name(line: &str) -> Option<&str> {
    let candidate = line.split('#').next().unwrap_or("");
    let mut parts = candidate.splitn(2, '=');
    let raw_key = parts.next()?.trim();
    let re = regex::Regex::new(r"^[A-Za-z0-9_-]+$").unwrap();
    if re.is_match(raw_key) {
        Some(raw_key)
    } else {
        None
    }
}

fn read_source_config_payload(config_path: &Utf8Path) -> toml::map::Map<String, toml::Value> {
    if !config_path.is_file() {
        return toml::map::Map::new();
    }
    let text = safe_read_text(config_path);
    text.parse::<toml::Value>()
        .ok()
        .and_then(|v| v.as_table().cloned())
        .unwrap_or_default()
}

fn source_config_valid(config_path: &Utf8Path) -> bool {
    if !config_path.is_file() {
        return true;
    }
    let text = safe_read_text(config_path);
    text.parse::<toml::Value>().is_ok()
}

fn materialize_auth_file(
    source: &Utf8Path,
    target: &Utf8Path,
    profile: Option<&ProviderProfileSpec>,
    authority: Option<&CodexApiAuthority>,
) -> crate::Result<()> {
    if authority.is_some() {
        let key = explicit_api_key(profile);
        if !key.is_empty() {
            write_auth_file(target, &key)?;
        } else {
            let _ = fs::remove_file(target);
        }
        return Ok(());
    }
    sync_auth_file(source, target, profile)
}

fn sync_auth_file(
    source: &Utf8Path,
    target: &Utf8Path,
    profile: Option<&ProviderProfileSpec>,
) -> crate::Result<()> {
    if !inherits_auth(profile) || !source.is_file() {
        let _ = fs::remove_file(target);
        return Ok(());
    }
    sync_file(source, target)
}

fn write_auth_file(target: &Utf8Path, api_key: &str) -> crate::Result<()> {
    fs::create_dir_all(target.parent().unwrap_or(target.as_ref()))?;
    let payload = serde_json::json!({"OPENAI_API_KEY": api_key});
    let text = serde_json::to_string(&payload)? + "\n";
    ccb_storage::atomic::atomic_write_text(target, &text)?;
    Ok(())
}

fn sync_file(source: &Utf8Path, target: &Utf8Path) -> crate::Result<()> {
    if !source.is_file() {
        let _ = fs::remove_file(target);
        return Ok(());
    }
    fs::create_dir_all(target.parent().unwrap_or(target.as_ref()))?;
    fs::copy(source, target)?;
    Ok(())
}

fn copy_inherited_tree(
    source: &Utf8Path,
    target: &Utf8Path,
    enabled: bool,
    label: &str,
) -> crate::Result<()> {
    if !enabled {
        ccb_provider_core::projected_assets::remove_projected_path(
            target.as_std_path(),
            label,
            Some(source.as_std_path()),
            None,
            true,
        )
        .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;
        return Ok(());
    }
    if !source.is_dir() {
        ccb_provider_core::projected_assets::remove_projected_path(
            target.as_std_path(),
            label,
            Some(source.as_std_path()),
            None,
            true,
        )
        .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;
        return Ok(());
    }
    if same_path(source, target) {
        return Ok(());
    }

    let marker = Utf8PathBuf::from(format!("{}.ccb-projection.json", target));
    if (target.exists() || target.is_symlink()) && !marker.is_file() {
        if target.is_symlink() {
            repair_owned_codex_skill_entries(source, target)?;
            return Ok(());
        }
        if !target.is_dir()
            || ccb_provider_core::projected_assets::tree_content_fingerprint(target.as_std_path())
                != ccb_provider_core::projected_assets::tree_content_fingerprint(
                    source.as_std_path(),
                )
        {
            repair_owned_codex_skill_entries(source, target)?;
            return Ok(());
        }
    }

    if ccb_provider_core::projected_assets::copy_projected_tree_to_cache(
        source.as_std_path(),
        target.as_std_path(),
        label,
    )
    .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?
    {
        return Ok(());
    }
    ccb_provider_core::projected_assets::remove_projected_path(
        target.as_std_path(),
        label,
        Some(source.as_std_path()),
        None,
        true,
    )
    .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;
    Ok(())
}

fn route_inherited_tree(
    source: &Utf8Path,
    target: &Utf8Path,
    enabled: bool,
    label: &str,
) -> crate::Result<()> {
    ccb_provider_core::projected_assets::route_projected_tree(
        source.as_std_path(),
        target.as_std_path(),
        enabled,
        label,
        None,
        true,
    )
    .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;
    Ok(())
}

fn repair_owned_codex_skill_entries(source: &Utf8Path, target: &Utf8Path) -> crate::Result<()> {
    if !target.is_dir() || target.is_symlink() {
        return Ok(());
    }
    for legacy_name in CODEX_LEGACY_OWNED_SKILL_NAMES {
        let _ = fs::remove_dir_all(target.join(legacy_name));
    }
    for skill_name in CODEX_OWNED_SKILL_NAMES {
        let source_skill = source.join(skill_name);
        if !source_skill.is_dir() {
            continue;
        }
        let target_skill = target.join(skill_name);
        let _ = fs::remove_dir_all(&target_skill);
        copy_dir_all(&source_skill, &target_skill)?;
    }
    Ok(())
}

fn sync_codex_plugin_projection(
    source_home: &Utf8Path,
    target_home: &Utf8Path,
    project_root: Option<&Utf8Path>,
    shared_cache_root: Option<&Utf8Path>,
) -> crate::Result<()> {
    let source_tree = source_home.join(".tmp/plugins");
    let source_sha = source_home.join(".tmp/plugins.sha");
    let target_tree = target_home.join(".tmp/plugins");
    let target_sha = target_home.join(".tmp/plugins.sha");

    if !source_tree.is_dir() {
        ccb_provider_core::projected_assets::remove_projected_path(
            target_tree.as_std_path(),
            CODEX_PLUGIN_PROJECTION_LABEL,
            Some(source_tree.as_std_path()),
            None,
            true,
        )
        .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;
        let _ = fs::remove_file(&target_sha);
        return Ok(());
    }
    if same_path(&source_tree, &target_tree) {
        return Ok(());
    }

    let bundle_sha = codex_plugin_bundle_sha(&source_tree, &source_sha);
    let bundle_tree = bundle_sha.as_deref().and_then(|sha| {
        codex_plugin_shared_bundle_path(project_root, target_home, shared_cache_root, sha)
    });

    let projected;
    if let Some(ref bundle) = bundle_tree {
        if plugin_projection_is_current(&source_tree, &source_sha, &target_tree, &target_sha)
            && same_path(&target_tree, bundle)
        {
            ccb_provider_core::projected_assets::write_projected_marker(
                target_tree.as_std_path(),
                CODEX_PLUGIN_PROJECTION_LABEL,
                "symlink",
                bundle.as_std_path(),
            )
            .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;
            return Ok(());
        }
        if ccb_provider_core::projected_assets::copy_projected_tree_to_cache(
            source_tree.as_std_path(),
            bundle.as_std_path(),
            CODEX_PLUGIN_PROJECTION_LABEL,
        )
        .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?
        {
            ccb_provider_core::projected_assets::remove_projected_path(
                target_tree.as_std_path(),
                CODEX_PLUGIN_PROJECTION_LABEL,
                Some(source_tree.as_std_path()),
                None,
                true,
            )
            .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;
            fs::create_dir_all(target_tree.parent().unwrap_or(target_tree.as_ref()))?;
            #[cfg(unix)]
            {
                if std::os::unix::fs::symlink(bundle, &target_tree).is_ok() {
                    ccb_provider_core::projected_assets::write_projected_marker(
                        target_tree.as_std_path(),
                        CODEX_PLUGIN_PROJECTION_LABEL,
                        "symlink",
                        bundle.as_std_path(),
                    )
                    .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;
                    projected = true;
                } else {
                    projected = ccb_provider_core::projected_assets::route_projected_tree(
                        bundle.as_std_path(),
                        target_tree.as_std_path(),
                        true,
                        CODEX_PLUGIN_PROJECTION_LABEL,
                        None,
                        true,
                    )
                    .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;
                }
            }
            #[cfg(not(unix))]
            {
                projected = ccb_provider_core::projected_assets::route_projected_tree(
                    bundle.as_std_path(),
                    target_tree.as_std_path(),
                    true,
                    CODEX_PLUGIN_PROJECTION_LABEL,
                    None,
                    true,
                )
                .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;
            }
        } else {
            projected = ccb_provider_core::projected_assets::route_projected_tree(
                source_tree.as_std_path(),
                target_tree.as_std_path(),
                true,
                CODEX_PLUGIN_PROJECTION_LABEL,
                None,
                true,
            )
            .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;
        }
    } else {
        projected = ccb_provider_core::projected_assets::route_projected_tree(
            source_tree.as_std_path(),
            target_tree.as_std_path(),
            true,
            CODEX_PLUGIN_PROJECTION_LABEL,
            None,
            true,
        )
        .map_err(|e| crate::ProfilesError::Validation(e.to_string()))?;
    }

    if !projected || !plugin_required_paths_available(&source_tree, &target_tree) {
        return Ok(());
    }
    let _ = fs::remove_file(&target_sha);
    if source_sha.is_file() {
        sync_file(&source_sha, &target_sha)?;
    } else if let Some(ref sha) = bundle_sha {
        fs::create_dir_all(target_sha.parent().unwrap_or(target_sha.as_ref()))?;
        ccb_storage::atomic::atomic_write_text(&target_sha, &format!("{}\n", sha))?;
    }
    Ok(())
}

fn codex_plugin_bundle_sha(source_tree: &Utf8Path, source_sha: &Utf8Path) -> Option<String> {
    if source_sha.is_file() {
        let digest = safe_read_text(source_sha).trim().to_string();
        if !digest.is_empty() {
            return Some(safe_cache_segment(&digest));
        }
    }
    Some(ccb_provider_core::projected_assets::tree_content_fingerprint(source_tree.as_std_path()))
        .filter(|s| !s.is_empty())
}

fn safe_cache_segment(value: &str) -> String {
    let re = regex::Regex::new(r"[^A-Za-z0-9._-]+").unwrap();
    let normalized = re
        .replace_all(value.trim(), "-")
        .trim_matches(&['.', '-'][..])
        .to_string();
    if !normalized.is_empty() {
        normalized.chars().take(160).collect()
    } else {
        format!("{:x}", Sha256::digest(value.as_bytes()))
    }
}

fn codex_plugin_shared_bundle_path(
    project_root: Option<&Utf8Path>,
    _target_home: &Utf8Path,
    shared_cache_root: Option<&Utf8Path>,
    bundle_sha: &str,
) -> Option<Utf8PathBuf> {
    let cache_root = shared_cache_root
        .map(|p| p.to_path_buf())
        .or_else(|| project_root.map(|p| p.join(".ccb/shared-cache")))?;
    Some(cache_root.join("codex/plugin-bundles").join(bundle_sha))
}

fn plugin_projection_is_current(
    source_tree: &Utf8Path,
    source_sha: &Utf8Path,
    target_tree: &Utf8Path,
    target_sha: &Utf8Path,
) -> bool {
    if !target_tree.is_dir() {
        return false;
    }
    if !plugin_required_paths_available(source_tree, target_tree) {
        return false;
    }
    if source_sha.is_file() {
        return target_sha.is_file() && safe_read_text(source_sha) == safe_read_text(target_sha);
    }
    let source_fp =
        ccb_provider_core::projected_assets::tree_content_fingerprint(source_tree.as_std_path());
    let target_fp =
        ccb_provider_core::projected_assets::tree_content_fingerprint(target_tree.as_std_path());
    !source_fp.is_empty() && source_fp == target_fp
}

fn plugin_required_paths_available(source_tree: &Utf8Path, target_tree: &Utf8Path) -> bool {
    for relative in CODEX_PLUGIN_REQUIRED_RELATIVE_PATHS {
        if source_tree.join(relative).exists() && !target_tree.join(relative).exists() {
            return false;
        }
    }
    true
}

fn install_codex_activity_hooks(
    target_home: &Utf8Path,
    target_config: &Utf8Path,
    project_root: Option<&Utf8Path>,
    agent_name: Option<&str>,
    runtime_dir: Option<&Utf8Path>,
    workspace_path: Option<&Utf8Path>,
) -> crate::Result<()> {
    let (project_root, agent_name, runtime_dir, workspace_path) =
        match (project_root, agent_name, runtime_dir, workspace_path) {
            (Some(p), Some(a), Some(r), Some(w)) => (p, a, r, w),
            _ => return Ok(()),
        };

    let project_id = match project_id_for_path(project_root) {
        Some(id) if !id.is_empty() => id,
        _ => return Ok(()),
    };

    let hooks_path = target_home.join("hooks.json");
    let command = codex_activity_hook_command(&project_id, agent_name, runtime_dir, workspace_path);
    let event_groups = codex_activity_hook_events(&command);

    fs::create_dir_all(hooks_path.parent().unwrap_or(hooks_path.as_ref()))?;
    let hooks_payload = json!({"hooks": event_groups});
    let text = serde_json::to_string_pretty(&hooks_payload)? + "\n";
    ccb_storage::atomic::atomic_write_text(&hooks_path, &text)?;

    merge_codex_activity_hook_state(target_config, &hooks_path, &event_groups)?;
    Ok(())
}

fn project_id_for_path(project_root: &Utf8Path) -> Option<String> {
    let layout = ccb_storage::paths::PathLayout::new(project_root);
    let id = layout.project_id();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn codex_activity_hook_command(
    project_id: &str,
    agent_name: &str,
    runtime_dir: &Utf8Path,
    workspace_path: &Utf8Path,
) -> String {
    let hook_path = resolve_provider_activity_hook_path()
        .map(|p| p.to_string())
        .unwrap_or_else(|| "bin/ccb-provider-activity-hook".into());
    let parts: Vec<String> = vec![
        "python".into(),
        hook_path,
        "--provider".into(),
        "codex".into(),
        "--project-id".into(),
        project_id.into(),
        "--agent-name".into(),
        agent_name.into(),
        "--runtime-dir".into(),
        runtime_dir.to_string(),
        "--workspace".into(),
        workspace_path.to_string(),
    ];
    parts
        .into_iter()
        .map(|p| shell_escape::unix::escape(p.into()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Locate the absolute path to the CCB provider activity hook script.
///
/// Mirrors Python's use of `Path(__file__).resolve().parents[2] / 'bin'`.
/// At runtime the hook must be reachable from Codex's working directory,
/// so a relative path only works when the project root happens to be the CCB
/// install root. This helper falls back through runtime, build-time and
/// relative locations.
fn resolve_provider_activity_hook_path() -> Option<Utf8PathBuf> {
    let hook_name = "ccb-provider-activity-hook";

    // 1. Build-time crate workspace parent. This is the canonical CCB install
    // root where `bin/` and `lib/` live side-by-side, so the hook script can
    // resolve its Python dependencies. In release installs this is the install
    // prefix; in development it is the repository root.
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    // ccb-provider-profiles lives at <repo>/rust/crates/ccb-provider-profiles.
    let install_candidate = manifest_dir
        .parent()?
        .parent()?
        .parent()?
        .join("bin")
        .join(hook_name);
    if install_candidate.is_file() {
        return Some(install_candidate);
    }

    // 2. Same directory as the running executable (standalone release layout).
    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = exe.parent()?;
        let candidate = exe_dir.join(hook_name);
        if candidate.is_file() {
            return Utf8PathBuf::from_path_buf(candidate).ok();
        }
    }

    None
}

fn codex_activity_hook_events(command: &str) -> HashMap<String, Vec<serde_json::Value>> {
    let mut result = HashMap::new();
    for event_name in CODEX_ACTIVITY_HOOK_EVENTS {
        let group = json!({
            "hooks": [{
                "type": "command",
                "command": command,
                "timeout": CODEX_ACTIVITY_HOOK_TIMEOUT_S,
            }]
        });
        result.insert((*event_name).into(), vec![group]);
    }
    result
}

fn merge_codex_activity_hook_state(
    target_config: &Utf8Path,
    hooks_path: &Utf8Path,
    event_groups: &HashMap<String, Vec<serde_json::Value>>,
) -> crate::Result<()> {
    let source_path = hooks_path
        .canonicalize_utf8()
        .unwrap_or_else(|_| hooks_path.to_path_buf());
    let mut state_table = serde_json::Map::new();

    for (event_name, groups) in event_groups {
        let event_label = codex_hook_event_label(event_name);
        for (group_index, group) in groups.iter().enumerate() {
            let handlers = group
                .get("hooks")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for (handler_index, handler) in handlers.iter().enumerate() {
                let key = format!(
                    "{}:{}:{}:{}",
                    source_path, event_label, group_index, handler_index
                );
                let trusted_hash = codex_command_hook_hash(&event_label, group, handler);
                state_table.insert(
                    key,
                    json!({
                        "enabled": true,
                        "trusted_hash": trusted_hash,
                    }),
                );
            }
        }
    }

    fs::create_dir_all(target_config.parent().unwrap_or(target_config.as_ref()))?;
    let existing_text = safe_read_text(target_config);
    let payload = read_source_config_payload(target_config);
    if payload.is_empty() && !existing_text.trim().is_empty() {
        let replaced = replace_managed_codex_activity_state_block(&existing_text, &state_table);
        ccb_storage::atomic::atomic_write_text(target_config, &replaced)?;
        return Ok(());
    }

    let mut hooks_table = match payload.get("hooks") {
        Some(toml::Value::Table(t)) => t.clone(),
        _ => toml::map::Map::new(),
    };
    let state_toml = json_map_to_toml_table(&state_table);
    hooks_table.insert("state".into(), toml::Value::Table(state_toml));
    let mut new_payload = payload.clone();
    new_payload.insert("hooks".into(), toml::Value::Table(hooks_table));
    let text = render_toml_document(&new_payload)?;
    ccb_storage::atomic::atomic_write_text(target_config, &text)?;
    Ok(())
}

fn replace_managed_codex_activity_state_block(
    text: &str,
    state_table: &serde_json::Map<String, serde_json::Value>,
) -> String {
    let begin = "# ccb managed codex activity hook state: begin";
    let end = "# ccb managed codex activity hook state: end";
    let lines: Vec<&str> = text.lines().collect();
    let mut cleaned: Vec<&str> = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if lines[index].trim() == begin {
            index += 1;
            while index < lines.len() && lines[index].trim() != end {
                index += 1;
            }
            if index < lines.len() {
                index += 1;
            }
            continue;
        }
        cleaned.push(lines[index]);
        index += 1;
    }
    let block = {
        let mut table = toml::map::Map::new();
        let mut hooks = toml::map::Map::new();
        hooks.insert("state".into(), toml::Value::Table(json_map_to_toml_table(state_table)));
        table.insert("hooks".into(), toml::Value::Table(hooks));
        render_toml_document(&table)
            .unwrap_or_default()
            .trim_end()
            .to_string()
    };
    let mut result = cleaned.join("\n");
    result.push('\n');
    result.push('\n');
    result.push_str(begin);
    result.push('\n');
    result.push_str(&block);
    result.push('\n');
    result.push_str(end);
    result.push('\n');
    result.trim_start().to_string()
}

fn json_map_to_toml_table(map: &serde_json::Map<String, serde_json::Value>) -> toml::map::Map<String, toml::Value> {
    map.iter()
        .map(|(key, value)| {
            let inner = match value {
                serde_json::Value::Object(obj) => obj
                    .iter()
                    .map(|(inner_key, inner_value)| {
                        let toml_value = match inner_value {
                            serde_json::Value::Bool(b) => toml::Value::Boolean(*b),
                            serde_json::Value::String(s) => toml::Value::String(s.clone()),
                            serde_json::Value::Number(n) => n
                                .as_i64()
                                .map(toml::Value::Integer)
                                .or_else(|| n.as_f64().map(toml::Value::Float))
                                .unwrap_or_else(|| toml::Value::String(inner_value.to_string())),
                            _ => toml::Value::String(inner_value.to_string()),
                        };
                        (inner_key.clone(), toml_value)
                    })
                    .collect(),
                _ => toml::map::Map::new(),
            };
            (key.clone(), toml::Value::Table(inner))
        })
        .collect()
}

fn codex_hook_event_label(event_name: &str) -> String {
    event_name
        .chars()
        .enumerate()
        .map(|(idx, ch)| {
            if ch.is_uppercase() && idx > 0 {
                format!("_{}", ch.to_lowercase())
            } else {
                ch.to_lowercase().to_string()
            }
        })
        .collect()
}

fn codex_command_hook_hash(
    event_label: &str,
    group: &serde_json::Value,
    handler: &serde_json::Value,
) -> String {
    let mut normalized_handler = serde_json::Map::new();
    normalized_handler.insert("type".into(), json!("command"));
    normalized_handler.insert(
        "command".into(),
        json!(handler
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")),
    );
    normalized_handler.insert(
        "timeout".into(),
        json!(handler
            .get("timeout")
            .and_then(|v| v.as_i64())
            .unwrap_or(CODEX_ACTIVITY_HOOK_TIMEOUT_S)),
    );
    normalized_handler.insert(
        "async".into(),
        json!(handler
            .get("async")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)),
    );
    if handler.get("statusMessage").is_some() {
        normalized_handler.insert(
            "statusMessage".into(),
            json!(handler
                .get("statusMessage")
                .and_then(|v| v.as_str())
                .unwrap_or("")),
        );
    }
    let mut normalized_group = serde_json::Map::new();
    if group.get("matcher").is_some() {
        normalized_group.insert(
            "matcher".into(),
            json!(group.get("matcher").and_then(|v| v.as_str()).unwrap_or("")),
        );
    }
    normalized_group.insert("hooks".into(), json!([normalized_handler]));
    let mut identity = serde_json::Map::new();
    identity.insert("event_name".into(), json!(event_label));
    for (k, v) in normalized_group {
        identity.insert(k, v);
    }
    let encoded = serde_json::to_string(&canonical_json(&serde_json::Value::Object(identity)))
        .unwrap_or_default();
    let hash = Sha256::digest(encoded.as_bytes());
    format!("sha256:{:x}", hash)
}

fn canonical_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(obj) => {
            let mut keys: Vec<_> = obj.keys().collect();
            keys.sort();
            let mut sorted = serde_json::Map::new();
            for k in keys {
                sorted.insert(k.clone(), canonical_json(&obj[k]));
            }
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(canonical_json).collect())
        }
        _ => value.clone(),
    }
}

fn materialize_codex_memory(
    source_home: &Utf8Path,
    target_home: &Utf8Path,
    profile: Option<&ProviderProfileSpec>,
    project_root: Option<&Utf8Path>,
    agent_name: Option<&str>,
    workspace_path: Option<&Utf8Path>,
) -> crate::Result<ccb_provider_core::memory_projection::MemoryProjectionResult> {
    let target = target_home.join("AGENTS.md");
    if same_path(source_home, target_home) {
        return Ok(
            ccb_provider_core::memory_projection::memory_projection_result(
                "skipped",
                "source_home_is_target_home",
                target.as_std_path(),
                None,
                None,
                None,
                None,
            ),
        );
    }
    if !inherits_memory(profile) {
        let _ = fs::remove_file(&target);
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
    if project_root.is_none() || agent_name.is_none() {
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
    }
    materialize_provider_memory_file(
        project_root.unwrap(),
        agent_name.unwrap(),
        "codex",
        &target,
        &source_home.join("AGENTS.md"),
        workspace_path,
    )
}

fn materialize_provider_memory_file(
    project_root: &Utf8Path,
    agent_name: &str,
    provider: &str,
    target: &Utf8Path,
    provider_memory_path: &Utf8Path,
    workspace_path: Option<&Utf8Path>,
) -> crate::Result<ccb_provider_core::memory_projection::MemoryProjectionResult> {
    let content = render_provider_home_memory(
        project_root.as_std_path(),
        agent_name,
        provider,
        workspace_path.map(|p| p.as_std_path()),
        Some(provider_memory_path.as_std_path()),
    )
    .map_err(|e| {
        crate::ProfilesError::Validation(format!("failed to render memory bundle: {e}"))
    })?;

    if content.trim().is_empty() {
        let _ = fs::remove_file(target);
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

    fs::create_dir_all(target.parent().unwrap_or(target.as_ref()))?;
    ccb_storage::atomic::atomic_write_text(target, &content)?;

    let sha256 = ccb_provider_core::memory_projection::text_file_sha256(target.as_std_path());
    Ok(
        ccb_provider_core::memory_projection::memory_projection_result(
            "materialized",
            "provider_memory_file_written",
            target.as_std_path(),
            Some(&sha256),
            Some(1),
            None,
            None,
        ),
    )
}

fn project_role_skills_to_home(
    project_root: Option<&Utf8Path>,
    agent_name: Option<&str>,
    provider: &str,
    target_skills_dir: &Utf8Path,
) -> crate::Result<()> {
    let (project_root, agent_name) = match (project_root, agent_name) {
        (Some(p), Some(a)) => (p, a),
        _ => return Ok(()),
    };
    let rolepack_dir = project_root
        .join(".ccb")
        .join("rolepacks")
        .join(agent_name)
        .join(provider)
        .join("skills");
    if !rolepack_dir.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(target_skills_dir)?;
    copy_dir_all(&rolepack_dir, target_skills_dir)?;
    Ok(())
}

fn copy_dir_all(src: &Utf8Path, dst: &Utf8Path) -> crate::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let dest = dst.as_std_path().join(file_name);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_all(
                &Utf8PathBuf::from_path_buf(path)
                    .unwrap_or_else(|_| Utf8PathBuf::from("/dev/null")),
                &Utf8PathBuf::from_path_buf(dest)
                    .unwrap_or_else(|_| Utf8PathBuf::from("/dev/null")),
            )?;
        } else if file_type.is_file() {
            fs::copy(&path, &dest)?;
        } else if file_type.is_symlink() {
            #[cfg(unix)]
            {
                let link_target = fs::read_link(&path)?;
                std::os::unix::fs::symlink(link_target, dest)?;
            }
            #[cfg(not(unix))]
            {
                let _ = (path, dest);
            }
        }
    }
    Ok(())
}

fn same_path(left: &Utf8Path, right: &Utf8Path) -> bool {
    let left = Utf8PathBuf::from(expand_tilde(left.as_str()));
    let right = Utf8PathBuf::from(expand_tilde(right.as_str()));
    left.canonicalize_utf8()
        .unwrap_or(left.clone())
        .eq(&right.canonicalize_utf8().unwrap_or(right))
}

fn safe_read_text(path: &Utf8Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn expand_tilde(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    raw.to_string()
}

fn render_toml_document(payload: &toml::map::Map<String, toml::Value>) -> crate::Result<String> {
    let value = toml::Value::Table(payload.clone());
    let mut text = toml::ser::to_string_pretty(&value)?;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_codex_api_authority_inherit_returns_none() {
        let spec = ProviderProfileSpec::default();
        assert!(codex_api_authority(Some(&spec)).is_none());
    }

    #[test]
    fn test_codex_api_authority_from_env() {
        let mut env = HashMap::new();
        env.insert("OPENAI_BASE_URL".into(), "https://api.example.test".into());
        let spec = ProviderProfileSpec {
            inherit_api: false,
            env,
            ..Default::default()
        };
        let authority = codex_api_authority(Some(&spec)).unwrap();
        assert_eq!(authority.provider_id, "custom");
        assert_eq!(authority.base_url, "https://api.example.test");
    }

    #[test]
    fn test_disable_interactive_migration_features() {
        let mut payload = toml::map::Map::new();
        payload.insert("model".into(), toml::Value::String("gpt-5".into()));
        disable_interactive_migration_features(&mut payload);
        let features = payload.get("features").unwrap().as_table().unwrap();
        assert_eq!(
            features.get("external_migration"),
            Some(&toml::Value::Boolean(false))
        );
    }

    #[test]
    fn test_merge_managed_codex_feature_overrides_adds_section() {
        let text = "model = \"gpt-5\"\n";
        let merged = merge_managed_codex_feature_overrides(text).unwrap();
        assert!(merged.contains("[features]"));
        assert!(merged.contains("external_migration = false"));
    }

    #[test]
    fn test_merge_managed_codex_feature_overrides_updates_section() {
        let text = "[features]\nexternal_migration = true\nmemories = true\n";
        let merged = merge_managed_codex_feature_overrides(text).unwrap();
        assert_eq!(merged.matches("[features]").count(), 1);
        assert!(merged.contains("external_migration = false"));
        assert!(merged.contains("memories = true"));
    }

    #[test]
    fn test_write_managed_config_stub_adds_project_trust() {
        let dir = TempDir::new().unwrap();
        let target_path = dir.path().join("config.toml");
        let target = Utf8Path::from_path(&target_path).unwrap();
        let project_root_path = dir.path().join("repo");
        let project_root = Utf8Path::from_path(&project_root_path).unwrap();
        fs::create_dir_all(project_root).unwrap();
        write_managed_config_stub(target, Some(project_root), None).unwrap();
        let text = fs::read_to_string(target).unwrap();
        assert!(text.contains("trust_level = \"trusted\""));
    }

    #[test]
    fn test_copy_inherited_tree_creates_projection() {
        let dir = TempDir::new().unwrap();
        let source_path = dir.path().join("source");
        let source = Utf8Path::from_path(&source_path).unwrap();
        let target_path = dir.path().join("target");
        let target = Utf8Path::from_path(&target_path).unwrap();
        let marker_path = dir.path().join("target.ccb-projection.json");
        fs::create_dir_all(source.join("demo")).unwrap();
        ccb_storage::atomic::atomic_write_text(&source.join("demo/SKILL.md"), "demo\n").unwrap();

        copy_inherited_tree(source, target, true, "test-label").unwrap();
        assert!(target.join("demo/SKILL.md").is_file());
        assert!(Utf8Path::from_path(&marker_path).unwrap().is_file());
    }
}
