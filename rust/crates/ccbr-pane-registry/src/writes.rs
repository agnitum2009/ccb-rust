use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use camino::Utf8Path;
use ccbr_project::runtime_paths::project_anchor_exists;
use serde_json::{Map, Value};

use crate::common::{debug, get_providers_map, load_registry_file, registry_path_for_session};

/// Upsert a registry record for a session.
pub fn upsert_registry<F, G>(
    record: &Map<String, Value>,
    atomic_write_text_fn: F,
    compute_project_id_fn: G,
) -> bool
where
    F: Fn(&Path, &str) -> std::io::Result<()>,
    G: Fn(&str) -> String,
{
    let session_id = match record.get("ccbr_session_id").and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => {
            debug("Registry update skipped: missing ccbr_session_id");
            return false;
        }
    };
    let work_dir = match record.get("work_dir").and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => {
            debug("Registry update skipped: missing work_dir for project-scoped registry");
            return false;
        }
    };
    if !project_anchor_exists(Utf8Path::new(work_dir)) {
        debug(&format!(
            "Registry update skipped: no .ccbr anchor for {work_dir}"
        ));
        return false;
    }
    let path = registry_path_for_session(session_id, Some(Path::new(work_dir)));

    let mut data: Map<String, Value> = if path.exists() {
        load_registry_file(&path).unwrap_or_default()
    } else {
        Map::new()
    };

    let mut providers = get_providers_map(&data);
    _merge_provider_maps(&mut providers, record);
    _merge_top_level_fields(&mut data, record);

    let providers_value: Map<String, Value> = providers
        .into_iter()
        .map(|(k, v)| (k, Value::Object(v)))
        .collect();
    data.insert("providers".to_string(), Value::Object(providers_value));
    _ensure_project_id(&mut data, &compute_project_id_fn);
    data.insert("updated_at".to_string(), Value::from(now_secs()));

    let json = serde_json::to_string_pretty(&data).unwrap_or_default();
    match atomic_write_text_fn(&path, &json) {
        Ok(()) => true,
        Err(e) => {
            debug(&format!("Failed to write registry {path:?}: {e}"));
            false
        }
    }
}

fn _merge_provider_maps(
    providers: &mut HashMap<String, Map<String, Value>>,
    record: &Map<String, Value>,
) {
    if let Some(incoming) = record.get("providers").and_then(|v| v.as_object()) {
        for (provider, entry) in incoming {
            let provider = provider.trim().to_lowercase();
            if let Some(entry) = entry.as_object() {
                _merge_provider_entry(providers, &provider, entry);
            }
        }
    }
    if let Some(provider) = record.get("provider").and_then(|v| v.as_str()) {
        let provider = provider.trim().to_lowercase();
        if !provider.is_empty() {
            let fields = _single_provider_fields(record);
            _merge_provider_entry(providers, &provider, &fields);
        }
    }
}

