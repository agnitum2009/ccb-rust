use serde::{Deserialize, Serialize};

use crate::services::registry::{AgentRegistry, AgentRuntimeEntry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthInspection {
    pub generation: u32,
    pub daemon_alive: bool,
    pub socket_connectable: bool,
    pub agent_count: usize,
    pub healthy_count: usize,
    pub degraded_count: usize,
    pub failed_count: usize,
}

impl HealthInspection {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "generation": self.generation,
            "daemon_alive": self.daemon_alive,
            "socket_connectable": self.socket_connectable,
            "agent_count": self.agent_count,
            "healthy_count": self.healthy_count,
            "degraded_count": self.degraded_count,
            "failed_count": self.failed_count,
        })
    }
}

pub struct HealthMonitor {
    generation: u32,
    socket_path: Option<String>,
}

impl HealthMonitor {
    pub fn new(socket_path: Option<String>) -> Self {
        Self {
            generation: 1,
            socket_path,
        }
    }

    pub fn daemon_health(&self) -> HealthInspection {
        HealthInspection {
            generation: self.generation,
            daemon_alive: true,
            socket_connectable: self.socket_connectable(),
            agent_count: 0,
            healthy_count: 0,
            degraded_count: 0,
            failed_count: 0,
        }
    }

    pub fn inspect_registry(&self, registry: &AgentRegistry) -> HealthInspection {
        let mut healthy = 0;
        let mut degraded = 0;
        let mut failed = 0;
        for entry in registry.all_entries() {
            match entry.health.as_str() {
                "healthy" | "idle" | "ok" => healthy += 1,
                "degraded" | "stale" => degraded += 1,
                _ => failed += 1,
            }
        }
        HealthInspection {
            generation: self.generation,
            daemon_alive: true,
            socket_connectable: self.socket_connectable(),
            agent_count: registry.len(),
            healthy_count: healthy,
            degraded_count: degraded,
            failed_count: failed,
        }
    }

    pub fn classify_agent(&self, entry: &AgentRuntimeEntry) -> &'static str {
        match entry.health.as_str() {
            "healthy" | "idle" | "ok" => "healthy",
            "degraded" | "stale" => "degraded",
            _ => "failed",
        }
    }

    pub fn bump_generation(&mut self) {
        self.generation += 1;
    }

    fn socket_connectable(&self) -> bool {
        self.socket_path
            .as_ref()
            .is_some_and(|p| std::path::Path::new(p).exists())
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new(None)
    }
}
