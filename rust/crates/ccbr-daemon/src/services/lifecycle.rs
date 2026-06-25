use crate::models::lifecycle::{build_lifecycle, CcbdLifecycle, CcbdLifecycleUpdates};
use ccbr_storage::json::JsonStore;
use ccbr_storage::paths::PathLayout;

/// Persisted lifecycle store for the CCBR daemon.
///
/// Mirrors Python `ccbd.services.lifecycle.CcbdLifecycleStore`. Records are
/// saved to `.ccbr/ccbrd/lifecycle.json` and carry the daemon phase, startup
/// progress, and shared startup deadline used by the CLI wait logic.
#[derive(Clone)]
pub struct LifecycleStore {
    layout: PathLayout,
    store: JsonStore,
}

impl LifecycleStore {
    pub fn new(layout: PathLayout) -> Self {
        Self {
            layout,
            store: JsonStore::new(),
        }
    }

    pub fn with_store(layout: PathLayout, store: JsonStore) -> Self {
        Self { layout, store }
    }

    pub fn load(&self) -> Option<CcbdLifecycle> {
        let path = self.layout.ccbrd_lifecycle_path();
        if !path.exists() {
            return None;
        }
        let value = self.store.load::<serde_json::Value>(&path).ok()?;
        CcbdLifecycle::from_record(value)
    }

    pub fn save(&self, lifecycle: &CcbdLifecycle) -> ccbr_storage::Result<()> {
        let path = self.layout.ccbrd_lifecycle_path();
        self.store.save(&path, &lifecycle.to_record())
    }

    /// Build an initial "unmounted" lifecycle record for `project_id`.
    pub fn build_default(
        &self,
        project_id: impl Into<String>,
        occurred_at: impl Into<String>,
    ) -> CcbdLifecycle {
        build_lifecycle(
            project_id,
            occurred_at,
            LIFECYCLE_DESIRED_STATE_STOPPED,
            LIFECYCLE_PHASE_UNMOUNTED,
            0,
            CcbdLifecycleUpdates::default(),
        )
    }
}

pub const LIFECYCLE_DESIRED_STATE_RUNNING: &str = "running";
pub const LIFECYCLE_DESIRED_STATE_STOPPED: &str = "stopped";

pub const LIFECYCLE_PHASE_UNMOUNTED: &str = "unmounted";
pub const LIFECYCLE_PHASE_STARTING: &str = "starting";
pub const LIFECYCLE_PHASE_MOUNTED: &str = "mounted";
pub const LIFECYCLE_PHASE_STOPPING: &str = "stopping";
pub const LIFECYCLE_PHASE_FAILED: &str = "failed";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::lifecycle::CcbdLifecycleUpdates;
    use tempfile::TempDir;

    #[test]
    fn lifecycle_store_roundtrip_preserves_startup_progress_fields() {
        let dir = TempDir::new().unwrap();
        let layout = PathLayout::new(dir.path().to_str().unwrap());
        let store = LifecycleStore::new(layout.clone());

        let lifecycle = build_lifecycle(
            "proj-1",
            "2026-04-24T00:00:00Z",
            LIFECYCLE_DESIRED_STATE_RUNNING,
            LIFECYCLE_PHASE_STARTING,
            3,
            CcbdLifecycleUpdates {
                startup_id: Some(Some("startup-123".into())),
                startup_stage: Some(Some("socket_listening".into())),
                last_progress_at: Some(Some("2026-04-24T00:00:04Z".into())),
                startup_deadline_at: Some(Some("2026-04-24T00:00:20Z".into())),
                keeper_pid: Some(Some(111)),
                socket_path: Some(Some(layout.ccbrd_socket_path().to_string())),
                ..Default::default()
            },
        );

        store.save(&lifecycle).unwrap();
        let loaded = store.load().expect("lifecycle should load");
        assert_eq!(loaded, lifecycle);
    }

    #[test]
    fn lifecycle_store_load_missing_returns_none() {
        let dir = TempDir::new().unwrap();
        let layout = PathLayout::new(dir.path().to_str().unwrap());
        let store = LifecycleStore::new(layout);
        assert!(store.load().is_none());
    }
}