fn _merge_provider_entry(
    providers: &mut HashMap<String, Map<String, Value>>,
    provider: &str,
    entry: &Map<String, Value>,
) {
    let target = providers.entry(provider.to_string()).or_default();
    for (key, value) in entry {
        if !value.is_null() {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn _single_provider_fields(record: &Map<String, Value>) -> Map<String, Value> {
    record
        .iter()
        .filter(|(k, v)| {
            !v.is_null() && *k != "provider" && *k != "providers" && _provider_field(k)
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn _provider_field(key: &str) -> bool {
    key == "pane_id"
        || key == "pane_title_marker"
        || key.ends_with("_session_id")
        || key.ends_with("_session_path")
        || key.ends_with("_project_id")
}

fn _merge_top_level_fields(data: &mut Map<String, Value>, record: &Map<String, Value>) {
    for (key, value) in record {
        if !value.is_null() && key != "providers" && key != "provider" {
            data.insert(key.clone(), value.clone());
        }
    }
}

fn _ensure_project_id<G>(data: &mut Map<String, Value>, compute_project_id_fn: &G)
where
    G: Fn(&str) -> String,
{
    let has_id = data
        .get("ccbr_project_id")
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if has_id {
        return;
    }
    let work_dir = data
        .get("work_dir")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .unwrap_or("");
    if work_dir.is_empty() {
        return;
    }
    data.insert(
        "ccbr_project_id".to_string(),
        Value::String(compute_project_id_fn(work_dir)),
    );
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn atomic_fn(path: &Path, text: &str) -> std::io::Result<()> {
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, text)
    }

    fn compute_id_fn(work_dir: &str) -> String {
        format!("id-for-{work_dir}")
    }

    fn create_anchor(tmp: &tempfile::TempDir) -> std::path::PathBuf {
        let anchor = tmp.path().join(".ccbr");
        fs::create_dir_all(&anchor).unwrap();
        anchor
    }

    #[test]
    fn test_upsert_registry_missing_session_id() {
        let record = Map::new();
        assert!(!upsert_registry(&record, atomic_fn, compute_id_fn));
    }

    #[test]
    fn test_upsert_registry_missing_work_dir() {
        let mut record = Map::new();
        record.insert("ccbr_session_id".into(), "s1".into());
        assert!(!upsert_registry(&record, atomic_fn, compute_id_fn));
    }

    #[test]
    fn test_upsert_registry_missing_anchor() {
        let tmp = tempfile::tempdir().unwrap();
        let mut record = Map::new();
        record.insert("ccbr_session_id".into(), "s1".into());
        record.insert(
            "work_dir".into(),
            tmp.path().to_string_lossy().into_owned().into(),
        );
        assert!(!upsert_registry(&record, atomic_fn, compute_id_fn));
    }

    #[test]
    fn test_upsert_registry_new_record() {
        let tmp = tempfile::tempdir().unwrap();
        create_anchor(&tmp);
        let mut record = Map::new();
        record.insert("ccbr_session_id".into(), "s1".into());
        record.insert(
            "work_dir".into(),
            tmp.path().to_string_lossy().into_owned().into(),
        );
        record.insert("provider".into(), "claude".into());
        record.insert("pane_id".into(), "%1".into());

        assert!(upsert_registry(&record, atomic_fn, compute_id_fn));

        let path = registry_path_for_session("s1", Some(tmp.path()));
        assert!(path.exists());
        let data = load_registry_file(&path).unwrap();
        assert_eq!(data["providers"]["claude"]["pane_id"], "%1");
        assert_eq!(
            data["ccbr_project_id"],
            "id-for-".to_string() + &tmp.path().to_string_lossy()
        );
    }

    #[test]
    fn test_upsert_registry_merge_existing_providers() {
        let tmp = tempfile::tempdir().unwrap();
        create_anchor(&tmp);
        let mut first = Map::new();
        first.insert("ccbr_session_id".into(), "s1".into());
        first.insert(
            "work_dir".into(),
            tmp.path().to_string_lossy().into_owned().into(),
        );
        first.insert("provider".into(), "claude".into());
        first.insert("pane_id".into(), "%1".into());
        upsert_registry(&first, atomic_fn, compute_id_fn);

        let mut second = Map::new();
        second.insert("ccbr_session_id".into(), "s1".into());
        second.insert(
            "work_dir".into(),
            tmp.path().to_string_lossy().into_owned().into(),
        );
        second.insert("provider".into(), "codex".into());
        second.insert("pane_id".into(), "%2".into());
        upsert_registry(&second, atomic_fn, compute_id_fn);

        let path = registry_path_for_session("s1", Some(tmp.path()));
        let data = load_registry_file(&path).unwrap();
        assert_eq!(data["providers"]["claude"]["pane_id"], "%1");
        assert_eq!(data["providers"]["codex"]["pane_id"], "%2");
    }

    #[test]
    fn test_upsert_registry_preserves_existing_project_id() {
        let tmp = tempfile::tempdir().unwrap();
        create_anchor(&tmp);
        let mut first = Map::new();
        first.insert("ccbr_session_id".into(), "s1".into());
        first.insert(
            "work_dir".into(),
            tmp.path().to_string_lossy().into_owned().into(),
        );
        first.insert("ccbr_project_id".into(), "preset-id".into());
        upsert_registry(&first, atomic_fn, compute_id_fn);

        let path = registry_path_for_session("s1", Some(tmp.path()));
        let data = load_registry_file(&path).unwrap();
        assert_eq!(data["ccbr_project_id"], "preset-id");
    }
}
