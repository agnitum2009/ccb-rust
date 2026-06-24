use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// User session information used for backend selection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserSession {
    pub terminal: Option<String>,
    pub tmux_socket_name: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub pane_id: Option<String>,
    pub tmux_session: Option<String>,
}

/// Registry entry mapping an agent to a pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneEntry {
    pub pane_id: String,
    pub agent_name: String,
    pub provider: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
}

/// In-memory registry for mapping agent names to pane IDs.
#[derive(Debug, Default)]
pub struct PaneRegistry {
    entries: HashMap<String, PaneEntry>,
}

impl PaneRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn register(&mut self, entry: PaneEntry) {
        self.entries.insert(entry.agent_name.clone(), entry);
    }

    pub fn get(&self, agent_name: &str) -> Option<&PaneEntry> {
        self.entries.get(agent_name)
    }

    pub fn remove(&mut self, agent_name: &str) -> Option<PaneEntry> {
        self.entries.remove(agent_name)
    }

    pub fn all_entries(&self) -> Vec<&PaneEntry> {
        self.entries.values().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

const REGISTRY_PREFIX: &str = "ccb-session-";
const REGISTRY_SUFFIX: &str = ".json";
const REGISTRY_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;

/// Return the registry directory for a work directory.
pub fn registry_dir(work_dir: Option<&Path>) -> PathBuf {
    match work_dir {
        Some(dir) => dir.join(".ccbr").join("registry"),
        None => std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".ccbr")
            .join("registry"),
    }
}

/// Return the registry file path for a session.
pub fn registry_path_for_session(session_id: &str, work_dir: Option<&Path>) -> PathBuf {
    registry_dir(work_dir).join(format!("{REGISTRY_PREFIX}{session_id}{REGISTRY_SUFFIX}"))
}

/// Iterate over registry file paths.
pub fn iter_registry_files(work_dir: Option<&Path>) -> Vec<PathBuf> {
    let dir = registry_dir(work_dir);
    if !dir.exists() {
        return Vec::new();
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    name.starts_with(REGISTRY_PREFIX) && name.ends_with(REGISTRY_SUFFIX)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Coerce an updated_at value to seconds, falling back to file mtime.
pub fn coerce_updated_at(value: Option<i64>, fallback_path: Option<&Path>) -> i64 {
    if let Some(v) = value {
        return v;
    }
    if let Some(path) = fallback_path {
        if let Ok(meta) = path.metadata() {
            if let Ok(modified) = meta.modified() {
                return modified
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
            }
        }
    }
    0
}

/// Check if a registry timestamp is stale.
pub fn is_stale(updated_at: i64, now: Option<i64>) -> bool {
    if updated_at <= 0 {
        return true;
    }
    let now_ts = now.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    });
    (now_ts - updated_at) > REGISTRY_TTL_SECONDS
}

/// Load a registry file as JSON.
pub fn load_registry_file(path: &Path) -> Option<serde_json::Map<String, serde_json::Value>> {
    let data = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&data).ok()?;
    value.as_object().cloned()
}

/// Load a fresh registry by session id.
pub fn load_registry_by_session_id(
    session_id: &str,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    if session_id.is_empty() {
        return None;
    }
    let path = registry_path_for_session(session_id, None);
    load_fresh_registry(&path, None).map(|(data, _)| data)
}

/// Load a fresh registry record from a path.
pub fn load_fresh_registry(
    path: &Path,
    stale_debug_message: Option<&str>,
) -> Option<(serde_json::Map<String, serde_json::Value>, i64)> {
    if !path.exists() {
        return None;
    }
    let data = load_registry_file(path)?;
    let updated_at = coerce_updated_at(data.get("updated_at").and_then(|v| v.as_i64()), Some(path));
    if is_stale(updated_at, None) {
        if let Some(msg) = stale_debug_message {
            eprintln!("[DEBUG] {msg}");
        }
        return None;
    }
    Some((data, updated_at))
}

/// Upsert a registry record for a session.
pub fn upsert_registry(record: &serde_json::Map<String, serde_json::Value>) -> bool {
    let session_id = match record.get("ccb_session_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => {
            eprintln!("[DEBUG] Registry update skipped: missing ccb_session_id");
            return false;
        }
    };
    let work_dir = match record.get("work_dir").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => {
            eprintln!(
                "[DEBUG] Registry update skipped: missing work_dir for project-scoped registry"
            );
            return false;
        }
    };
    let path = registry_path_for_session(session_id, Some(Path::new(work_dir)));
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut data: serde_json::Map<String, serde_json::Value> = if path.exists() {
        load_registry_file(&path).unwrap_or_default()
    } else {
        serde_json::Map::new()
    };

    merge_provider_maps(&mut data, record);
    merge_top_level_fields(&mut data, record);
    data.insert("updated_at".to_string(), now_secs().into());

    match std::fs::write(
        &path,
        serde_json::to_string_pretty(&data).unwrap_or_default(),
    ) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("[DEBUG] Failed to write registry {path:?}: {e}");
            false
        }
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn merge_provider_maps(
    data: &mut serde_json::Map<String, serde_json::Value>,
    record: &serde_json::Map<String, serde_json::Value>,
) {
    let mut providers = data
        .get("providers")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    if let Some(incoming) = record.get("providers").and_then(|v| v.as_object()) {
        for (provider, entry) in incoming {
            let provider = provider.trim().to_lowercase();
            if let Some(entry) = entry.as_object() {
                merge_provider_entry(&mut providers, &provider, entry);
            }
        }
    }
    if let Some(provider) = record.get("provider").and_then(|v| v.as_str()) {
        let provider = provider.trim().to_lowercase();
        if !provider.is_empty() {
            let fields = single_provider_fields(record);
            merge_provider_entry(&mut providers, &provider, &fields);
        }
    }
    data.insert("providers".to_string(), providers.into());
}

