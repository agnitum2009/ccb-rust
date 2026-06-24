use std::path::Path;

use camino::Utf8Path;
use ccb_project::identity::compute_ccb_project_id;
use ccb_storage::atomic::atomic_write_text;
use ccb_terminal::backend::{TerminalBackend, TerminalBackendSelection};
use ccb_terminal::registry::UserSession;
use serde_json::{Map, Value};

use crate::lookup::{
    load_registry_by_claude_pane as load_registry_by_claude_pane_impl,
    load_registry_by_project_id as load_registry_by_project_id_impl,
    load_registry_by_session_id as load_registry_by_session_id_impl,
};
use crate::writes::upsert_registry as upsert_registry_impl;

/// Load a fresh registry record by session id.
pub fn load_registry_by_session_id(session_id: &str) -> Option<Map<String, Value>> {
    load_registry_by_session_id_impl(session_id)
}

/// Load a fresh registry record by claude pane id.
pub fn load_registry_by_claude_pane(pane_id: &str) -> Option<Map<String, Value>> {
    load_registry_by_claude_pane_impl(pane_id, get_backend_for_session)
}

/// Load the newest alive registry record matching `{ccb_project_id, provider}`.
pub fn load_registry_by_project_id(
    ccb_project_id: &str,
    provider: &str,
    work_dir: Option<&Path>,
) -> Option<Map<String, Value>> {
    load_registry_by_project_id_impl(
        ccb_project_id,
        provider,
        work_dir,
        get_backend_for_session,
        |work_dir| compute_ccb_project_id(work_dir),
        upsert_registry,
    )
}

/// Upsert a registry record for a session.
pub fn upsert_registry(record: &Map<String, Value>) -> bool {
    upsert_registry_impl(
        record,
        |path: &Path, text: &str| -> std::io::Result<()> {
            let utf8 = Utf8Path::from_path(path).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "non-utf8 path")
            })?;
            atomic_write_text(utf8, text)
        },
        |work_dir| compute_ccb_project_id(work_dir),
    )
}

fn get_backend_for_session(session: &UserSession) -> Option<Box<dyn TerminalBackend>> {
    let selection = TerminalBackendSelection::new();
    Some(Box::new(selection.get_backend_for_session(session)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_anchor(tmp: &tempfile::TempDir) {
        fs::create_dir_all(tmp.path().join(".ccbr")).unwrap();
    }

    #[test]
    fn test_api_upsert_registry_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        create_anchor(&tmp);
        let mut record = Map::new();
        record.insert("ccb_session_id".into(), "sess-api".into());
        record.insert(
            "work_dir".into(),
            tmp.path().to_string_lossy().into_owned().into(),
        );
        record.insert("provider".into(), "claude".into());
        record.insert("pane_id".into(), "%99".into());

        assert!(upsert_registry(&record));

        let data = load_registry_by_session_id("sess-api").unwrap();
        assert_eq!(data["providers"]["claude"]["pane_id"], "%99");
    }
}
