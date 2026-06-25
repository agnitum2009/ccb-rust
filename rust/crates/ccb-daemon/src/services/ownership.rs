use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl OwnershipService {
    pub fn new() -> Self {
        Self {
            ownership: None,
            last_generation: 0,
        }
    }

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
