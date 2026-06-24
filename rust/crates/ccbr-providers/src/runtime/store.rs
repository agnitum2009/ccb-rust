use camino::Utf8Path;
use ccbr_storage::jsonl::JsonlStore;
use ccbr_storage::paths::PathLayout;

use super::health::ProviderHealthSnapshot;

/// Store for provider health snapshots.
#[derive(Clone)]
pub struct ProviderHealthSnapshotStore {
    layout: PathLayout,
    store: JsonlStore,
}

impl ProviderHealthSnapshotStore {
    pub fn new(layout: PathLayout) -> Self {
        Self {
            layout,
            store: JsonlStore::new(),
        }
    }

    pub fn with_store(layout: PathLayout, store: JsonlStore) -> Self {
        Self { layout, store }
    }

    pub fn append(&self, snapshot: &ProviderHealthSnapshot) -> ccbr_storage::Result<()> {
        self.store.append(
            &self.layout.provider_health_path(&snapshot.job_id),
            snapshot,
        )
    }

    pub fn list_job(&self, job_id: &str) -> ccbr_storage::Result<Vec<ProviderHealthSnapshot>> {
        self.store
            .read_all(&self.layout.provider_health_path(job_id))
    }

    pub fn latest(&self, job_id: &str) -> Option<ProviderHealthSnapshot> {
        self.list_job(job_id).ok()?.into_iter().next_back()
    }

    pub fn list_all(&self) -> Vec<ProviderHealthSnapshot> {
        let directory = self.layout.ccbd_provider_health_dir();
        if !directory.exists() {
            return Vec::new();
        }
        let mut snapshots = Vec::new();
        let Ok(entries) = std::fs::read_dir(&directory) else {
            return snapshots;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            if let Some(utf8_path) = Utf8Path::from_path(&path) {
                if let Ok(rows) = self.store.read_all(utf8_path) {
                    snapshots.extend(rows);
                }
            }
        }
        snapshots.sort_by(|a, b| a.observed_at.cmp(&b.observed_at));
        snapshots
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::health::{ProgressState, ProviderCompletionState, ProviderHealthSnapshot};
    use serde_json::json;
    use std::collections::HashMap;

    fn make_layout(tmp: &tempfile::TempDir) -> PathLayout {
        PathLayout::new(
            camino::Utf8Path::from_path(tmp.path()).unwrap_or(camino::Utf8Path::new("/")),
        )
    }

    #[test]
    fn health_snapshot_store_tracks_job_history() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = make_layout(&tmp);
        let store = ProviderHealthSnapshotStore::new(layout);

        let mut d1 = HashMap::new();
        d1.insert("phase".into(), json!("accepted"));
        store
            .append(
                &ProviderHealthSnapshot::new("job-1", "codex", "Agent1", "2026-03-30T12:00:00Z")
                    .with_runtime_alive(true)
                    .with_session_reachable(Some(true))
                    .with_progress_state(ProgressState::Accepted)
                    .with_completion_state(ProviderCompletionState::NotComplete)
                    .with_last_progress_at("2026-03-30T12:00:00Z")
                    .with_diagnostics(d1),
            )
            .unwrap();

        let mut d2 = HashMap::new();
        d2.insert("phase".into(), json!("complete"));
        store
            .append(
                &ProviderHealthSnapshot::new("job-1", "codex", "agent1", "2026-03-30T12:00:05Z")
                    .with_runtime_alive(true)
                    .with_session_reachable(Some(true))
                    .with_progress_state(ProgressState::OutputAdvancing)
                    .with_completion_state(ProviderCompletionState::TerminalComplete)
                    .with_last_progress_at("2026-03-30T12:00:03Z")
                    .with_diagnostics(d2),
            )
            .unwrap();

        let latest = store.latest("job-1").unwrap();
        assert_eq!(latest.agent_name, "agent1");
        assert_eq!(latest.progress_state, ProgressState::OutputAdvancing);
        assert_eq!(
            latest.completion_state,
            ProviderCompletionState::TerminalComplete
        );
        assert_eq!(store.list_job("job-1").unwrap().len(), 2);
        assert_eq!(store.list_all().len(), 2);
    }
}
