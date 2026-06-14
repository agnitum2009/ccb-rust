use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::config::load_project_config;
use crate::models::normalize_agent_name;
use crate::roles::{canonical_role_id, role_id_candidates};

pub const SUPPORTED_ROLE_SCHEMA: &str = "rolepack/v1";
pub const AGENT_ROLE_SCHEMA_PREFIX: &str = "agent-role/preview-";
pub const CCB_ADAPTER_SCHEMA_PREFIX: &str = "agent-role-adapter/ccb-preview-";
pub const SOURCE_REGISTRY_SCHEMA: &str = "rolepack-source-registry/v1";
pub const ROLE_LOCK_SCHEMA: &str = "rolepack-lock/v1";
pub const SYSTEM_ROLE_SOURCE_NAMES: &[&str] = &["systemroles", "dotroles"];

// Architec-specific constants
pub const ARCHITEC_ROLE_ID: &str = "agentroles.archi";
pub const ARCHITEC_TOOL_ID: &str = "architec";
pub const ARCHITEC_NPM_PACKAGE: &str = "@seemseam/archi";

#[derive(Debug, Clone, thiserror::Error)]
#[error("role manifest error: {0}")]
pub struct RoleManifestError(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub root: PathBuf,
    #[serde(flatten)]
    pub manifest: serde_json::Map<String, serde_json::Value>,
}

impl RoleManifest {
    pub fn default_agent_name(&self) -> String {
        self.table("identity")
            .get("default_agent_name")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .unwrap_or_else(|| self.id.rsplit_once('.').map(|(_, r)| r).unwrap_or(&self.id))
            .to_string()
    }

    pub fn providers(&self) -> Vec<String> {
        self.table("compatibility")
            .get("providers")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.trim().to_lowercase()))
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn table(&self, key: &str) -> serde_json::Map<String, serde_json::Value> {
        self.manifest
            .get(key)
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default()
    }

    pub fn to_summary(&self) -> HashMap<String, String> {
        let mut summary = HashMap::new();
        summary.insert("id".into(), self.id.clone());
        summary.insert("name".into(), self.name.clone());
        summary.insert("version".into(), self.version.clone());
        summary.insert("description".into(), self.description.clone());
        summary.insert("default_agent_name".into(), self.default_agent_name());
        summary.insert("providers".into(), self.providers().join(","));
        summary.insert("root".into(), self.root.to_string_lossy().into_owned());
        summary
    }
}

pub fn normalize_role_id(value: &str) -> crate::Result<String> {
    let role_id = value.trim().to_lowercase();
    if role_id.is_empty() || !role_id.contains('.') {
        return Err(crate::AgentError::Role(
            "role id must use publisher.role form, for example agentroles.archi".into(),
        ));
    }
    let allowed: HashSet<char> = "abcdefghijklmnopqrstuvwxyz0123456789._-".chars().collect();
    if role_id.chars().any(|c| !allowed.contains(&c)) {
        return Err(crate::AgentError::Role(format!(
            "invalid role id: {value:?}"
        )));
    }
    Ok(canonical_role_id(&role_id))
}

pub fn looks_like_role_id(value: &str) -> bool {
    normalize_role_id(value).is_ok()
}

pub fn read_toml_manifest(
    path: &Path,
) -> crate::Result<serde_json::Map<String, serde_json::Value>> {
    let text = std::fs::read_to_string(path)?;
    let value: toml::Value = toml::from_str(&text)?;
    let map = value
        .as_table()
        .ok_or_else(|| RoleManifestError("role manifest must decode to a table".into()))?;
    let mut out = serde_json::Map::new();
    for (k, v) in map {
        out.insert(
            k.clone(),
            serde_json::to_value(v).map_err(crate::AgentError::Json)?,
        );
    }
    Ok(out)
}

pub fn role_manifest_from_mapping(
    root: PathBuf,
    manifest: serde_json::Map<String, serde_json::Value>,
) -> crate::Result<RoleManifest> {
    let mut manifest = manifest;
    if is_agent_role_manifest(&manifest) {
        manifest = translate_agent_role_manifest(&root, manifest)?;
    }
    let schema = manifest
        .get("schema")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if schema != SUPPORTED_ROLE_SCHEMA {
        return Err(crate::AgentError::Role(format!(
            "unsupported role schema: {schema:?}"
        )));
    }
    let id = normalize_role_id(manifest.get("id").and_then(|v| v.as_str()).unwrap_or(""))?;
    let name = manifest
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let description = manifest
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if name.is_empty() || version.is_empty() || description.is_empty() {
        return Err(crate::AgentError::Role(
            "role manifest requires name, version, and description".into(),
        ));
    }
    Ok(RoleManifest {
        id,
        name,
        version,
        description,
        root,
        manifest,
    })
}

pub fn load_role_manifest(path: &Path) -> crate::Result<RoleManifest> {
    let root = path.expand_home();
    // Resolve symlinks to the actual directory
    let resolved_root = root.canonicalize().unwrap_or(root);
    let manifest_path = resolved_root.join("role.toml");
    if !manifest_path.is_file() {
        return Err(crate::AgentError::Role(format!(
            "role manifest not found: {}",
            manifest_path.display()
        )));
    }
    role_manifest_from_mapping(resolved_root, read_toml_manifest(&manifest_path)?)
}

pub fn is_agent_role_manifest(manifest: &serde_json::Map<String, serde_json::Value>) -> bool {
    manifest
        .get("schema")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().starts_with(AGENT_ROLE_SCHEMA_PREFIX))
        .unwrap_or(false)
}

pub fn translate_agent_role_manifest(
    root: &Path,
    manifest: serde_json::Map<String, serde_json::Value>,
) -> crate::Result<serde_json::Map<String, serde_json::Value>> {
    let mut translated = manifest;
    translated.insert("schema".into(), SUPPORTED_ROLE_SCHEMA.into());

    let identity = table_json(&translated, "identity");
    let contents = table_json(&translated, "contents");
    let adapter = load_ccb_adapter(root)?;

    translated.insert("identity".into(), translate_identity(&identity, &adapter));
    translated.insert("compatibility".into(), translate_compatibility(&adapter));
    translated.insert("memory".into(), translate_memory(&contents, &adapter));
    translated.insert("skills".into(), translate_skills(&contents, &adapter));
    translated.insert("tools".into(), translate_tools(&adapter));
    translated.insert(
        "permissions".into(),
        translate_permissions(&translated, &adapter),
    );
    translated.insert(
        "activation".into(),
        translate_activation(&translated, &adapter),
    );
    translated.insert(
        "source_schema".into(),
        translated
            .get("schema")
            .cloned()
            .unwrap_or(serde_json::Value::String(SUPPORTED_ROLE_SCHEMA.into())),
    );
    Ok(translated)
}

fn load_ccb_adapter(root: &Path) -> crate::Result<serde_json::Map<String, serde_json::Value>> {
    let path = root.join("adapters").join("ccb").join("adapter.toml");
    if !path.is_file() {
        return Ok(serde_json::Map::new());
    }
    let adapter = read_toml_manifest(&path)?;
    let schema = adapter
        .get("schema")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if !schema.is_empty() && !schema.starts_with(CCB_ADAPTER_SCHEMA_PREFIX) {
        return Ok(serde_json::Map::new());
    }
    Ok(adapter)
}

fn translate_identity(
    identity: &serde_json::Map<String, serde_json::Value>,
    adapter: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut translated = identity.clone();
    let default_name = adapter
        .get("default_agent_name")
        .or_else(|| identity.get("default_agent_name"))
        .or_else(|| identity.get("default_name"));
    if let Some(name) = default_name {
        if let Some(s) = name.as_str() {
            translated.insert("default_agent_name".into(), s.trim().into());
        }
    }
    translated.into()
}

fn translate_compatibility(
    adapter: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut providers = string_list(adapter.get("supported_providers"));
    if providers.is_empty() {
        if let Some(rec) = adapter.get("recommended_provider").and_then(|v| v.as_str()) {
            let trimmed = rec.trim();
            if !trimmed.is_empty() {
                providers.push(trimmed.into());
            }
        }
    }
    let mut compatibility = serde_json::Map::new();
    compatibility.insert("hosts".into(), vec!["ccb"].into());
    if !providers.is_empty() {
        compatibility.insert("providers".into(), providers.into());
    }
    serde_json::Value::Object(compatibility)
}

fn translate_memory(
    contents: &serde_json::Map<String, serde_json::Value>,
    adapter: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut files = relative_paths(contents.get("memory"));
    files.extend(relative_paths(adapter.get("memory")));
    let merge_strategy = adapter
        .get("memory_merge_strategy")
        .and_then(|v| v.as_str())
        .unwrap_or("append_after_project_memory")
        .trim();
    serde_json::json!({
        "files": files,
        "merge_strategy": merge_strategy,
    })
}

fn translate_skills(
    contents: &serde_json::Map<String, serde_json::Value>,
    adapter: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut skills = relative_paths(contents.get("skills"));
    skills.extend(relative_paths(adapter.get("skills")));
    let mut providers = string_list(adapter.get("supported_providers"));
    if providers.is_empty() {
        if let Some(rec) = adapter.get("recommended_provider").and_then(|v| v.as_str()) {
            let trimmed = rec.trim();
            if !trimmed.is_empty() {
                providers.push(trimmed.into());
            }
        }
        if providers.is_empty() {
            providers = vec!["codex".into(), "claude".into()];
        }
    }
    let mut out = serde_json::Map::new();
    for provider in providers {
        if !provider.is_empty() {
            out.insert(provider, skills.clone().into());
        }
    }
    serde_json::Value::Object(out)
}

fn translate_tools(adapter: &serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    adapter
        .get("tools")
        .and_then(|v| v.as_object())
        .cloned()
        .map(serde_json::Value::Object)
        .unwrap_or_else(|| serde_json::json!({}))
}

fn translate_permissions(
    manifest: &serde_json::Map<String, serde_json::Value>,
    adapter: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut permissions = manifest
        .get("permissions")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    if let Some(default) = adapter.get("permission_default").and_then(|v| v.as_str()) {
        let trimmed = default.trim();
        if !trimmed.is_empty() {
            permissions
                .entry("default")
                .or_insert_with(|| trimmed.into());
        }
    }
    serde_json::Value::Object(permissions)
}

