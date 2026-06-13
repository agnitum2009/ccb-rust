use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartPolicy {
    pub auto_permission: bool,
    pub source: String,
    pub created_at: String,
}

pub struct StartPolicyStore {
    path: camino::Utf8PathBuf,
}

impl StartPolicyStore {
    pub fn new(layout: &ccb_storage::paths::PathLayout) -> Self {
        Self {
            path: layout.ccbd_dir().join("start-policy.json"),
        }
    }

    pub fn save(&self, policy: &StartPolicy) -> Result<(), String> {
        ccb_storage::json::JsonStore::new()
            .save(&self.path, policy)
            .map_err(|e| e.to_string())
    }

    pub fn load(&self) -> Result<Option<StartPolicy>, String> {
        if !self.path.exists() {
            return Ok(None);
        }
        ccb_storage::json::JsonStore::new()
            .load(&self.path)
            .map(Some)
            .map_err(|e| e.to_string())
    }

    pub fn clear(&self) -> Result<(), String> {
        if self.path.exists() {
            std::fs::remove_file(&self.path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
