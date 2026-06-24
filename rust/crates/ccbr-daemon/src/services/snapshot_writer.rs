pub struct SnapshotWriter {
    layout: ccbr_storage::paths::PathLayout,
    json_store: ccbr_storage::json::JsonStore,
}

impl SnapshotWriter {
    pub fn new(layout: ccbr_storage::paths::PathLayout) -> Self {
        Self {
            layout,
            json_store: ccbr_storage::json::JsonStore::new(),
        }
    }

    pub fn save(&self, job_id: &str, snapshot: &serde_json::Value) -> Result<(), String> {
        let path = self
            .layout
            .ccbd_dir()
            .join("snapshots")
            .join(format!("{}.json", job_id));
        self.json_store
            .save(&path, snapshot)
            .map_err(|e| e.to_string())
    }

    pub fn load(&self, job_id: &str) -> Option<serde_json::Value> {
        let path = self
            .layout
            .ccbd_dir()
            .join("snapshots")
            .join(format!("{}.json", job_id));
        if !path.exists() {
            return None;
        }
        self.json_store.load(&path).ok()
    }

    pub fn delete(&self, job_id: &str) -> Result<(), String> {
        let path = self
            .layout
            .ccbd_dir()
            .join("snapshots")
            .join(format!("{}.json", job_id));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
