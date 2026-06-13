use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryAction {
    pub agent_name: String,
    pub action: String,
    pub reason: String,
    pub timestamp: String,
}

pub struct RecoveryService {
    actions: Vec<RecoveryAction>,
}

impl RecoveryService {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    pub fn record_action(&mut self, action: RecoveryAction) {
        self.actions.push(action);
    }

    pub fn recent_actions(&self, limit: usize) -> &[RecoveryAction] {
        let start = self.actions.len().saturating_sub(limit);
        &self.actions[start..]
    }
}

impl Default for RecoveryService {
    fn default() -> Self {
        Self::new()
    }
}
