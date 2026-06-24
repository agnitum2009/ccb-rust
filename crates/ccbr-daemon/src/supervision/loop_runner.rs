pub struct SupervisionLoop {
    store: crate::supervision::store::SupervisionStore,
    poll_interval_ms: u64,
    max_retries: u32,
}

impl SupervisionLoop {
    pub fn new(poll_interval_ms: u64, max_retries: u32) -> Self {
        Self {
            store: crate::supervision::store::SupervisionStore::new(),
            poll_interval_ms,
            max_retries,
        }
    }

    pub fn store(&self) -> &crate::supervision::store::SupervisionStore {
        &self.store
    }
    pub fn store_mut(&mut self) -> &mut crate::supervision::store::SupervisionStore {
        &mut self.store
    }
    #[allow(dead_code)]
    pub fn poll_interval_ms(&self) -> u64 {
        self.poll_interval_ms
    }
    #[allow(dead_code)]
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    pub fn tick(&mut self, agents: &[String]) -> Vec<String> {
        let mut needs_restart = Vec::new();
        for agent_name in agents {
            if self.store.should_restart(agent_name) {
                needs_restart.push(agent_name.clone());
            }
        }
        needs_restart
    }

    pub fn record_restart(&mut self, agent_name: &str, reason: &str) {
        self.store.record_restart(agent_name, reason);
    }

    pub fn record_success(&mut self, agent_name: &str) {
        self.store.record_success(agent_name);
    }
}
