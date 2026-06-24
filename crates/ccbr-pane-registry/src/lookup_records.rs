use std::path::Path;

use serde_json::{Map, Value};

use crate::common::{
    coerce_updated_at, debug, get_providers_map, is_stale, iter_registry_files, load_registry_file,
};

/// Load a fresh (non-stale) registry record from a path.
pub fn load_fresh_registry(
    path: &Path,
    stale_debug_message: Option<&str>,
) -> Option<(Map<String, Value>, i64)> {
    if !path.exists() {
        return None;
    }
    let data = load_registry_file(path)?;
    let updated_at = coerce_updated_at(data.get("updated_at"), Some(path));
    if is_stale(updated_at, None) {
        if let Some(msg) = stale_debug_message {
            debug(msg);
        }
        return None;
    }
    Some((data, updated_at))
}

/// Iterate over fresh registry records in a work directory.
pub fn iter_fresh_registry_records(
    work_dir: Option<&Path>,
    stale_debug_message_fn: Option<&dyn Fn(&Path) -> String>,
) -> Vec<(Map<String, Value>, i64)> {
    let mut records = Vec::new();
    for path in iter_registry_files(work_dir) {
        let stale_debug_message = stale_debug_message_fn.map(|f| f(&path));
        if let Some(record) = load_fresh_registry(&path, stale_debug_message.as_deref()) {
            records.push(record);
        }
    }
    records
}

/// Return the latest registry record by `updated_at`.
pub fn latest_registry_record<I>(records: I) -> Option<(Map<String, Value>, i64)>
where
    I: IntoIterator<Item = (Map<String, Value>, i64)>,
{
    let mut best: Option<(Map<String, Value>, i64)> = None;
    let mut best_ts = -1;
    for (data, updated_at) in records {
        if updated_at > best_ts {
            best = Some((data, updated_at));
            best_ts = updated_at;
        }
    }
    best
}

/// Extract the claude pane id from registry data.
pub fn claude_pane_id(data: &Map<String, Value>) -> Option<String> {
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
    use std::fs;

    #[test]
    fn test_load_fresh_registry_missing() {
        let path = Path::new("/no/such/registry.json");
        assert!(load_fresh_registry(path, None).is_none());
    }

    #[test]
    fn test_load_fresh_registry_stale() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("stale.json");
        fs::write(&path, r#"{"updated_at": 1}"#).unwrap();
        assert!(load_fresh_registry(&path, None).is_none());
    }

    #[test]
    fn test_load_fresh_registry_fresh() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("fresh.json");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        fs::write(&path, format!(r#"{{"updated_at": {now}}}"#)).unwrap();
        let (data, ts) = load_fresh_registry(&path, None).unwrap();
        assert_eq!(data.get("updated_at").unwrap().as_i64(), Some(now));
        assert_eq!(ts, now);
    }

    #[test]
    fn test_latest_registry_record() {
        let mut a = Map::new();
        a.insert("name".into(), "a".into());
        let mut b = Map::new();
        b.insert("name".into(), "b".into());
        let records = vec![(a, 10), (b, 20)];
        let (best, ts) = latest_registry_record(records).unwrap();
        assert_eq!(best["name"], "b");
        assert_eq!(ts, 20);
    }

    #[test]
    fn test_claude_pane_id() {
        let mut data = Map::new();
        let mut providers = Map::new();
        let mut claude = Map::new();
        claude.insert("pane_id".into(), "%42".into());
        providers.insert("claude".into(), Value::Object(claude));
        data.insert("providers".into(), Value::Object(providers));
        assert_eq!(claude_pane_id(&data), Some("%42".into()));
    }

    #[test]
    fn test_claude_pane_id_empty() {
        let data = Map::new();
        assert_eq!(claude_pane_id(&data), None);
    }
}
