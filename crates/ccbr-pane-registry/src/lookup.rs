use std::path::{Path, PathBuf};

use ccbr_terminal::backend::TerminalBackend;
use ccbr_terminal::registry::UserSession;
use serde_json::{Map, Value};

use crate::common::registry_path_for_session;
use crate::lookup_project::{latest_project_registry_record, migrate_project_id};
use crate::lookup_records::{
    claude_pane_id, iter_fresh_registry_records, latest_registry_record, load_fresh_registry,
};

/// Load a fresh registry record by session id.
pub fn load_registry_by_session_id(session_id: &str) -> Option<Map<String, Value>> {
    if session_id.is_empty() {
        return None;
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path = registry_path_for_session(session_id, Some(&cwd));
    load_fresh_registry(
        &path,
        Some(&format!(
            "Registry stale for session {session_id}: {}",
            path.display()
        )),
    )
    .map(|(data, _)| data)
}

/// Load a fresh registry record by claude pane id.
pub fn load_registry_by_claude_pane<F>(
    pane_id: &str,
    _get_backend_for_session_fn: F,
) -> Option<Map<String, Value>>
where
    F: Fn(&UserSession) -> Option<Box<dyn TerminalBackend>>,
{
    if pane_id.is_empty() {
        return None;
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let stale_fn = |path: &Path| format!("Registry stale for pane {pane_id}: {}", path.display());
    let records = iter_fresh_registry_records(Some(&cwd), Some(&stale_fn));
    let best = latest_registry_record(
        records
            .into_iter()
            .filter(|(data, _)| claude_pane_id(data).as_deref() == Some(pane_id)),
    );
    best.map(|(data, _)| data)
}

/// Load the newest alive registry record matching `{ccbr_project_id, provider}`.
pub fn load_registry_by_project_id<F, G, H>(
    ccbr_project_id: &str,
    provider: &str,
    work_dir: Option<&Path>,
    get_backend_for_session_fn: F,
    compute_project_id_fn: G,
    upsert_registry_fn: H,
) -> Option<Map<String, Value>>
where
    F: Fn(&UserSession) -> Option<Box<dyn TerminalBackend>>,
    G: Fn(&str) -> String,
    H: Fn(&Map<String, Value>) -> bool,
{
    let project_id = ccbr_project_id.trim();
    let qualified_provider = provider.trim().to_lowercase();
    let requested_work_dir = work_dir.map(|p| p.to_string_lossy().to_string());
    if project_id.is_empty() || qualified_provider.is_empty() {
        return None;
    }

    let best = latest_project_registry_record(
        project_id,
        &qualified_provider,
        requested_work_dir.as_deref(),
        &get_backend_for_session_fn,
        &compute_project_id_fn,
    )?;
    let (mut best_record, best_needs_migration) = best;
    if best_needs_migration {
        migrate_project_id(
            &mut best_record,
            &compute_project_id_fn,
            &upsert_registry_fn,
        );
    }

    Some(best_record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct MockBackend;

    impl TerminalBackend for MockBackend {
        fn send_text(&self, _pane_id: &str, _text: &str) -> ccbr_terminal::backend::Result<()> {
            Ok(())
        }
        fn is_alive(&self, _pane_id: &str) -> ccbr_terminal::backend::Result<bool> {
            Ok(true)
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
        Some(Box::new(MockBackend))
    }

    fn write_registry(tmp: &tempfile::TempDir, session_id: &str, content: &str) {
        let path = registry_path_for_session(session_id, Some(tmp.path()));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
    }

    fn set_cwd(tmp: &tempfile::TempDir) {
        std::env::set_current_dir(tmp.path()).unwrap();
    }

    #[test]
    fn test_load_registry_by_session_id_empty() {
        assert!(load_registry_by_session_id("").is_none());
    }

    #[test]
    fn test_load_registry_by_session_id_found() {
        let tmp = tempfile::tempdir().unwrap();
        set_cwd(&tmp);
        write_registry(&tmp, "sess-1", r#"{"updated_at": 9999999999}"#);
        let data = load_registry_by_session_id("sess-1").unwrap();
        assert_eq!(data["updated_at"], 9999999999i64);
    }

    #[test]
    fn test_load_registry_by_claude_pane_found() {
        let tmp = tempfile::tempdir().unwrap();
        set_cwd(&tmp);
        write_registry(
            &tmp,
            "sess-1",
            r#"{"updated_at": 9999999999, "providers": {"claude": {"pane_id": "%42"}}}"#,
        );
        let data = load_registry_by_claude_pane("%42", backend_fn).unwrap();
        assert_eq!(data["providers"]["claude"]["pane_id"], "%42");
    }

    #[test]
    fn test_load_registry_by_claude_pane_empty() {
        assert!(load_registry_by_claude_pane("", backend_fn).is_none());
    }

    #[test]
    fn test_load_registry_by_claude_pane_ignores_flat_legacy_field() {
        // Mirrors test/test_registry_cleanup.py.
        let tmp = tempfile::tempdir().unwrap();
        set_cwd(&tmp);
        // A legacy record stores the pane id as a top-level flat field, not
        // under providers.claude.pane_id. The lookup should ignore it.
        write_registry(
            &tmp,
            "sess-legacy",
            r#"{"updated_at": 9999999999, "claude_pane_id": "%legacy"}"#,
        );
        assert!(load_registry_by_claude_pane("%legacy", backend_fn).is_none());
    }
}
