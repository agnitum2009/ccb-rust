use camino::Utf8Path;
use ccb_storage::jsonl::JsonlStore;
use ccb_storage::paths::PathLayout;

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

    pub fn append(&self, snapshot: &ProviderHealthSnapshot) -> ccb_storage::Result<()> {
        self.store.append(
            &self.layout.provider_health_path(&snapshot.job_id),
            snapshot,
        )
    }

    pub fn list_job(&self, job_id: &str) -> ccb_storage::Result<Vec<ProviderHealthSnapshot>> {
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
