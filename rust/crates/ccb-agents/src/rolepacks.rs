use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::roles::{canonical_role_id, role_id_candidates};

pub const SUPPORTED_ROLE_SCHEMA: &str = "rolepack/v1";
pub const AGENT_ROLE_SCHEMA_PREFIX: &str = "agent-role/preview-";
pub const CCB_ADAPTER_SCHEMA_PREFIX: &str = "agent-role-adapter/ccb-preview-";

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
    let manifest_path = root.join("role.toml");
    if !manifest_path.is_file() {
        return Err(crate::AgentError::Role(format!(
            "role manifest not found: {}",
            manifest_path.display()
        )));
    }
    role_manifest_from_mapping(root, read_toml_manifest(&manifest_path)?)
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
    vec![agent_roles_installed_root()]
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
}

#[derive(Debug, Clone)]
pub struct RoleSource {
    pub name: String,
    pub path: PathBuf,
    pub source_type: String,
}

pub fn discover_path_roles(path: &Path) -> crate::Result<Vec<SourceRole>> {
    discover_roles_from_sources(&[RoleSource {
        name: "path".into(),
        path: path.expand_home(),
        source_type: "path".into(),
    }])
}

fn discover_roles_from_sources(sources: &[RoleSource]) -> crate::Result<Vec<SourceRole>> {
    let mut discovered: HashMap<String, SourceRole> = HashMap::new();
    for source in sources {
        for role_path in iter_role_paths(&source.path) {
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
            };
            discovered.insert(role.id.clone(), source_role);
        }
    }
    let mut roles: Vec<SourceRole> = discovered.into_values().collect();
    roles.sort_by(|a, b| a.role_id.cmp(&b.role_id));
    Ok(roles)
}

fn iter_role_paths(source_root: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for base_name in &["roles", "reference_roles"] {
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
}