fn merge_provider_entry(
    providers: &mut serde_json::Map<String, serde_json::Value>,
    provider: &str,
    entry: &serde_json::Map<String, serde_json::Value>,
) {
    let target = providers
        .entry(provider.to_string())
        .or_insert_with(|| serde_json::Map::new().into());
    let target_obj = target.as_object_mut().unwrap();
    for (key, value) in entry {
        if !value.is_null() {
            target_obj.insert(key.clone(), value.clone());
        }
    }
}

fn single_provider_fields(
    record: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    record
        .iter()
        .filter(|(k, v)| {
            !v.is_null() && *k != "provider" && *k != "providers" && is_provider_field(k)
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn is_provider_field(key: &str) -> bool {
    key == "pane_id"
        || key == "pane_title_marker"
        || key.ends_with("_session_id")
        || key.ends_with("_session_path")
        || key.ends_with("_project_id")
}

fn merge_top_level_fields(
    data: &mut serde_json::Map<String, serde_json::Value>,
    record: &serde_json::Map<String, serde_json::Value>,
) {
    for (key, value) in record {
        if !value.is_null() && key != "providers" && key != "provider" {
            data.insert(key.clone(), value.clone());
        }
    }
}

/// Normalize a path for directory matching.
pub fn normalize_path_for_match(value: &str) -> String {
    let raw = value.trim();
    if raw.is_empty() {
        return String::new();
    }
    let path = Path::new(raw);
    let resolved = if let Ok(r) = path.canonicalize() {
        r
    } else {
        path.to_path_buf()
    };
    let normalized = resolved
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();
    #[cfg(target_os = "windows")]
    let normalized = normalized.to_lowercase();
    normalized
}

/// Check if parent is the same as or a parent of child.
pub fn path_is_same_or_parent(parent: &str, child: &str) -> bool {
    let normalized_parent = normalize_path_for_match(parent);
    let normalized_child = normalize_path_for_match(child);
    if normalized_parent.is_empty() || normalized_child.is_empty() {
        return false;
    }
    if normalized_parent == normalized_child {
        return true;
    }
    if !normalized_child.starts_with(&normalized_parent) {
        return false;
    }
    normalized_child[normalized_parent.len()..].starts_with('/')
}

/// Extract provider map from registry data.
pub fn get_providers_map(
    data: &serde_json::Map<String, serde_json::Value>,
) -> HashMap<String, serde_json::Map<String, serde_json::Value>> {
    data.get("providers")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_object().map(|o| (k.trim().to_lowercase(), o.clone())))
                .collect()
        })
        .unwrap_or_default()
}

/// Extract the claude pane id from registry data.
pub fn claude_pane_id(data: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let providers = get_providers_map(data);
    providers.get("claude").and_then(|entry| {
        entry
            .get("pane_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_registry_crud() {
        let mut reg = PaneRegistry::new();
        assert!(reg.is_empty());

        reg.register(PaneEntry {
            pane_id: "%0".into(),
            agent_name: "agent-a".into(),
            provider: "claude".into(),
            workspace_path: None,
        });
        assert_eq!(reg.len(), 1);

        let entry = reg.get("agent-a").unwrap();
        assert_eq!(entry.pane_id, "%0");

        reg.remove("agent-a");
        assert!(reg.get("agent-a").is_none());
    }

    #[test]
    fn test_registry_path_and_is_stale() {
        let path = registry_path_for_session("abc123", None);
        assert!(path.to_string_lossy().contains("ccb-session-abc123.json"));
        assert!(!is_stale(now_secs(), None));
        assert!(is_stale(1, Some(now_secs())));
    }

    #[test]
    fn test_upsert_registry() {
        let tmp = std::env::temp_dir().join(format!("ccb-registry-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let work_dir = tmp.to_string_lossy().to_string();
        let mut record = serde_json::Map::new();
        record.insert("ccb_session_id".to_string(), "sess-1".into());
        record.insert("work_dir".to_string(), work_dir.clone().into());
        record.insert("provider".to_string(), "claude".into());
        record.insert("pane_id".to_string(), "%1".into());
        upsert_registry(&record);

        let path = registry_path_for_session("sess-1", Some(&tmp));
        assert!(path.exists());
        let data = load_registry_file(&path).unwrap();
        let providers = get_providers_map(&data);
        assert_eq!(
            providers["claude"]["pane_id"],
            serde_json::Value::String("%1".to_string())
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_path_is_same_or_parent() {
        assert!(path_is_same_or_parent("/tmp", "/tmp/foo"));
        assert!(path_is_same_or_parent("/tmp", "/tmp"));
        assert!(!path_is_same_or_parent("/tmp", "/tmpfoo"));
    }
}
