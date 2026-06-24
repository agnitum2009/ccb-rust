use std::path::Path;

use ccbr_terminal::backend::TerminalBackend;
use ccbr_terminal::registry::UserSession;
use serde_json::{Map, Value};

use crate::common::{path_is_same_or_parent, provider_pane_alive};
use crate::lookup_records::iter_fresh_registry_records;

/// Find the latest alive registry record matching a project/provider.
pub fn latest_project_registry_record<F, G>(
    project_id: &str,
    qualified_provider: &str,
    requested_work_dir: Option<&str>,
    get_backend_for_session_fn: &F,
    compute_project_id_fn: &G,
) -> Option<(Map<String, Value>, bool)>
where
    F: Fn(&UserSession) -> Option<Box<dyn TerminalBackend>>,
    G: Fn(&str) -> String,
{
    let mut best: Option<(Map<String, Value>, bool)> = None;
    let mut best_ts = -1;
    let registry_work_dir = requested_work_dir.map(Path::new);
    for (data, updated_at) in iter_fresh_registry_records(registry_work_dir, None) {
        let m = match_project_registry_record(
            &data,
            project_id,
            qualified_provider,
            requested_work_dir,
            get_backend_for_session_fn,
            compute_project_id_fn,
        );
        if let Some((_, needs_migration)) = m {
            if updated_at > best_ts {
                best = Some((data, needs_migration));
                best_ts = updated_at;
            }
        }
    }
    best
}

/// Match a single registry record against project/provider criteria.
pub fn match_project_registry_record<F, G>(
    data: &Map<String, Value>,
    project_id: &str,
    qualified_provider: &str,
    requested_work_dir: Option<&str>,
    get_backend_for_session_fn: &F,
    compute_project_id_fn: &G,
) -> Option<(String, bool)>
where
    F: Fn(&UserSession) -> Option<Box<dyn TerminalBackend>>,
    G: Fn(&str) -> String,
{
    let existing_project_id = existing_project_id_from_record(data);
    let inferred_project_id = inferred_project_id_from_record(data, compute_project_id_fn);
    let effective_project_id = if existing_project_id.is_empty() {
        inferred_project_id.clone()
    } else {
        existing_project_id.clone()
    };
    if effective_project_id != project_id {
        return None;
    }
    if let Some(requested) = requested_work_dir {
        if !matches_requested_work_dir(data, requested) {
            return None;
        }
    }
    if !provider_pane_alive(data, qualified_provider, get_backend_for_session_fn) {
        return None;
    }
    Some((
        effective_project_id,
        existing_project_id.is_empty() && !inferred_project_id.is_empty(),
    ))
}

