use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountEvent {
    pub agent_name: String,
    pub event_type: String,
    pub timestamp: String,
    pub details: serde_json::Value,
}

pub struct SupervisionMountService {
    events: Vec<MountEvent>,
}

impl SupervisionMountService {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn record_event(&mut self, event: MountEvent) {
        self.events.push(event);
    }

    pub fn recent_events(&self, limit: usize) -> &[MountEvent] {
        let start = self.events.len().saturating_sub(limit);
        &self.events[start..]
    }
}

impl Default for SupervisionMountService {
    fn default() -> Self {
        Self::new()
    }
}
