use ccbr_storage::json::JsonStore;
use ccbr_storage::jsonl::JsonlStore;
use ccbr_storage::paths::PathLayout;
use serde_json::Value;

use super::models::{ProjectNamespaceEvent, ProjectNamespaceState};

pub struct ProjectNamespaceStateStore {
    layout: PathLayout,
    store: JsonStore,
}

impl ProjectNamespaceStateStore {
    pub fn new(layout: &PathLayout) -> Self {
        Self {
            layout: layout.clone(),
            store: JsonStore::new(),
        }
    }

    pub fn load(&self) -> anyhow::Result<Option<ProjectNamespaceState>> {
        let path = self.layout.ccbrd_state_path();
        if !path.exists() {
            return Ok(None);
        }
        let value: Value = self.store.load(&path)?;
        Ok(Some(ProjectNamespaceState::from_record(&value)?))
    }

    pub fn save(&self, state: &ProjectNamespaceState) -> anyhow::Result<()> {
        let path = self.layout.ccbrd_state_path();
        self.store.save(&path, &state.to_record())?;
        Ok(())
    }
}

pub struct ProjectNamespaceEventStore {
    layout: PathLayout,
    store: JsonlStore,
}

impl ProjectNamespaceEventStore {
    pub fn new(layout: &PathLayout) -> Self {
        Self {
            layout: layout.clone(),
            store: JsonlStore::new(),
        }
    }

    pub fn append(&self, event: &ProjectNamespaceEvent) -> anyhow::Result<()> {
        let path = self.layout.ccbrd_lifecycle_log_path();
        self.store.append(&path, &event.to_record())?;
        Ok(())
    }

    pub fn read_all(&self) -> anyhow::Result<Vec<ProjectNamespaceEvent>> {
        let path = self.layout.ccbrd_lifecycle_log_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let rows: Vec<Value> = self.store.read_all(&path)?;
        rows.iter()
            .map(ProjectNamespaceEvent::from_record)
            .collect()
    }

    pub fn load_latest(&self) -> anyhow::Result<Option<ProjectNamespaceEvent>> {
        let rows = self.read_all()?;
        Ok(rows.into_iter().last())
    }
}

pub fn next_namespace_epoch(current: Option<&ProjectNamespaceState>) -> u64 {
    match current {
        None => 1,
        Some(state) => state.namespace_epoch + 1,
    }
}