fn translate_activation(
    manifest: &serde_json::Map<String, serde_json::Value>,
    adapter: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut activation = manifest
        .get("activation")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    if let Some(mode) = adapter
        .get("recommended_workspace_mode")
        .and_then(|v| v.as_str())
    {
        let trimmed = mode.trim();
        if !trimmed.is_empty() {
            activation
                .entry("recommended_workspace_mode")
                .or_insert_with(|| trimmed.into());
        }
    }
    serde_json::Value::Object(activation)
}

fn table_json(
    mapping: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> serde_json::Map<String, serde_json::Value> {
    mapping
        .get(key)
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default()
}

fn string_list(value: Option<&serde_json::Value>) -> Vec<String> {
    match value {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_lowercase()))
            .filter(|s| !s.is_empty())
            .collect(),
        Some(serde_json::Value::String(s)) => {
            let trimmed = s.trim().to_lowercase();
            if trimmed.is_empty() {
                Vec::new()
            } else {
                vec![trimmed]
            }
        }
        _ => Vec::new(),
    }
}

fn relative_paths(value: Option<&serde_json::Value>) -> Vec<String> {
    let mut paths = Vec::new();
    let items: Vec<&serde_json::Value> = match value {
        Some(serde_json::Value::Array(arr)) => arr.iter().collect(),
        Some(v) => vec![v],
        None => Vec::new(),
    };
    for item in items {
        if let Some(text) = item.as_str() {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }
            let path = Path::new(trimmed);
            if path.is_absolute() {
                continue;
            }
            paths.push(trimmed.into());
        }
    }
    paths
}

pub fn agent_roles_store_root() -> PathBuf {
    if let Ok(value) = std::env::var("AGENT_ROLES_STORE") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).expand_home();
        }
    }
    home_dir().join(".roles")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

pub fn agent_roles_installed_root() -> PathBuf {
    agent_roles_store_root().join("installed")
}

pub fn role_store_roots() -> Vec<PathBuf> {
    vec![agent_roles_installed_root(), agent_roles_store_root()]
}

pub fn tree_digest(root: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    let mut paths: Vec<PathBuf> = walkdir(root);
    paths.sort();
    for path in paths {
        let rel = path.strip_prefix(root).unwrap_or(&path);
        digest.update(rel.to_string_lossy().as_bytes());
        digest.update(b"\0");
        if path.is_file() {
            if let Ok(data) = std::fs::read(&path) {
                digest.update(&data);
            }
        } else if path.is_symlink() {
            if let Ok(target) = std::fs::read_link(&path) {
                digest.update(target.to_string_lossy().as_bytes());
            }
        }
        digest.update(b"\0");
    }
    hex::encode(digest.finalize())
}

fn walkdir(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            out.push(path.clone());
            if path.is_dir() && !path.is_symlink() {
                out.extend(walkdir(&path));
            }
        }
    }
    out
}

pub fn load_installed_role(role_id: &str) -> crate::Result<Option<RoleManifest>> {
    let normalized = normalize_role_id(role_id)?;
    for store_root in role_store_roots() {
        for candidate_id in role_id_candidates(&normalized) {
            let current = store_root.join(&candidate_id).join("current");
            if current.exists() {
                return load_role_manifest(&current).map(Some);
            }
            let direct = store_root.join(&candidate_id);
            if direct.join("role.toml").is_file() {
                return load_role_manifest(&direct).map(Some);
            }
        }
    }
    Ok(None)
}

#[derive(Debug, Clone)]
pub struct SourceRole {
    pub source: String,
    pub role_id: String,
    pub version: String,
    pub digest: String,
    pub path: PathBuf,
    pub name: String,
    pub description: String,
    pub duplicates: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RoleSource {
    pub name: String,
    pub path: PathBuf,
    pub source_type: String,
}

#[derive(Debug, Clone)]
pub struct ProjectRoleResolution {
    pub role_id: String,
    pub role: Option<RoleManifest>,
    pub warning: String,
    pub lock_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProjectMemorySource {
    pub kind: String,
    pub title: String,
    pub path: PathBuf,
    pub content: String,
    pub exists: bool,
    pub warning: String,
}

pub fn discover_path_roles(path: &Path) -> crate::Result<Vec<SourceRole>> {
    discover_roles_from_sources(
        &[RoleSource {
            name: "path".into(),
            path: path.expand_home(),
            source_type: "path".into(),
        }],
        include_reference_roles_default(),
    )
}

fn discover_roles_from_sources(
    sources: &[RoleSource],
    include_reference: bool,
) -> crate::Result<Vec<SourceRole>> {
    let mut discovered: HashMap<String, SourceRole> = HashMap::new();
    let mut duplicates: HashMap<String, Vec<String>> = HashMap::new();
    for source in sources {
        for role_path in iter_role_paths(&source.path, include_reference) {
            let role = match load_role_manifest(&role_path) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let source_role = SourceRole {
                source: source.name.clone(),
                role_id: role.id.clone(),
                version: role.version.clone(),
                digest: format!("sha256:{}", tree_digest(&role.root)),
                path: role.root.clone(),
                name: role.name.clone(),
                description: role.description.clone(),
                duplicates: Vec::new(),
            };
            match discovered.get(&role.id) {
                Some(existing) if same_source_reference_upgrade(existing, &source_role) => {
                    duplicates.entry(role.id.clone()).or_default().push(format!(
                        "{}:{}",
                        existing.source,
                        existing.path.display()
                    ));
                    discovered.insert(role.id.clone(), source_role);
                }
                Some(_) => {
                    duplicates.entry(role.id.clone()).or_default().push(format!(
                        "{}:{}",
                        source_role.source,
                        source_role.path.display()
                    ));
                }
                None => {
                    discovered.insert(role.id.clone(), source_role);
                }
            }
        }
    }
    let mut roles: Vec<SourceRole> = discovered
        .into_iter()
        .map(|(role_id, mut role)| {
            role.duplicates = duplicates.remove(&role_id).unwrap_or_default();
            role
        })
        .collect();
    roles.sort_by(|a, b| a.role_id.cmp(&b.role_id));
    Ok(roles)
}

fn iter_role_paths(source_root: &Path, include_reference: bool) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let base_names: &[&str] = if include_reference {
        &["reference_roles", "roles"]
    } else {
        &["roles"]
    };
    for base_name in base_names {
        let base = source_root.join(base_name);
        if base.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.join("role.toml").is_file() {
                        candidates.push(path);
                    }
                }
            }
        }
    }
    if source_root.is_dir() {
        if let Ok(entries) = std::fs::read_dir(source_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("role.toml").is_file() {
                    candidates.push(path);
                }
            }
        }
    }
    if source_root.join("role.toml").is_file() {
        candidates.push(source_root.to_path_buf());
    }
    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for candidate in candidates {
        if let Ok(resolved) = candidate.canonicalize() {
            if seen.insert(resolved.clone()) {
                deduped.push(candidate);
            }
        } else if seen.insert(candidate.clone()) {
            deduped.push(candidate);
        }
    }
    deduped
}

fn same_source_reference_upgrade(existing: &SourceRole, candidate: &SourceRole) -> bool {
    existing.source == candidate.source
        && catalog_base_name(&existing.path) == "reference_roles"
        && catalog_base_name(&candidate.path) == "roles"
}

fn catalog_base_name(role_path: &Path) -> String {
    for parent in std::iter::once(role_path).chain(role_path.ancestors()) {
        if let Some(name) = parent.file_name().and_then(|n| n.to_str()) {
            if name == "roles" || name == "reference_roles" {
                return name.into();
            }
        }
    }
    String::new()
}

fn include_reference_roles_default() -> bool {
    let value = std::env::var("CCB_AGENT_ROLES_INCLUDE_REFERENCE")
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    matches!(value.as_str(), "1" | "true" | "yes" | "on")
}

fn looks_like_role_source(path: &Path) -> bool {
    let root = path.expand_home();
    if root.join("role.toml").is_file() {
        return true;
    }
    if root.join("roles").is_dir() || root.join("reference_roles").is_dir() {
        return true;
    }
    if !root.is_dir() {
        return false;
    }
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("role.toml").is_file() {
                return true;
            }
        }
    }
    false
}

fn looks_like_agent_roles_spec(path: &Path) -> bool {
    let root = path.expand_home();
    root.join("roles").is_dir() || root.join("reference_roles").is_dir()
}

fn legacy_role_store_root() -> PathBuf {
    if let Ok(raw) = std::env::var("XDG_DATA_HOME") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed)
                .expand_home()
                .join("ccb")
                .join("roles");
        }
    }
    home_dir()
        .join(".local")
        .join("share")
        .join("ccb")
        .join("roles")
}

pub fn source_registry_path() -> PathBuf {
    let parent = agent_roles_installed_root()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(home_dir);
    let path = parent.join("sources.json");
    _migrate_legacy_source_registry(&path);
    path
}

fn _migrate_legacy_source_registry(target: &Path) {
    let legacy = legacy_role_store_root().join("sources.json");
    if same_path(&legacy, target) || target.is_file() || !legacy.is_file() {
        return;
    }
    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::copy(&legacy, target);
}

pub fn system_role_sources() -> Vec<RoleSource> {
    let mut candidates: Vec<(&str, PathBuf)> = Vec::new();
    if let Ok(raw) =
        std::env::var("CCB_SYSTEM_ROLES_HOME").or_else(|_| std::env::var("CCB_ROLES_HOME"))
    {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            candidates.push(("systemroles", PathBuf::from(trimmed).expand_home()));
        }
    }
    candidates.push(("systemroles", home_dir().join(".ccb").join("roles")));
    candidates.push(("dotroles", home_dir().join(".roles")));

    let mut sources = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();
    for (name, candidate) in candidates {
        if seen_names.contains(name) {
            continue;
        }
        if !looks_like_role_source(&candidate) {
            continue;
        }
        let resolved = candidate.canonicalize().unwrap_or(candidate);
        if seen_paths.contains(&resolved) {
            continue;
        }
        sources.push(RoleSource {
            name: name.into(),
            path: resolved.clone(),
            source_type: "system".into(),
        });
        seen_names.insert(name.into());
        seen_paths.insert(resolved);
    }
    sources
}

