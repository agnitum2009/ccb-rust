use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

/// Persisted ownership record for the CCBR daemon.
///
/// Mirrors the fields captured by the Python `OwnershipGuard` / `MountManager`
/// lease so that runtime mount ownership survives daemon restarts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnershipRecord {
    pub owner_pid: u32,
    pub socket_path: String,
    pub instance_id: String,
    pub acquired_at: String,
    pub generation: u32,
}

pub struct OwnershipService {
    ownership: Option<OwnershipRecord>,
    last_generation: u32,
    state_path: Option<Utf8PathBuf>,
}

impl OwnershipService {
    pub fn new() -> Self {
        Self {
            ownership: None,
            last_generation: 0,
            state_path: None,
        }
    }

    /// Build a service bound to a durable state file.
    pub fn with_state_path(path: impl Into<Utf8PathBuf>) -> Self {
        Self {
            ownership: None,
            last_generation: 0,
            state_path: Some(path.into()),
        }
    }

    /// Configure the durable state path after construction.
    pub fn set_state_path(&mut self, path: impl Into<Utf8PathBuf>) {
        self.state_path = Some(path.into());
    }

    /// Acquire ownership, bumping the generation.
    pub fn acquire(&mut self, pid: u32, socket_path: &str, instance_id: &str) -> OwnershipRecord {
        let generation = self.last_generation + 1;
        let record = OwnershipRecord {
            owner_pid: pid,
            socket_path: socket_path.into(),
            instance_id: instance_id.into(),
            acquired_at: chrono::Utc::now().to_rfc3339(),
            generation,
        };
        self.last_generation = generation;
        self.ownership = Some(record.clone());
        record
    }

    /// Load any previously persisted ownership record and update the internal
    /// generation counter so the next acquire continues the sequence.
    pub fn load(&mut self) -> crate::Result<Option<OwnershipRecord>> {
        let Some(path) = self.state_path.as_ref() else {
            return Ok(None);
        };
        if !path.exists() {
            return Ok(None);
        }
        let record: OwnershipRecord = ccbr_storage::json::JsonStore::new().load(path)?;
        self.last_generation = record.generation;
        self.ownership = Some(record.clone());
        Ok(Some(record))
    }

    /// Persist the current ownership record, if any, to disk.
    pub fn save(&self) -> crate::Result<()> {
        let Some(path) = self.state_path.as_ref() else {
            return Ok(());
        };
        if let Some(record) = self.ownership.as_ref() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            ccbr_storage::json::JsonStore::new().save(path, record)?;
        }
        Ok(())
    }

    /// Restore an existing ownership record if it describes the same holder;
    /// otherwise acquire fresh ownership.
    ///
    /// Idempotency: when the persisted record matches the requested
    /// `socket_path` and `instance_id`, the existing record is reused instead
    /// of creating a duplicate guard.
    pub fn restore_or_acquire(
        &mut self,
        pid: u32,
        socket_path: &str,
        instance_id: &str,
    ) -> crate::Result<OwnershipRecord> {
        self.load()?;
        if let Some(current) = self.ownership.as_ref() {
            if current.socket_path == socket_path && current.instance_id == instance_id {
                return Ok(current.clone());
            }
        }
        Ok(self.acquire(pid, socket_path, instance_id))
    }

    pub fn release(&mut self) {
        self.ownership = None;
    }

    pub fn current(&self) -> Option<&OwnershipRecord> {
        self.ownership.as_ref()
    }

    pub fn is_owner(&self, pid: u32) -> bool {
        self.ownership.as_ref().is_some_and(|o| o.owner_pid == pid)
    }
}

impl Default for OwnershipService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;
    use tempfile::TempDir;

    #[test]
    fn test_save_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("ownership-state.json");
        let path = Utf8Path::from_path(&file).unwrap();

        let mut service = OwnershipService::with_state_path(path);
        let record = service.acquire(1234, "/tmp/ccbr.sock", "instance-a");
        service.save().unwrap();

        let mut loaded = OwnershipService::with_state_path(path);
        let restored = loaded.load().unwrap();
        assert_eq!(restored, Some(record));
        assert_eq!(loaded.current().unwrap().generation, 1);
    }

    #[test]
    fn test_restore_or_acquire_reuses_same_holder() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("ownership-state.json");
        let path = Utf8Path::from_path(&file).unwrap();

        let mut service = OwnershipService::with_state_path(path);
        let first = service.acquire(1234, "/tmp/ccbr.sock", "instance-a");
        service.save().unwrap();

        let mut restarted = OwnershipService::with_state_path(path);
        let second = restarted
            .restore_or_acquire(1234, "/tmp/ccbr.sock", "instance-a")
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(second.generation, 1);
    }

    #[test]
    fn test_restore_or_acquire_bumps_generation_for_new_holder() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("ownership-state.json");
        let path = Utf8Path::from_path(&file).unwrap();

        let mut service = OwnershipService::with_state_path(path);
        service.acquire(1234, "/tmp/ccbr.sock", "instance-a");
        service.save().unwrap();

        let mut restarted = OwnershipService::with_state_path(path);
        let second = restarted
            .restore_or_acquire(5678, "/tmp/ccbr.sock", "instance-b")
            .unwrap();
        assert_eq!(second.generation, 2);
        assert_eq!(second.owner_pid, 5678);
    }
}
