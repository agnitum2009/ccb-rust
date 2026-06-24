use camino::Utf8Path;
use ccbr_storage::json::JsonStore;
use ccbr_storage::paths::PathLayout;

use super::models::PersistedExecutionState;

/// Store for persisted execution state records.
#[derive(Clone)]
pub struct ExecutionStateStore {
    layout: PathLayout,
    store: JsonStore,
}

impl ExecutionStateStore {
    pub fn new(layout: PathLayout) -> Self {
        Self {
            layout,
            store: JsonStore::new(),
        }
    }

    pub fn with_store(layout: PathLayout, store: JsonStore) -> Self {
        Self { layout, store }
    }

    pub fn load(&self, job_id: &str) -> Option<PersistedExecutionState> {
        let path = self.layout.execution_state_path(job_id);
        if !path.exists() {
            return None;
        }
        self.store.load(&path).ok()
    }

    pub fn save(&self, state: &PersistedExecutionState) -> ccbr_storage::Result<()> {
        let path = self.layout.execution_state_path(state.job_id());
        self.store.save(&path, state)
    }

    pub fn remove(&self, job_id: &str) {
        let path = self.layout.execution_state_path(job_id);
        let _ = std::fs::remove_file(path);
    }

    pub fn list_all(&self) -> Vec<PersistedExecutionState> {
        let directory = self.layout.ccbd_executions_dir();
        if !directory.exists() {
            return Vec::new();
        }
        let mut states = Vec::new();
        let Ok(entries) = std::fs::read_dir(&directory) else {
            return states;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Some(utf8_path) = Utf8Path::from_path(&path) {
                if let Ok(state) = self.store.load(utf8_path) {
                    states.push(state);
                }
            }
        }
        states.sort_by(|a, b| a.job_id().cmp(b.job_id()));
        states
    }

    pub fn summary(&self) -> serde_json::Value {
        let states = self.list_all();
        let recoverable: Vec<_> = states.iter().filter(|s| s.resume_capable).collect();
        let nonrecoverable: Vec<_> = states.iter().filter(|s| !s.resume_capable).collect();
        serde_json::json!({
            "active_execution_count": states.len(),
            "recoverable_execution_count": recoverable.len(),
            "nonrecoverable_execution_count": nonrecoverable.len(),
            "pending_items_count": states.iter().filter(|s| !s.pending_items.is_empty()).count(),
            "terminal_pending_count": states.iter().filter(|s| s.pending_decision.is_some()).count(),
            "recoverable_execution_providers": recoverable.iter().map(|s| s.provider()).collect::<std::collections::BTreeSet<_>>(),
            "nonrecoverable_execution_providers": nonrecoverable.iter().map(|s| s.provider()).collect::<std::collections::BTreeSet<_>>(),
        })
    }
}
