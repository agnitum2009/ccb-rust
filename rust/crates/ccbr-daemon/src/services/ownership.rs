use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipRecord {
    pub owner_pid: u32,
    pub socket_path: String,
    pub acquired_at: String,
    pub generation: u32,
}

pub struct OwnershipService {
    ownership: Option<OwnershipRecord>,
}

impl OwnershipService {
    pub fn new() -> Self {
        Self { ownership: None }
    }

    pub fn acquire(&mut self, pid: u32, socket_path: &str) -> OwnershipRecord {
        let record = OwnershipRecord {
            owner_pid: pid,
            socket_path: socket_path.into(),
            acquired_at: chrono::Utc::now().to_rfc3339(),
            generation: self.ownership.as_ref().map_or(1, |o| o.generation + 1),
        };
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