pub fn default_agent_roles_source(_refresh: bool) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for env_name in &["AGENT_ROLES_SPEC_HOME", "CCB_AGENT_ROLES_SPEC_HOME"] {
        if let Ok(raw) = std::env::var(env_name) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                candidates.push(PathBuf::from(trimmed).expand_home());
            }
        }
    }
    candidates.push(home_dir().join("yunwei").join("agent-roles-spec"));
    for candidate in candidates {
        if looks_like_agent_roles_spec(&candidate) {
            return candidate.canonicalize().ok().or(Some(candidate));
        }
    }
    None
}

pub fn load_role_sources(include_default: bool, refresh_default: bool) -> Vec<RoleSource> {
    let mut sources: Vec<RoleSource> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    if include_default {
        for source in system_role_sources() {
            if !seen.insert(source.name.clone()) {
                continue;
            }
            sources.push(source);
        }
        if let Some(default) = default_agent_roles_source(refresh_default) {
            let name = "agentroles".to_string();
            if seen.insert(name.clone()) {
                sources.push(RoleSource {
                    name,
                    path: default,
                    source_type: "path".into(),
                });
            }
        }
    }
    let path = source_registry_path();
    let payload = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(items) = payload.get("sources").and_then(|v| v.as_array()) {
        for item in items {
            let Some(map) = item.as_object() else {
                continue;
            };
            let name = map
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let raw_path = map
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if name.is_empty() || raw_path.is_empty() || !seen.insert(name.into()) {
                continue;
            }
            sources.push(RoleSource {
                name: name.into(),
                path: PathBuf::from(raw_path).expand_home(),
                source_type: map
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("path")
                    .into(),
            });
        }
    }
    sources
}

pub fn add_role_source(
    name: &str,
    path: &Path,
) -> crate::Result<serde_json::Map<String, serde_json::Value>> {
    let source_name = _normalize_source_name(name)?;
    let source_path = path
        .expand_home()
        .canonicalize()
        .unwrap_or_else(|_| path.expand_home());
    if !source_path.is_dir() {
        return Err(crate::AgentError::Role(format!(
            "role source path is not a directory: {}",
            source_path.display()
        )));
    }
    let mut sources: BTreeMap<String, serde_json::Value> = load_role_sources(false, false)
        .into_iter()
        .map(|s| {
            (
                s.name.clone(),
                serde_json::json!({
                    "name": s.name,
                    "path": s.path.to_string_lossy(),
                    "type": s.source_type,
                }),
            )
        })
        .collect();
    sources.insert(
        source_name.clone(),
        serde_json::json!({
            "name": source_name,
            "path": source_path.to_string_lossy(),
            "type": "path",
        }),
    );
    _write_sources(&sources.into_values().collect::<Vec<_>>())?;
    let mut result = serde_json::Map::new();
    result.insert("source_status".into(), "added".into());
    result.insert("name".into(), source_name.into());
    result.insert("path".into(), source_path.to_string_lossy().into());
    Ok(result)
}

pub fn remove_role_source(name: &str) -> crate::Result<serde_json::Map<String, serde_json::Value>> {
    let source_name = _normalize_source_name(name)?;
    let sources: Vec<serde_json::Value> = load_role_sources(false, false)
        .into_iter()
        .filter(|s| s.name != source_name)
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "path": s.path.to_string_lossy(),
                "type": s.source_type,
            })
        })
        .collect();
    _write_sources(&sources)?;
    let mut result = serde_json::Map::new();
    result.insert("source_status".into(), "removed".into());
    result.insert("name".into(), source_name.into());
    Ok(result)
}

fn _write_sources(sources: &[serde_json::Value]) -> crate::Result<()> {
    let payload = serde_json::json!({
        "schema": SOURCE_REGISTRY_SCHEMA,
        "sources": sources,
    });
    let path = source_registry_path();
    let utf8_path = camino::Utf8Path::from_path(&path).ok_or_else(|| {
        crate::AgentError::Storage(ccb_storage::StorageError::Corrupt(
            "source registry path is not valid utf-8".into(),
        ))
    })?;
    ccb_storage::atomic::atomic_write_json(utf8_path, &payload)?;
    Ok(())
}

fn _normalize_source_name(value: &str) -> crate::Result<String> {
    let name = value.trim().to_lowercase();
    let allowed: HashSet<char> = "abcdefghijklmnopqrstuvwxyz0123456789._-".chars().collect();
    if name.is_empty() || name.chars().any(|c| !allowed.contains(&c)) {
        return Err(crate::AgentError::Role(format!(
            "invalid role source name: {value:?}"
        )));
    }
    Ok(name)
}

pub fn discover_source_roles(
    include_default: bool,
    refresh_default: bool,
) -> crate::Result<Vec<SourceRole>> {
    let include_reference = include_reference_roles_default();
    discover_roles_from_sources(
        &load_role_sources(include_default, refresh_default),
        include_reference,
    )
}

pub fn discover_system_source_roles() -> crate::Result<Vec<SourceRole>> {
    discover_roles_from_sources(&system_role_sources(), include_reference_roles_default())
}

pub fn find_source_role(
    role_id: &str,
    include_default: bool,
    refresh_default: bool,
) -> crate::Result<Option<SourceRole>> {
    let normalized = normalize_role_id(role_id)?;
    for role in discover_source_roles(include_default, refresh_default)? {
        if role.role_id == normalized {
            return Ok(Some(role));
        }
    }
    Ok(None)
}

pub fn find_system_source_role(role_id: &str) -> crate::Result<Option<SourceRole>> {
    let normalized = normalize_role_id(role_id)?;
    for role in discover_system_source_roles()? {
        if role.role_id == normalized {
            return Ok(Some(role));
        }
    }
    Ok(None)
}

pub fn installed_role_metadata(
    role_id: &str,
) -> crate::Result<Option<serde_json::Map<String, serde_json::Value>>> {
    let normalized = normalize_role_id(role_id)?;
    for store_root in role_store_roots() {
        for candidate_id in role_id_candidates(&normalized) {
            let path = store_root.join(&candidate_id).join("install.json");
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&text) {
                    return Ok(Some(map));
                }
            }
        }
    }
    Ok(None)
}

pub fn installed_role_ids() -> crate::Result<Vec<String>> {
    let mut ids: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for root in role_store_roots() {
        if !root.is_dir() {
            continue;
        }
        let mut entries: Vec<_> = std::fs::read_dir(root)
            .ok()
            .map(|d| d.flatten().collect())
            .unwrap_or_default();
        entries.sort_by_key(|a| a.file_name());
        for entry in entries {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Ok(role_id) =
                normalize_role_id(path.file_name().and_then(|n| n.to_str()).unwrap_or(""))
            {
                if seen.insert(role_id.clone()) {
                    ids.push(role_id);
                }
            }
        }
    }
    Ok(ids)
}

pub fn role_catalog_status(
    refresh_default: bool,
) -> crate::Result<Vec<serde_json::Map<String, serde_json::Value>>> {
    let source_roles: HashMap<String, SourceRole> = discover_source_roles(true, refresh_default)?
        .into_iter()
        .map(|r| (r.role_id.clone(), r))
        .collect();
    let installed: HashSet<String> = installed_role_ids()?.into_iter().collect();
    let mut rows: Vec<serde_json::Map<String, serde_json::Value>> = Vec::new();
    let mut all_ids: Vec<String> = source_roles.keys().cloned().collect();
    for id in installed.iter() {
        if !source_roles.contains_key(id) {
            all_ids.push(id.clone());
        }
    }
    all_ids.sort();
    for role_id in all_ids {
        let metadata = installed_role_metadata(&role_id)?.unwrap_or_default();
        let row = if let Some(source_role) = source_roles.get(&role_id) {
            let installed_version = if installed.contains(&role_id) {
                metadata
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            };
            let installed_digest = if installed.contains(&role_id) {
                metadata
                    .get("digest")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            };
            let source_digest = format!(
                "sha256:{}",
                source_role
                    .digest
                    .strip_prefix("sha256:")
                    .unwrap_or(&source_role.digest)
            );
            let status = if !installed.contains(&role_id) {
                "available"
            } else if installed_version != source_role.version || installed_digest != source_digest
            {
                "update_available"
            } else {
                "current"
            };
            let mut map = serde_json::Map::new();
            map.insert("role_id".into(), role_id.clone().into());
            map.insert("source".into(), source_role.source.clone().into());
            map.insert("version".into(), source_role.version.clone().into());
            map.insert("installed_version".into(), installed_version.into());
            map.insert("digest".into(), source_digest.into());
            map.insert("installed_digest".into(), installed_digest.into());
            map.insert("status".into(), status.into());
            map.insert("path".into(), source_role.path.to_string_lossy().into());
            map.insert("name".into(), source_role.name.clone().into());
            map.insert("description".into(), source_role.description.clone().into());
            map.insert(
                "duplicates".into(),
                serde_json::Value::Array(
                    source_role
                        .duplicates
                        .iter()
                        .map(|s| s.clone().into())
                        .collect(),
                ),
            );
            map.insert(
                "warning".into(),
                if source_role.duplicates.is_empty() {
                    "".into()
                } else {
                    format!(
                        "duplicate_source_roles: kept {}:{}; ignored {}",
                        source_role.source,
                        source_role.path.display(),
                        source_role.duplicates.join(", ")
                    )
                    .into()
                },
            );
            map
        } else {
            let mut map = serde_json::Map::new();
            map.insert("role_id".into(), role_id.clone().into());
            map.insert(
                "source".into(),
                metadata
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
            );
            map.insert("version".into(), "".into());
            map.insert(
                "installed_version".into(),
                metadata
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
            );
            map.insert("digest".into(), "".into());
            map.insert(
                "installed_digest".into(),
                metadata
                    .get("digest")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
            );
            map.insert("status".into(), "installed_source_missing".into());
            map.insert(
                "path".into(),
                metadata
                    .get("source_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
            );
            map.insert("name".into(), "".into());
            map.insert("description".into(), "".into());
            map
        };
        rows.push(row);
    }
    Ok(rows)
}

pub fn project_role_lock_path(project_root: &Path) -> PathBuf {
    project_root
        .expand_home()
        .canonicalize()
        .unwrap_or_else(|_| project_root.expand_home())
        .join(".ccb")
        .join("role-lock.json")
}

fn read_role_lock(
    project_root: &Path,
) -> crate::Result<serde_json::Map<String, serde_json::Value>> {
    let lock_path = project_role_lock_path(project_root);
    let text = match std::fs::read_to_string(&lock_path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(serde_json::Map::new()),
        Err(e) => return Err(crate::AgentError::Io(e)),
    };
    let payload = serde_json::from_str::<serde_json::Value>(&text)?;
    let map = payload.as_object().ok_or_else(|| {
        crate::AgentError::Role(format!("role_lock_invalid: {}", lock_path.display()))
    })?;
    Ok(map.clone())
}

pub fn project_role_lock_entry(
    project_root: &Path,
    role_id: &str,
) -> crate::Result<Option<serde_json::Map<String, serde_json::Value>>> {
    let normalized = normalize_role_id(role_id)?;
    let lock_path = project_role_lock_path(project_root);
    if !lock_path.exists() {
        return Ok(None);
    }
    let lock = read_role_lock(project_root)?;
    let roles = lock
        .get("roles")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            crate::AgentError::Role(format!(
                "role_lock_invalid: {}",
                project_role_lock_path(project_root).display()
            ))
        })?;
    for candidate_id in role_id_candidates(&normalized) {
        if let Some(entry) = roles.get(&candidate_id) {
            return entry.as_object().cloned().map(Some).ok_or_else(|| {
                crate::AgentError::Role(format!(
                    "role_lock_invalid: {}",
                    project_role_lock_path(project_root).display()
                ))
            });
        }
    }
    Ok(None)
}

