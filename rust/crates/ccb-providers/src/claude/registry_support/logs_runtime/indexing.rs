//! Mirrors Python `lib/provider_backends/claude/registry_support/logs_runtime/indexing.py`.

use std::path::{Path, PathBuf};

use crate::claude::session_index_runtime::{
    candidate_paths_for_work_dir, load_index_entries, resolve_registry_index_location,
    select_best_session_path,
};

/// Parse the sessions index for a work directory and return the best log path.
pub fn parse_sessions_index(work_dir: &Path, root: &Path) -> Option<PathBuf> {
    let candidates = candidate_paths_for_work_dir(work_dir, false);
    let location = resolve_registry_index_location(work_dir, root)?;
    let entries = load_index_entries(&location.index_path)?;
    select_best_session_path(&entries, &candidates, &location.project_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_sessions_index_selects_existing_session() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("claude-root");
        let work_dir = tmp.path().join("repo");
        std::fs::create_dir_all(&work_dir).unwrap();
        let project_key = work_dir
            .to_string_lossy()
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
            .collect::<String>();
        let project_dir = root.join(&project_key);
        std::fs::create_dir_all(&project_dir).unwrap();
        let session = project_dir.join("bound-session.jsonl");
        std::fs::write(&session, "").unwrap();
        std::fs::write(
            project_dir.join("sessions-index.json"),
            serde_json::json!({
                "entries": [
                    {"fullPath": "bound-session.jsonl", "fileMtime": 1000, "projectPath": work_dir.to_string_lossy().to_string()},
                ]
            })
            .to_string(),
        )
        .unwrap();

        let result = parse_sessions_index(&work_dir, &root);
        assert_eq!(result, Some(session));
    }
}