/// Return the existing project id stored in the record.
pub fn existing_project_id_from_record(data: &Map<String, Value>) -> String {
    data.get("ccbr_project_id")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Infer the project id from the record's work_dir.
pub fn inferred_project_id_from_record<G>(
    data: &Map<String, Value>,
    compute_project_id_fn: &G,
) -> String
where
    G: Fn(&str) -> String,
{
    let work_dir_value = record_work_dir(data);
    if work_dir_value.is_empty() {
        return String::new();
    }
    compute_project_id_fn(&work_dir_value)
}

/// Check whether the record's work_dir matches the requested directory.
pub fn matches_requested_work_dir(data: &Map<String, Value>, requested_work_dir: &str) -> bool {
    path_is_same_or_parent(&record_work_dir(data), requested_work_dir)
}

/// Return the work_dir value from a record.
pub fn record_work_dir(data: &Map<String, Value>) -> String {
    data.get("work_dir")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Migrate a record by computing and storing its project id.
pub fn migrate_project_id<F, G>(
    record: &mut Map<String, Value>,
    compute_project_id_fn: &F,
    upsert_registry_fn: &G,
) where
    F: Fn(&str) -> String,
    G: Fn(&Map<String, Value>) -> bool,
{
    if !existing_project_id_from_record(record).is_empty() {
        return;
    }
    let work_dir_value = record_work_dir(record);
    if work_dir_value.is_empty() {
        return;
    }
    record.insert(
        "ccbr_project_id".into(),
        Value::String(compute_project_id_fn(&work_dir_value)),
    );
    upsert_registry_fn(record);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockBackend {
        alive: HashMap<String, bool>,
    }

    impl TerminalBackend for MockBackend {
        fn send_text(&self, _pane_id: &str, _text: &str) -> ccbr_terminal::backend::Result<()> {
            Ok(())
        }
        fn is_alive(&self, pane_id: &str) -> ccbr_terminal::backend::Result<bool> {
            Ok(self.alive.get(pane_id).copied().unwrap_or(false))
        }
        fn kill_pane(&self, _pane_id: &str) -> ccbr_terminal::backend::Result<()> {
            Ok(())
        }
        fn activate(&self, _pane_id: &str) -> ccbr_terminal::backend::Result<()> {
            Ok(())
        }
        fn create_pane(
            &self,
            _cmd: &str,
            _cwd: &str,
            _direction: &str,
            _percent: u32,
            _parent_pane: Option<&str>,
        ) -> ccbr_terminal::backend::Result<String> {
            Ok("%0".into())
        }
    }

    fn backend_fn(_session: &UserSession) -> Option<Box<dyn TerminalBackend>> {
        let mut alive = HashMap::new();
        alive.insert("%1".into(), true);
        Some(Box::new(MockBackend { alive }))
    }

    fn compute_id_fn(work_dir: &str) -> String {
        format!("id-for-{work_dir}")
    }

    fn sample_record() -> Map<String, Value> {
        let mut data = Map::new();
        data.insert("work_dir".into(), "/tmp/proj".into());
        let mut providers = Map::new();
        let mut claude = Map::new();
        claude.insert("pane_id".into(), "%1".into());
        providers.insert("claude".into(), Value::Object(claude));
        data.insert("providers".into(), Value::Object(providers));
        data
    }

    #[test]
    fn test_existing_project_id_from_record() {
        let mut data = Map::new();
        data.insert("ccbr_project_id".into(), "  pid  ".into());
        assert_eq!(existing_project_id_from_record(&data), "pid");
    }

    #[test]
    fn test_inferred_project_id_from_record() {
        let data = sample_record();
        assert_eq!(
            inferred_project_id_from_record(&data, &compute_id_fn),
            "id-for-/tmp/proj"
        );
    }

    #[test]
    fn test_match_project_registry_record_existing_id() {
        let mut data = sample_record();
        data.insert("ccbr_project_id".into(), "pid".into());
        let result = match_project_registry_record(
            &data,
            "pid",
            "claude",
            None,
            &backend_fn,
            &compute_id_fn,
        );
        assert_eq!(result, Some(("pid".into(), false)));
    }

    #[test]
    fn test_match_project_registry_record_inferred_id() {
        let data = sample_record();
        let result = match_project_registry_record(
            &data,
            "id-for-/tmp/proj",
            "claude",
            None,
            &backend_fn,
            &compute_id_fn,
        );
        assert_eq!(result, Some(("id-for-/tmp/proj".into(), true)));
    }

    #[test]
    fn test_match_project_registry_record_wrong_id() {
        let data = sample_record();
        assert!(match_project_registry_record(
            &data,
            "wrong",
            "claude",
            None,
            &backend_fn,
            &compute_id_fn,
        )
        .is_none());
    }

    #[test]
    fn test_matches_requested_work_dir() {
        let mut data = Map::new();
        data.insert("work_dir".into(), "/tmp/proj".into());
        assert!(matches_requested_work_dir(&data, "/tmp/proj"));
        assert!(matches_requested_work_dir(&data, "/tmp/proj/sub"));
        assert!(!matches_requested_work_dir(&data, "/tmp/other"));
    }

    #[test]
    fn test_migrate_project_id() {
        let mut data = sample_record();
        migrate_project_id(&mut data, &compute_id_fn, &|_record: &Map<
            String,
            Value,
        >| true);
        assert_eq!(data["ccbr_project_id"], "id-for-/tmp/proj");
    }

    #[test]
    fn test_migrate_project_id_skips_existing() {
        let mut data = sample_record();
        data.insert("ccbr_project_id".into(), "existing".into());
        migrate_project_id(&mut data, &compute_id_fn, &|_record: &Map<
            String,
            Value,
        >| true);
        assert_eq!(data["ccbr_project_id"], "existing");
    }
}