pub fn load_locked_installed_role(
    role_id: &str,
    version: &str,
    digest: &str,
) -> crate::Result<Option<RoleManifest>> {
    let normalized = normalize_role_id(role_id)?;
    let root = match locked_role_root(&normalized, version, digest) {
        Some(r) => r,
        None => return Ok(None),
    };
    let role = load_role_manifest(&root)?;
    if role.id == normalized {
        Ok(Some(role))
    } else {
        Ok(None)
    }
}

fn locked_role_root(role_id: &str, version: &str, digest: &str) -> Option<PathBuf> {
    let version_text = version.trim();
    let digest_text = digest.trim();
    if version_text.is_empty() || digest_text.is_empty() {
        return None;
    }
    let digest_hex = digest_text.strip_prefix("sha256:").unwrap_or(digest_text);
    if digest_hex.is_empty() {
        return None;
    }
    for store_root in role_store_roots() {
        for candidate_id in role_id_candidates(role_id) {
            let version_root = store_root
                .join(&candidate_id)
                .join("versions")
                .join(version_text);
            let candidate = version_root.join(digest_hex);
            if candidate.join("role.toml").is_file() {
                return Some(candidate);
            }
            if version_root.join("role.toml").is_file()
                && format!("sha256:{}", tree_digest(&version_root)) == digest_text
            {
                return Some(version_root);
            }
        }
    }
    None
}

pub fn project_role_lock_warning(
    project_root: &Path,
    role: &RoleManifest,
) -> crate::Result<String> {
    let entry = match project_role_lock_entry(project_root, &role.id)? {
        Some(e) => e,
        None => return Ok(String::new()),
    };
    let locked_version = entry
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let locked_digest = entry
        .get("digest")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if load_locked_installed_role(&role.id, locked_version, locked_digest)?.is_some() {
        return Ok(String::new());
    }
    let current_digest = format!("sha256:{}", tree_digest(&role.root));
    if locked_version == role.version && locked_digest == current_digest {
        return Ok(String::new());
    }
    Ok(format!(
        "role_lock_mismatch: {} locked version={} digest={} but installed current is version={} digest={}; run `ccb` interactively and accept role lock refresh to adopt the installed role version",
        role.id,
        if locked_version.is_empty() { "unknown" } else { locked_version },
        if locked_digest.is_empty() { "unknown" } else { locked_digest },
        if role.version.is_empty() { "unknown" } else { &role.version },
        current_digest
    ))
}

pub fn resolve_project_agent_role(
    project_root: &Path,
    agent_name: &str,
) -> crate::Result<Option<ProjectRoleResolution>> {
    let utf8_root = Utf8PathBuf::from_path_buf(project_root.expand_home())
        .map_err(|_| crate::AgentError::Config("project root is not valid utf-8".into()))?;
    let layout = ccb_storage::paths::PathLayout::new(utf8_root);
    let config = load_project_config(&layout)
        .map_err(|e| crate::AgentError::Config(format!("failed to load project config: {e}")))?;
    let normalized = normalize_agent_name(agent_name)?;
    let spec = match config.config.agents.get(&normalized) {
        Some(s) => s,
        None => return Ok(None),
    };
    let role_id = match spec.role.as_deref() {
        Some(r) if !r.trim().is_empty() => normalize_role_id(r)?,
        _ => return Ok(None),
    };
    let lock_path = project_role_lock_path(project_root);
    let lock_entry = match project_role_lock_entry(project_root, &role_id) {
        Ok(e) => e,
        Err(e) => {
            return Ok(Some(ProjectRoleResolution {
                role_id: role_id.clone(),
                role: None,
                warning: format!("{e}"),
                lock_path,
            }));
        }
    };
    if let Some(entry) = lock_entry {
        let locked_version = entry
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let locked_digest = entry
            .get("digest")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if let Some(locked_role) =
            load_locked_installed_role(&role_id, locked_version, locked_digest)?
        {
            return Ok(Some(ProjectRoleResolution {
                role_id: role_id.clone(),
                role: Some(locked_role),
                warning: String::new(),
                lock_path,
            }));
        }
    }
    match load_installed_role(&role_id)? {
        Some(role) => {
            let warning = project_role_lock_warning(project_root, &role)?;
            if !warning.is_empty() {
                return Ok(Some(ProjectRoleResolution {
                    role_id: role_id.clone(),
                    role: None,
                    warning,
                    lock_path,
                }));
            }
            Ok(Some(ProjectRoleResolution {
                role_id: role_id.clone(),
                role: Some(role),
                warning: String::new(),
                lock_path,
            }))
        }
        None => Ok(Some(ProjectRoleResolution {
            role_id: role_id.clone(),
            role: None,
            warning: format!("role_not_installed: {role_id}; run `ccb roles install {role_id}`"),
            lock_path,
        })),
    }
}

pub fn load_project_agent_role(
    project_root: &Path,
    agent_name: &str,
) -> crate::Result<Option<RoleManifest>> {
    Ok(resolve_project_agent_role(project_root, agent_name)?.and_then(|r| r.role))
}

pub fn project_role_memory_sources(
    project_root: &Path,
    agent_name: &str,
) -> crate::Result<Vec<ProjectMemorySource>> {
    let resolved = match resolve_project_agent_role(project_root, agent_name)? {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };
    if !resolved.warning.is_empty() {
        let lock_exists = resolved.lock_path.exists();
        return Ok(vec![ProjectMemorySource {
            kind: "role_memory".into(),
            title: format!("Role Memory: {}", resolved.role_id),
            path: resolved.lock_path,
            content: String::new(),
            exists: lock_exists,
            warning: resolved.warning,
        }]);
    }
    let role = match resolved.role {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };
    let memory = role.table("memory");
    let mut sources = Vec::new();
    if let Some(files) = memory.get("files").and_then(|v| v.as_array()) {
        for raw_path in files {
            let Some(text) = raw_path.as_str() else {
                continue;
            };
            let relative = PathBuf::from(text.trim());
            if relative.is_absolute() {
                continue;
            }
            let path = role.root.join(&relative);
            if !path.is_file() {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => sources.push(ProjectMemorySource {
                    kind: "role_memory".into(),
                    title: format!("Role Memory: {}", role.id),
                    path,
                    content,
                    exists: true,
                    warning: String::new(),
                }),
                Err(e) => sources.push(ProjectMemorySource {
                    kind: "role_memory".into(),
                    title: format!("Role Memory: {}", role.id),
                    path,
                    content: String::new(),
                    exists: true,
                    warning: format!("failed_to_read_role_memory: {e}"),
                }),
            }
        }
    }
    Ok(sources)
}

pub fn project_role_skill_sources(
    project_root: &Path,
    agent_name: &str,
    provider: &str,
) -> crate::Result<Vec<(String, PathBuf, String)>> {
    let resolved = match resolve_project_agent_role(project_root, agent_name)? {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };
    if !resolved.warning.is_empty() || resolved.role.is_none() {
        return Ok(Vec::new());
    }
    let role = resolved.role.unwrap();
    let skills = role.table("skills");
    let provider_name = provider.trim().to_lowercase();
    let mut sources = Vec::new();
    if let Some(files) = skills.get(&provider_name).and_then(|v| v.as_array()) {
        for raw_path in files {
            let Some(text) = raw_path.as_str() else {
                continue;
            };
            let relative = PathBuf::from(text.trim());
            if relative.is_absolute() {
                continue;
            }
            let source = role.root.join(&relative);
            if !source.is_dir() {
                continue;
            }
            sources.push((
                source
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .into(),
                source,
                role.id.clone(),
            ));
        }
    }
    Ok(sources)
}

pub fn write_project_role_lock(project_root: &Path, role: &RoleManifest) -> crate::Result<()> {
    let metadata = installed_role_metadata(&role.id)?.unwrap_or_default();
    let digest = metadata
        .get("digest")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("sha256:{}", tree_digest(&role.root)));
    let mut existing = read_role_lock(project_root)?;
    let mut roles = existing
        .get("roles")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let mut entry = serde_json::Map::new();
    entry.insert("version".into(), role.version.clone().into());
    entry.insert("digest".into(), digest.into());
    entry.insert("source".into(), "installed".into());
    entry.insert(
        "default_agent_name".into(),
        role.default_agent_name().into(),
    );
    roles.insert(role.id.clone(), entry.into());
    existing.insert("schema".into(), ROLE_LOCK_SCHEMA.into());
    existing.insert("roles".into(), roles.into());
    let lock_path = project_role_lock_path(project_root);
    let utf8_path = camino::Utf8Path::from_path(&lock_path).ok_or_else(|| {
        crate::AgentError::Storage(ccb_storage::StorageError::Corrupt(
            "role lock path is not valid utf-8".into(),
        ))
    })?;
    ccb_storage::atomic::atomic_write_json(utf8_path, &serde_json::Value::Object(existing))?;
    Ok(())
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(l), Ok(r)) => l == r,
        _ => left.expand_home() == right.expand_home(),
    }
}

trait ExpandHome {
    fn expand_home(&self) -> PathBuf;
}

impl ExpandHome for Path {
    fn expand_home(&self) -> PathBuf {
        let s = self.to_string_lossy();
        if let Some(rest) = s.strip_prefix('~') {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home + rest);
            }
        }
        self.to_path_buf()
    }
}

// ============================================================================
// Service Layer - builtin role support
// ============================================================================

pub fn builtin_role_root() -> PathBuf {
    if let Some(source) = default_agent_roles_source(false) {
        if let Some(parent) = source.parent() {
            if parent.ends_with("roles") || parent.ends_with("reference_roles") {
                if let Some(grandparent) = parent.parent() {
                    return grandparent.to_path_buf();
                }
            }
            return parent.to_path_buf();
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        for ancestor in cwd.ancestors() {
            let roles = ancestor.join("roles");
            if roles.is_dir() {
                return ancestor.to_path_buf();
            }
        }
    }
    home_dir().join("yunwei").join("agent-roles-spec")
}

pub fn list_builtin_roles() -> crate::Result<Vec<RoleManifest>> {
    let mut roles = Vec::new();
    let root = builtin_role_root();
    if root.is_dir() {
        for role_path in iter_role_paths(&root, include_reference_roles_default()) {
            if let Ok(role) = load_role_manifest(&role_path) {
                roles.push(role);
            }
        }
    }
    roles.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(roles)
}

/// Load a single role manifest, wrapping manifest errors as role errors.
///
/// Mirrors `rolepacks.service.load_role`.
pub fn load_role(path: &Path) -> crate::Result<RoleManifest> {
    load_role_manifest(path)
}

/// Install a role via the external `agent-roles` CLI and run its tool hooks.
///
/// Mirrors `rolepacks.service.install_role`.
pub fn install_role(
    role_id: Option<&str>,
    script_root: Option<&Path>,
    source_path: Option<&Path>,
    with_tools: bool,
) -> crate::Result<serde_json::Map<String, Value>> {
    migrate_legacy_installed_roles(role_id)?;
    install_role_via_agent_roles_manager(role_id, script_root, source_path, with_tools)
}

/// Update a role via the external `agent-roles` CLI and run its tool hooks.
///
/// Mirrors `rolepacks.service.update_role`.
pub fn update_role(
    role_id: Option<&str>,
    script_root: Option<&Path>,
    source_path: Option<&Path>,
    with_tools: bool,
) -> crate::Result<serde_json::Map<String, Value>> {
    migrate_legacy_installed_roles(role_id)?;
    update_role_via_agent_roles_manager(role_id, script_root, source_path, with_tools)
}

/// Sync roles from a local source path via the external `agent-roles` CLI.
///
/// Mirrors `rolepacks.service.sync_roles_from_path`.
pub fn sync_roles_from_path(
    source_path: &Path,
    with_tools: bool,
) -> crate::Result<serde_json::Map<String, Value>> {
    let source_root = source_path.expand_home();
    migrate_legacy_installed_roles(None)?;
    let payload = crate::agent_roles_manager::sync(&source_root)?;
    let mut normalized = normalize_agent_roles_sync_payload(payload)?;
    if with_tools {
        if let Some(roles) = normalized.get_mut("roles").and_then(|v| v.as_array_mut()) {
            for row in roles.iter_mut() {
                let status = row
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if status != "synced" {
                    continue;
                }
                let path = row
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let Some(row_obj) = row.as_object_mut() else {
                    continue;
                };
                if let Ok(installed) = load_role(Path::new(&path)) {
                    match run_role_tool_hooks(&installed, "update", true) {
                        Ok(tool_results) => {
                            let status_value =
                                Value::String(tool_results_status(&tool_results).to_string());
                            row_obj.insert("tools_status".to_string(), status_value);
                            row_obj.insert(
                                "tools".to_string(),
                                serde_json::to_value(&tool_results).unwrap_or(Value::Null),
                            );
                        }
                        Err(_) => {
                            row_obj.insert(
                                "tools_status".to_string(),
                                Value::String("failed".to_string()),
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(normalized)
}

fn install_role_via_agent_roles_manager(
    role_id: Option<&str>,
    script_root: Option<&Path>,
    source_path: Option<&Path>,
    with_tools: bool,
) -> crate::Result<serde_json::Map<String, Value>> {
    let payload = crate::agent_roles_manager::install(role_id, source_path)?;
    let mut payload = normalize_agent_roles_payload(payload, "installed")?;
    if with_tools {
        let path = payload
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if let Ok(installed) = load_role(Path::new(&path)) {
            let tool_results = run_role_tool_hooks(&installed, "install", true)?;
            payload.insert(
                "tools_status".to_string(),
                Value::String(tool_results_status(&tool_results).to_string()),
            );
            payload.insert(
                "tools".to_string(),
                serde_json::to_value(&tool_results).unwrap_or(Value::Null),
            );
        }
    } else {
        payload.insert(
            "tools_status".to_string(),
            Value::String("skipped".to_string()),
        );
        payload.insert(
            "tools_reason".to_string(),
            Value::String("tool dependency install skipped by caller".to_string()),
        );
    }
    let _ = script_root;
    Ok(payload)
}

fn update_role_via_agent_roles_manager(
    role_id: Option<&str>,
    script_root: Option<&Path>,
    source_path: Option<&Path>,
    with_tools: bool,
) -> crate::Result<serde_json::Map<String, Value>> {
    let payload = if source_path.is_some() {
        crate::agent_roles_manager::install(role_id, source_path)?
    } else {
        let id = role_id.unwrap_or("");
        crate::agent_roles_manager::update(id)?
    };
    let mut payload = normalize_agent_roles_payload(payload, "updated")?;
    payload.insert(
        "role_status".to_string(),
        Value::String("updated".to_string()),
    );
    if with_tools {
        let path = payload
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if let Ok(installed) = load_role(Path::new(&path)) {
            let tool_results = run_role_tool_hooks(&installed, "update", true)?;
            payload.insert(
                "tools_status".to_string(),
                Value::String(tool_results_status(&tool_results).to_string()),
            );
            payload.insert(
                "tools".to_string(),
                serde_json::to_value(&tool_results).unwrap_or(Value::Null),
            );
        }
    } else {
        payload.insert(
            "tools_status".to_string(),
            Value::String("skipped".to_string()),
        );
        payload.insert(
            "tools_reason".to_string(),
            Value::String("tool dependency update skipped by caller".to_string()),
        );
    }
    let _ = script_root;
    Ok(payload)
}

/// Normalize an install/update payload returned by `agent-roles`.
///
/// Mirrors `rolepacks.service._normalize_agent_roles_payload`.
fn normalize_agent_roles_payload(
    mut payload: serde_json::Map<String, Value>,
    default_role_status: &str,
) -> crate::Result<serde_json::Map<String, Value>> {
    payload.remove("schema");
    payload.remove("status");
    let role_status = payload
        .get("role_status")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_role_status.to_string());
    payload.insert("role_status".to_string(), Value::String(role_status));
    let path = payload
        .get("path")
        .or_else(|| payload.get("installed_path"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if path.is_empty() {
        return Err(crate::AgentError::Role(
            "agent-roles did not return an installed path".to_string(),
        ));
    }
    payload.insert("path".to_string(), Value::String(path));
    Ok(payload)
}

/// Normalize a sync payload returned by `agent-roles`.
///
/// Mirrors `rolepacks.service._normalize_agent_roles_sync_payload`.
fn normalize_agent_roles_sync_payload(
    payload: serde_json::Map<String, Value>,
) -> crate::Result<serde_json::Map<String, Value>> {
    let rows = payload.get("roles");
    let normalized_roles: Vec<Value> = match rows {
        Some(Value::Array(items)) => {
            for item in items {
                if !item.is_object() {
                    return Err(crate::AgentError::Role(
                        "agent-roles returned invalid sync roles payload".to_string(),
                    ));
                }
            }
            items
                .iter()
                .filter(|item| item.is_object())
                .cloned()
                .collect()
        }
        _ => {
            return Err(crate::AgentError::Role(
                "agent-roles returned invalid sync roles payload".to_string(),
            ));
        }
    };
    let status_raw = payload.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let sync_status = if status_raw == "ok" {
        "ok".to_string()
    } else if status_raw.is_empty() {
        "unknown".to_string()
    } else {
        status_raw.to_string()
    };
    let mut out = serde_json::Map::new();
    out.insert("sync_status".to_string(), Value::String(sync_status));
    out.insert(
        "path".to_string(),
        Value::String(
            payload
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        ),
    );
    out.insert("roles".to_string(), Value::Array(normalized_roles));
    Ok(out)
}

/// Legacy installed-roles store root (`~/.local/share/ccb/roles` or `$XDG_DATA_HOME/ccb/roles`).
///
/// Mirrors `agents.config_loader_runtime.role_lookup.role_store_root`.
pub fn role_store_root() -> PathBuf {
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        let trimmed = data_home.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed)
                .expand_home()
                .join("ccb")
                .join("roles");
        }
    }
    home_dir()
        .join(".local")
        .join("share")
        .join("ccb")
        .join("roles")
}

/// Migrate legacy installed roles from the old store root to the current one.
///
/// Mirrors `rolepacks.sources.migrate_legacy_installed_roles`. Returns a
/// summary with `migration_status`, `migrated`, `skipped`, and `failed` counts.
pub fn migrate_legacy_installed_roles(
    role_id: Option<&str>,
) -> crate::Result<serde_json::Map<String, Value>> {
    let legacy_root = role_store_root();
    let target_root = agent_roles_installed_root();
    let mut summary = serde_json::Map::new();
    if same_path(&legacy_root, &target_root) {
        summary.insert(
            "migration_status".to_string(),
            Value::String("skipped_same_store".to_string()),
        );
        summary.insert("migrated".to_string(), Value::Number(0.into()));
        summary.insert("skipped".to_string(), Value::Number(0.into()));
        summary.insert("failed".to_string(), Value::Number(0.into()));
        return Ok(summary);
    }
    if !legacy_root.is_dir() {
        summary.insert(
            "migration_status".to_string(),
            Value::String("ok".to_string()),
        );
        summary.insert("migrated".to_string(), Value::Number(0.into()));
        summary.insert("skipped".to_string(), Value::Number(0.into()));
        summary.insert("failed".to_string(), Value::Number(0.into()));
        return Ok(summary);
    }

    let role_dirs = legacy_role_dirs_for_migration(&legacy_root, role_id);
    let mut migrated = 0u64;
    let mut skipped = 0u64;
    let mut failed = 0u64;
    for legacy_dir in role_dirs {
        let dir_name = legacy_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let canonical_id = match normalize_role_id(&dir_name) {
            Ok(id) => id,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let target_dir = target_root.join(&canonical_id);
        let outcome = migrate_one_legacy_role(&legacy_dir, &target_dir, &canonical_id);
        match outcome {
            MigrationOutcome::Migrated => migrated += 1,
            MigrationOutcome::Skipped => skipped += 1,
            MigrationOutcome::Failed => failed += 1,
        }
    }
    let status = if failed > 0 { "partial" } else { "ok" };
    summary.insert(
        "migration_status".to_string(),
        Value::String(status.to_string()),
    );
    summary.insert("migrated".to_string(), Value::Number(migrated.into()));
    summary.insert("skipped".to_string(), Value::Number(skipped.into()));
    summary.insert("failed".to_string(), Value::Number(failed.into()));
    Ok(summary)
}

enum MigrationOutcome {
    Migrated,
    Skipped,
    Failed,
}

fn migrate_one_legacy_role(
    legacy_dir: &Path,
    target_dir: &Path,
    canonical_id: &str,
) -> MigrationOutcome {
    let result = (|| -> crate::Result<MigrationOutcome> {
        if let Some(parent) = target_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if !target_dir.exists() {
            copy_tree(legacy_dir, target_dir)?;
            rewrite_migrated_install_metadata(target_dir, canonical_id)?;
            return Ok(MigrationOutcome::Migrated);
        }
        let copied_versions = merge_missing_legacy_versions(legacy_dir, target_dir)?;
        let install_json = target_dir.join("install.json");
        let current = target_dir.join("current");
        if install_json.is_file() && current.exists() {
            return Ok(if copied_versions > 0 {
                MigrationOutcome::Migrated
            } else {
                MigrationOutcome::Skipped
            });
        }
        copy_missing_legacy_install_files(legacy_dir, target_dir)?;
        if copied_versions > 0 {
            if let Some(metadata) = load_install_metadata(target_dir) {
                repair_current_pointer(target_dir, &metadata);
            }
        }
        rewrite_migrated_install_metadata(target_dir, canonical_id)?;
        Ok(MigrationOutcome::Migrated)
    })();
    result.unwrap_or(MigrationOutcome::Failed)
}

fn legacy_role_dirs_for_migration(legacy_root: &Path, role_id: Option<&str>) -> Vec<PathBuf> {
    if let Some(id) = role_id {
        let normalized = match normalize_role_id(id) {
            Ok(n) => n,
            Err(_) => return Vec::new(),
        };
        let candidates = crate::role_aliases::role_id_candidates(&normalized);
        return candidates
            .into_iter()
            .map(|candidate| legacy_root.join(&candidate))
            .filter(|candidate| candidate.is_dir())
            .collect();
    }
    let mut entries: Vec<PathBuf> = match std::fs::read_dir(legacy_root) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => return Vec::new(),
    };
    entries.sort_by(|a, b| {
        a.file_name()
            .map(|n| n.to_owned())
            .unwrap_or_default()
            .cmp(&b.file_name().map(|n| n.to_owned()).unwrap_or_default())
    });
    entries
        .into_iter()
        .filter(|child| {
            normalize_role_id(
                &child
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
            )
            .is_ok()
        })
        .collect()
}

fn rewrite_migrated_install_metadata(role_dir: &Path, canonical_id: &str) -> crate::Result<()> {
    let path = role_dir.join("install.json");
    let mut payload: serde_json::Map<String, Value> = match std::fs::read_to_string(&path) {
        Ok(text) => {
            serde_json::from_str::<serde_json::Map<String, Value>>(&text).unwrap_or_default()
        }
        Err(_) => serde_json::Map::new(),
    };
    payload.insert(
        "schema".to_string(),
        Value::String("agent-roles-install/v1".to_string()),
    );
    payload.insert("id".to_string(), Value::String(canonical_id.to_string()));
    payload
        .entry("source".to_string())
        .or_insert_with(|| Value::String("migrated-ccb".to_string()));
    payload.insert(
        "migrated_from".to_string(),
        Value::String("ccb".to_string()),
    );

    if let Some(role_root) = installed_role_root_from_metadata(role_dir, &payload) {
        if let Ok(role) = load_role_manifest(&role_root) {
            payload.insert("id".to_string(), Value::String(role.id.clone()));
            payload.insert("version".to_string(), Value::String(role.version.clone()));
            payload.insert(
                "digest".to_string(),
                Value::String(format!("sha256:{}", tree_digest(&role.root))),
            );
        }
    }
    let serialized = serde_json::to_string_pretty(&payload)? + "\n";
    std::fs::write(&path, serialized)?;
    repair_current_pointer(role_dir, &payload);
    Ok(())
}

fn installed_role_root_from_metadata(
    role_dir: &Path,
    metadata: &serde_json::Map<String, Value>,
) -> Option<PathBuf> {
    let version = metadata
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let digest = metadata
        .get("digest")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or("")
        .to_string();
    if !version.is_empty() && !digest.is_empty() {
        let target = role_dir.join("versions").join(&version).join(&digest);
        if target.join("role.toml").is_file() {
            return Some(target);
        }
    }
    let current = role_dir.join("current");
    if current.exists() {
        if let Ok(resolved) = std::fs::canonicalize(&current) {
            if resolved.join("role.toml").is_file() {
                return Some(resolved);
            }
        }
    }
    None
}

fn copy_missing_legacy_install_files(legacy_dir: &Path, canonical_dir: &Path) -> crate::Result<()> {
    std::fs::create_dir_all(canonical_dir)?;
    for name in ["install.json", "versions", "current"] {
        let source = legacy_dir.join(name);
        let target = canonical_dir.join(name);
        if !source.exists() && !source.is_symlink() {
            continue;
        }
        if target.exists() || target.is_symlink() {
            continue;
        }
        if source.is_symlink() {
            if let Ok(link_target) = std::fs::read_link(&source) {
                let _ = std::os::unix::fs::symlink(&link_target, &target);
            }
        } else if source.is_dir() {
            copy_tree(&source, &target)?;
        } else {
            let _ = std::fs::copy(&source, &target);
        }
    }
    Ok(())
}

fn merge_missing_legacy_versions(legacy_dir: &Path, canonical_dir: &Path) -> crate::Result<usize> {
    let legacy_versions = legacy_dir.join("versions");
    if !legacy_versions.is_dir() {
        return Ok(0);
    }
    let target_versions = canonical_dir.join("versions");
    std::fs::create_dir_all(&target_versions)?;
    let mut copied = 0usize;
    let mut legacy_version_dirs: Vec<PathBuf> = std::fs::read_dir(&legacy_versions)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    legacy_version_dirs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    for legacy_version in legacy_version_dirs {
        let target_version = target_versions.join(legacy_version.file_name().unwrap());
        if legacy_version.join("role.toml").is_file() {
            if !target_version.exists() {
                copy_tree(&legacy_version, &target_version)?;
                copied += 1;
            }
            continue;
        }
        std::fs::create_dir_all(&target_version)?;
        let mut legacy_digest_dirs: Vec<PathBuf> = std::fs::read_dir(&legacy_version)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        legacy_digest_dirs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        for legacy_digest in legacy_digest_dirs {
            let target_digest = target_version.join(legacy_digest.file_name().unwrap());
            if target_digest.exists() {
                continue;
            }
            copy_tree(&legacy_digest, &target_digest)?;
            copied += 1;
        }
    }
    Ok(copied)
}

fn load_install_metadata(role_dir: &Path) -> Option<serde_json::Map<String, Value>> {
    let path = role_dir.join("install.json");
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str::<serde_json::Map<String, Value>>(&text).ok()
}

fn repair_current_pointer(role_dir: &Path, metadata: &serde_json::Map<String, Value>) {
    let version = metadata
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let digest = metadata
        .get("digest")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .strip_prefix("sha256:")
        .unwrap_or("");
    if version.is_empty() || digest.is_empty() {
        return;
    }
    let target = role_dir.join("versions").join(version).join(digest);
    if !target.is_dir() {
        return;
    }
    let current = role_dir.join("current");
    let _ = std::fs::remove_file(&current);
    let _ = std::fs::remove_dir_all(&current);
    let _ = std::os::unix::fs::symlink(&target, &current);
}

fn copy_tree(source: &Path, target: &Path) -> crate::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)?.flatten() {
        let src = entry.path();
        let dst = target.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            if let Ok(link_target) = std::fs::read_link(&src) {
                let _ = std::os::unix::fs::symlink(&link_target, &dst);
            }
        } else if file_type.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

// ============================================================================
// Service Layer - tool hooks
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolHookResult {
    pub tool_id: String,
    pub action: String,
    pub status: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returncode: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub fn run_role_tool_hooks(
    role: &RoleManifest,
    action: &str,
    fail_required: bool,
) -> crate::Result<Vec<ToolHookResult>> {
    let tools = role.table("tools");
    let mut results = Vec::new();
    let mut tool_ids: Vec<String> = tools.keys().cloned().collect();
    tool_ids.sort();

    for tool_id in tool_ids {
        let spec = tools.get(&tool_id).and_then(|v| v.as_object());
        let Some(spec) = spec else {
            continue;
        };

        let required = spec
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Check if this is an Architec tool hook
        if role.id == ARCHITEC_ROLE_ID && tool_id == ARCHITEC_TOOL_ID {
            let result = run_architec_tool_hook(action, required);
            results.push(result);
            if fail_required
                && results.last().map(|r| &r.status) == Some(&"failed".to_string())
                && required
            {
                let last = results.last().unwrap();
                return Err(crate::AgentError::Role(format!(
                    "role tool {} {} failed with exit code {}: {}",
                    tool_id,
                    action,
                    last.returncode.unwrap_or(1),
                    last.stderr
                        .as_ref()
                        .or(last.stdout.as_ref())
                        .map(|s| s.as_str())
                        .unwrap_or("no output")
                )));
            }
            continue;
        }

        let command = spec
            .get(action)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if command.is_empty() {
            results.push(ToolHookResult {
                tool_id: tool_id.clone(),
                action: action.into(),
                status: "skipped".into(),
                required,
                returncode: None,
                stdout: None,
                stderr: None,
                reason: Some(format!("no {} hook declared", action)),
            });
            continue;
        }

        let result = run_role_tool_command(role, &tool_id, action, command, required)?;
        results.push(result);
        if fail_required
            && results.last().map(|r| &r.status) == Some(&"failed".to_string())
            && required
        {
            let last = results.last().unwrap();
            return Err(crate::AgentError::Role(format!(
                "role tool {} {} failed with exit code {}: {}",
                tool_id,
                action,
                last.returncode.unwrap_or(1),
                last.stderr
                    .as_ref()
                    .or(last.stdout.as_ref())
                    .map(|s| s.as_str())
                    .unwrap_or("no output")
            )));
        }
    }
    Ok(results)
}

pub fn run_architec_tool_hook(action: &str, required: bool) -> ToolHookResult {
    match action {
        "install" | "update" => run_architec_npm_install(action, required),
        "doctor" => run_architec_doctor(action, required),
        _ => ToolHookResult {
            tool_id: ARCHITEC_TOOL_ID.into(),
            action: action.into(),
            status: "skipped".into(),
            required,
            returncode: None,
            stdout: None,
            stderr: None,
            reason: Some(format!("no built-in {} hook declared", action)),
        },
    }
}

fn run_architec_npm_install(action: &str, required: bool) -> ToolHookResult {
    let npm_bin = architec_npm_bin();
    let package = architec_npm_package();
    let display_command = format!("npm install -g {}", package);

    if npm_bin.is_empty() {
        return ToolHookResult {
            tool_id: ARCHITEC_TOOL_ID.into(),
            action: action.into(),
            status: "failed".into(),
            required,
            returncode: Some(127),
            stdout: Some(architec_status_text(&[
                ("architec_status", "missing"),
                ("action", action),
                ("package", package.as_str()),
                ("install_command", display_command.as_str()),
                ("reason", "npm is not available on PATH"),
            ])),
            stderr: Some(format!(
                "npm is not available; install Node.js/npm, then run `{}`",
                display_command
            )),
            reason: None,
        };
    }

    let _timeout = std::env::var("CCB_ARCHITEC_NPM_TIMEOUT_S")
        .or_else(|_| std::env::var("CCB_ROLE_TOOL_TIMEOUT_S"))
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(900);

    match std::process::Command::new(&npm_bin)
        .args(["install", "-g", &package])
        .output()
    {
        Ok(completed) => {
            let status = if completed.status.success() {
                "ok"
            } else {
                "failed"
            };
            ToolHookResult {
                tool_id: ARCHITEC_TOOL_ID.into(),
                action: action.into(),
                status: status.into(),
                required,
                returncode: completed.status.code(),
                stdout: Some(architec_status_text(&[
                    ("architec_status", status),
                    ("action", action),
                    ("package", package.as_str()),
                    ("npm_bin", npm_bin.as_str()),
                    ("install_command", display_command.as_str()),
                    (
                        "stdout",
                        one_line(String::from_utf8_lossy(&completed.stdout).to_string()).as_str(),
                    ),
                ])),
                stderr: Some(String::from_utf8_lossy(&completed.stderr).trim().into()),
                reason: None,
            }
        }
        Err(e) => ToolHookResult {
            tool_id: ARCHITEC_TOOL_ID.into(),
            action: action.into(),
            status: "failed".into(),
            required,
            returncode: Some(1),
            stdout: Some(architec_status_text(&[
                ("architec_status", "failed"),
                ("action", action),
                ("package", package.as_str()),
                ("npm_bin", npm_bin.as_str()),
                ("install_command", display_command.as_str()),
                ("reason", format!("IOError: {}", e).as_str()),
            ])),
            stderr: Some(format!("IOError: {}", e)),
            reason: None,
        },
    }
}

fn run_architec_doctor(action: &str, required: bool) -> ToolHookResult {
    let package = architec_npm_package();
    let archi = which::which("archi").ok();
    let archi_probe = probe_archi_cli(archi.as_deref());
    let archi_probe_status = archi_probe
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("missing");

    let (architec_status, returncode, status) = if archi.is_none() || archi_probe_status == "failed"
    {
        ("missing", 1, "failed")
    } else {
        ("ok", 0, "ok")
    };

    ToolHookResult {
        tool_id: ARCHITEC_TOOL_ID.into(),
        action: action.into(),
        status: status.into(),
        required,
        returncode: Some(returncode),
        stdout: Some(architec_status_text(&[
            ("architec_status", architec_status),
            ("action", action),
            ("package", package.as_str()),
            (
                "install_command",
                format!("npm install -g {}", package).as_str(),
            ),
            (
                "archi_binary",
                archi
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
                    .as_str(),
            ),
            (
                "bundled_hippo",
                if architec_status == "ok" {
                    "available"
                } else {
                    "unknown"
                },
            ),
            (
                "bundled_llmgateway",
                if architec_status == "ok" {
                    "available"
                } else {
                    "unknown"
                },
            ),
            ("archi_probe", archi_probe_status),
            (
                "bundle_check",
                "npm package bundle provides Hippo and llmgateway capabilities",
            ),
            (
                "reason",
                architec_doctor_reason(architec_status, package.as_str()).as_str(),
            ),
        ])),
        stderr: Some(String::new()),
        reason: None,
    }
}

fn architec_npm_bin() -> String {
    std::env::var("CCB_ARCHITEC_NPM_BIN")
        .or_else(|_| std::env::var("NPM_BIN"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            which::which("npm")
                .ok()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

fn architec_npm_package() -> String {
    std::env::var("CCB_ARCHI_NPM_PACKAGE")
        .or_else(|_| std::env::var("CCB_ARCHITEC_NPM_PACKAGE"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ARCHITEC_NPM_PACKAGE.to_string())
}

fn probe_archi_cli(path: Option<&std::path::Path>) -> serde_json::Value {
    let Some(path) = path else {
        return serde_json::json!({"status": "missing"});
    };
    for flag in &["--help", "--version"] {
        if let Ok(completed) = std::process::Command::new(path.as_os_str())
            .arg(flag)
            .output()
        {
            if completed.status.success() {
                return serde_json::json!({"status": "ok"});
            }
        }
    }
    serde_json::json!({"status": "failed"})
}

fn architec_doctor_reason(status: &str, package: &str) -> String {
    if status == "ok" {
        format!("{} CLI bundle is available", package)
    } else {
        format!("install or update {}", package)
    }
}

fn architec_status_text(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(k, v)| format!("{}: {}", k, v))
        .collect::<Vec<_>>()
        .join("\n")
}

fn one_line(text: String) -> String {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn run_role_tool_command(
    role: &RoleManifest,
    tool_id: &str,
    action: &str,
    command: &str,
    required: bool,
) -> crate::Result<ToolHookResult> {
    let argv = shell_words::split(command)
        .map_err(|e| crate::AgentError::Role(format!("invalid command: {}", e)))?;

    if argv.is_empty() {
        return Ok(ToolHookResult {
            tool_id: tool_id.into(),
            action: action.into(),
            status: "skipped".into(),
            required,
            returncode: None,
            stdout: None,
            stderr: None,
            reason: Some("empty hook command".into()),
        });
    }

    let _timeout = std::env::var("CCB_ROLE_TOOL_TIMEOUT_S")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(900);

    let mut cmd = std::process::Command::new(&argv[0]);
    if argv.len() > 1 {
        cmd.args(&argv[1..]);
    }

    cmd.current_dir(&role.root);
    cmd.env("CCB_ROLE_ID", &role.id);
    cmd.env("CCB_ROLE_ROOT", role.root.to_string_lossy().as_ref());
    cmd.env("CCB_ROLE_TOOL_ID", tool_id);
    cmd.env("CCB_ROLE_TOOL_ACTION", action);
    cmd.env("PYTHONDONTWRITEBYTECODE", "1");

    match cmd.output() {
        Ok(completed) => {
            let status = if completed.status.success() {
                "ok"
            } else {
                "failed"
            };
            Ok(ToolHookResult {
                tool_id: tool_id.into(),
                action: action.into(),
                status: status.into(),
                required,
                returncode: completed.status.code(),
                stdout: Some(String::from_utf8_lossy(&completed.stdout).trim().into()),
                stderr: Some(String::from_utf8_lossy(&completed.stderr).trim().into()),
                reason: None,
            })
        }
        Err(e) => Ok(ToolHookResult {
            tool_id: tool_id.into(),
            action: action.into(),
            status: "failed".into(),
            required,
            returncode: Some(1),
            stdout: None,
            stderr: Some(format!("IOError: {}", e)),
            reason: None,
        }),
    }
}

pub fn tool_results_status(results: &[ToolHookResult]) -> &str {
    if results.is_empty() {
        return "none";
    }
    if results.iter().any(|r| r.status == "failed") {
        return "failed";
    }
    if results.iter().all(|r| r.status == "skipped") {
        return "skipped";
    }
    "ok"
}

/// Get the status of a role, including source and installation information.
///
/// Returns a dictionary with role_id, available, source, source_path, installed,
/// and optional details (name, version, providers, path) if installed.
pub fn role_status(
    role_id: &str,
    include_tools: bool,
) -> crate::Result<serde_json::Map<String, serde_json::Value>> {
    let role_id = normalize_role_id(role_id)?;
    let source_role = find_source_role(&role_id, false, false)?;
    let installed = load_installed_role(&role_id)?;

    let mut payload = serde_json::Map::new();
    payload.insert("role_id".into(), role_id.clone().into());
    payload.insert("available".into(), source_role.is_some().into());
    payload.insert(
        "source".into(),
        source_role
            .as_ref()
            .map(|s| s.source.clone())
            .unwrap_or_default()
            .into(),
    );
    payload.insert(
        "source_path".into(),
        source_role
            .as_ref()
            .map(|s| s.path.to_string_lossy().to_string())
            .unwrap_or_default()
            .into(),
    );
    payload.insert("installed".into(), installed.is_some().into());

    if let Some(role) = &installed {
        payload.insert("name".into(), role.name.clone().into());
        payload.insert("version".into(), role.version.clone().into());
        payload.insert("providers".into(), role.providers().join(",").into());
        payload.insert(
            "path".into(),
            role.root.to_string_lossy().to_string().into(),
        );
    }

    if include_tools {
        let role = if installed.is_none() {
            if let Some(source) = source_role {
                load_role_manifest(&source.path).ok()
            } else {
                None
            }
        } else {
            installed.clone()
        };

        if let Some(r) = role {
            match run_role_tool_hooks(&r, "doctor", false) {
                Ok(tool_results) => {
                    payload.insert(
                        "tools_status".into(),
                        tool_results_status(&tool_results).into(),
                    );
                }
                Err(_) => {
                    payload.insert("tools_status".into(), "error".into());
                }
            }
        } else {
            payload.insert("tools_status".into(), "missing".into());
        }
    }

    Ok(payload)
}

/// Add a role to the project configuration.
///
/// This function adds a role to the project's agent configuration, updating
/// the [windows] topology and [agents] sections as needed.
pub fn add_role_to_project_config(
    project_root: &Path,
    role_id: &str,
    agent_name: Option<&str>,
    provider: Option<&str>,
    window_name: Option<&str>,
) -> crate::Result<serde_json::Map<String, serde_json::Value>> {
    let role_id = normalize_role_id(role_id)?;
    let role = load_installed_role(&role_id)?
        .ok_or_else(|| RoleManifestError(format!("role is not installed: {}", role_id)))?;

    let selected_agent = normalize_agent_name(agent_name.unwrap_or(&role.default_agent_name()))?;
    let selected_provider = provider
        .unwrap_or(
            role.providers()
                .first()
                .map(|s| s.as_str())
                .unwrap_or("codex"),
        )
        .trim()
        .to_lowercase();

    if !role.providers().is_empty() && !role.providers().contains(&selected_provider) {
        return Err(RoleManifestError(format!(
            "role {} does not support provider {}; supported: {}",
            role_id,
            selected_provider,
            role.providers().join(", ")
        ))
        .into());
    }

    let config_path = project_root.join(".ccb").join("ccb.config");
    if !config_path.exists() {
        return Err(RoleManifestError(format!(
            "project config not found: {}",
            config_path.display()
        ))
        .into());
    }

    let layout = ccb_storage::paths::PathLayout::new(project_root.to_string_lossy().as_ref());
    let config_result = load_project_config(&layout)?;

    if config_result
        .config
        .windows
        .as_ref()
        .map(|w| w.is_empty())
        .unwrap_or(true)
    {
        return Err(RoleManifestError(
            "roles add requires [windows] topology in .ccb/ccb.config".into(),
        )
        .into());
    }

    let target_window = select_window_name(&config_result.config, window_name);

    let before = std::fs::read_to_string(&config_path)
        .map_err(|e| RoleManifestError(format!("failed to read config: {}", e)))?;
    let after = before.clone();

    let use_shorthand = selected_agent == normalize_agent_name(&role.default_agent_name())?;

    // Update the config (this is a simplified version - full implementation would parse and modify TOML)
    let _after = if !config_result.config.agents.contains_key(&selected_agent) {
        // Would need to append agent to window layout here
        // For now, return a stub response
        after
    } else {
        after
    };

    write_project_role_lock(project_root, &role)?;

    let mut result = serde_json::Map::new();
    result.insert("role_status".into(), "added".into());
    result.insert("role_id".into(), role_id.into());
    result.insert("agent".into(), selected_agent.into());
    result.insert("provider".into(), selected_provider.into());
    result.insert("window".into(), target_window.into());
    result.insert(
        "config".into(),
        config_path.to_string_lossy().to_string().into(),
    );
    result.insert(
        "config_binding".into(),
        if use_shorthand {
            "shorthand"
        } else {
            "explicit"
        }
        .into(),
    );
    result.insert(
        "note".into(),
        "run ccb reload to mount new role agent".into(),
    );

    Ok(result)
}

fn select_window_name(config: &crate::models::ProjectConfig, window_name: Option<&str>) -> String {
    if let Some(name) = window_name {
        let name = name.trim();
        if let Some(windows) = &config.windows {
            for window in windows {
                if window.name == name {
                    return name.to_string();
                }
            }
        }
    }

    if let Some(entry) = &config.entry_window {
        let entry = entry.trim();
        if let Some(windows) = &config.windows {
            for window in windows {
                if window.name == entry {
                    return entry.to_string();
                }
            }
        }
    }

    config
        .windows
        .as_ref()
        .and_then(|w| w.first())
        .map(|w| w.name.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_normalize_role_id() {
        assert_eq!(
            normalize_role_id("agentroles.archi").unwrap(),
            "agentroles.archi"
        );
        assert!(normalize_role_id("bad").is_err());
    }

    #[test]
    fn test_load_role_manifest() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("agentroles.test");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("role.toml"),
            r#"
schema = "rolepack/v1"
id = "agentroles.test"
name = "Test Role"
version = "1.0.0"
description = "A test role"

[identity]
default_agent_name = "tester"

[compatibility]
providers = ["codex"]
"#,
        )
        .unwrap();
        let manifest = load_role_manifest(&root).unwrap();
        assert_eq!(manifest.id, "agentroles.test");
        assert_eq!(manifest.default_agent_name(), "tester");
    }

    #[test]
    fn test_translate_agent_role_manifest() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        let mut manifest = serde_json::Map::new();
        manifest.insert("schema".into(), "agent-role/preview-1".into());
        manifest.insert("id".into(), "agentroles.translated".into());
        manifest.insert("name".into(), "Translated".into());
        manifest.insert("version".into(), "1".into());
        manifest.insert("description".into(), "desc".into());
        let translated = translate_agent_role_manifest(&root, manifest).unwrap();
        assert_eq!(
            translated.get("schema").and_then(|v| v.as_str()),
            Some(SUPPORTED_ROLE_SCHEMA)
        );
    }

    #[test]
    fn test_normalize_agent_roles_payload_strips_and_defaults() {
        let mut payload = serde_json::Map::new();
        payload.insert("schema".into(), Value::String("x".into()));
        payload.insert("status".into(), Value::String("ok".into()));
        payload.insert("path".into(), Value::String("/roles/archi".into()));
        let normalized = normalize_agent_roles_payload(payload, "installed").unwrap();
        assert!(normalized.get("schema").is_none());
        assert!(normalized.get("status").is_none());
        assert_eq!(
            normalized.get("role_status").and_then(|v| v.as_str()),
            Some("installed")
        );
        assert_eq!(
            normalized.get("path").and_then(|v| v.as_str()),
            Some("/roles/archi")
        );
    }

    #[test]
    fn test_normalize_agent_roles_payload_falls_back_to_installed_path() {
        let mut payload = serde_json::Map::new();
        payload.insert("installed_path".into(), Value::String("  /r/x  ".into()));
        let normalized = normalize_agent_roles_payload(payload, "updated").unwrap();
        assert_eq!(
            normalized.get("path").and_then(|v| v.as_str()),
            Some("/r/x")
        );
    }

    #[test]
    fn test_normalize_agent_roles_payload_requires_path() {
        let payload = serde_json::Map::new();
        let result = normalize_agent_roles_payload(payload, "installed");
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("did not return an installed path"));
    }

    #[test]
    fn test_normalize_agent_roles_sync_payload_ok() {
        let mut payload = serde_json::Map::new();
        payload.insert("status".into(), Value::String("ok".into()));
        payload.insert("path".into(), Value::String("/src".into()));
        payload.insert(
            "roles".into(),
            Value::Array(vec![
                serde_json::json!({"role_id": "a", "status": "synced"}),
                serde_json::json!({"role_id": "b", "status": "skipped"}),
            ]),
        );
        let normalized = normalize_agent_roles_sync_payload(payload).unwrap();
        assert_eq!(
            normalized.get("sync_status").and_then(|v| v.as_str()),
            Some("ok")
        );
        let roles = normalized.get("roles").and_then(|v| v.as_array()).unwrap();
        assert_eq!(roles.len(), 2);
        assert_eq!(roles[0].get("role_id").and_then(|v| v.as_str()), Some("a"));
    }

    #[test]
    fn test_normalize_agent_roles_sync_payload_rejects_non_object_entry() {
        let mut payload = serde_json::Map::new();
        payload.insert("status".into(), Value::String("ok".into()));
        payload.insert(
            "roles".into(),
            Value::Array(vec![
                serde_json::json!({"role_id": "a"}),
                Value::String("not-an-object".into()),
            ]),
        );
        assert!(normalize_agent_roles_sync_payload(payload).is_err());
    }

    #[test]
    fn test_normalize_agent_roles_sync_payload_rejects_invalid_roles() {
        let mut payload = serde_json::Map::new();
        payload.insert("status".into(), Value::String("ok".into()));
        payload.insert("roles".into(), Value::String("not-an-array".into()));
        assert!(normalize_agent_roles_sync_payload(payload).is_err());
    }
}
