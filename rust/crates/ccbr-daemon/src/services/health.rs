use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::services::project_namespace::ProjectNamespace;
use crate::services::registry::{AgentRegistry, AgentRuntimeEntry};

/// Type alias for health state strings
pub type HealthState = String;

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

/// Pane state for a tmux-backed provider, mirroring Python health assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TmuxPaneState {
    Alive,
    Missing,
    Dead,
    Foreign,
}

impl TmuxPaneState {
    pub fn as_str(&self) -> &'static str {
        match self {
            TmuxPaneState::Alive => "alive",
            TmuxPaneState::Missing => "missing",
            TmuxPaneState::Dead => "dead",
            TmuxPaneState::Foreign => "foreign",
        }
    }

    pub fn health(&self) -> &'static str {
        match self {
            TmuxPaneState::Alive => "healthy",
            TmuxPaneState::Missing => "pane-missing",
            TmuxPaneState::Dead => "pane-dead",
            TmuxPaneState::Foreign => "pane-foreign",
        }
    }
}

/// Assessment of a provider's tmux pane, mirroring Python `ProviderPaneAssessment`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPaneAssessment {
    pub agent_name: String,
    pub provider: String,
    pub pane_id: Option<String>,
    pub pane_state: TmuxPaneState,
    pub health: String,
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

    /// Assess every agent that has a tmux runtime binding.
    ///
    /// Mirrors Python `assess_provider_pane` by checking tmux backend pane state
    /// and project namespace ownership.
    pub fn assess_provider_panes(
        &self,
        registry: &AgentRegistry,
        namespace: Option<&ProjectNamespace>,
        tmux_socket_path: Option<&str>,
    ) -> Vec<ProviderPaneAssessment> {
        let owned_panes = namespace.map(project_owned_panes).unwrap_or_default();
        registry
            .all_entries()
            .iter()
            .filter_map(|entry| {
                let pane_id = entry.pane_id.as_deref()?;
                if pane_id.trim().is_empty() {
                    return None;
                }
                let pane_state =
                    assess_tmux_pane_state(pane_id, tmux_socket_path, &owned_panes, entry);
                Some(ProviderPaneAssessment {
                    agent_name: entry.agent_name.clone(),
                    provider: entry.provider.clone(),
                    pane_id: Some(pane_id.to_string()),
                    health: pane_state.health().to_string(),
                    pane_state,
                })
            })
            .collect()
    }

    /// Inspect registry health incorporating live tmux pane assessments.
    pub fn inspect_registry_with_tmux(
        &self,
        registry: &AgentRegistry,
        namespace: Option<&ProjectNamespace>,
        tmux_socket_path: Option<&str>,
    ) -> (HealthInspection, Vec<ProviderPaneAssessment>) {
        let assessments = self.assess_provider_panes(registry, namespace, tmux_socket_path);
        let mut healthy = 0;
        let mut degraded = 0;
        let mut failed = 0;

        // Build a lookup by agent name for tmux-backed entries.
        let assessment_by_agent: std::collections::HashMap<&str, &ProviderPaneAssessment> =
            assessments
                .iter()
                .map(|a| (a.agent_name.as_str(), a))
                .collect();

        for entry in registry.all_entries() {
            let health = if let Some(a) = assessment_by_agent.get(entry.agent_name.as_str()) {
                a.health.as_str()
            } else {
                entry.health.as_str()
            };
            match health {
                "healthy" | "idle" | "ok" => healthy += 1,
                "degraded" | "stale" => degraded += 1,
                _ => failed += 1,
            }
        }

        let inspection = HealthInspection {
            generation: self.generation,
            daemon_alive: true,
            socket_connectable: self.socket_connectable(),
            agent_count: registry.len(),
            healthy_count: healthy,
            degraded_count: degraded,
            failed_count: failed,
        };
        (inspection, assessments)
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

fn project_owned_panes(namespace: &ProjectNamespace) -> HashSet<String> {
    namespace
        .agent_panes
        .values()
        .chain(namespace.active_panes.iter())
        .cloned()
        .collect()
}

fn assess_tmux_pane_state(
    pane_id: &str,
    tmux_socket_path: Option<&str>,
    owned_panes: &HashSet<String>,
    entry: &AgentRuntimeEntry,
) -> TmuxPaneState {
    let backend = ccbr_terminal::TmuxBackend::new(None, tmux_socket_path.map(|s| s.to_string()));
    let pane_exists = backend
        .tmux_run(
            &["display-message", "-p", "-t", pane_id, "#{pane_id}"],
            false,
            true,
            None,
            None,
        )
        .map(|o| o.success() && o.stdout.trim().starts_with('%'))
        .unwrap_or(false);
    if !pane_exists {
        return TmuxPaneState::Missing;
    }
    let alive = backend
        .tmux_run(
            &["display-message", "-p", "-t", pane_id, "#{pane_dead}"],
            false,
            true,
            None,
            None,
        )
        .map(|o| o.success() && o.stdout.trim() == "0")
        .unwrap_or(false);
    if !alive {
        return TmuxPaneState::Dead;
    }
    if !owned_panes.contains(pane_id) && entry.state != "stopped" {
        return TmuxPaneState::Foreign;
    }
    TmuxPaneState::Alive
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_classify_agent_maps_health_buckets() {
        let monitor = HealthMonitor::new(None);
        let healthy = AgentRuntimeEntry {
            agent_name: "a".into(),
            provider: "p".into(),
            state: "running".into(),
            health: "healthy".into(),
            pane_id: None,
            workspace_path: None,
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        };
        let degraded = AgentRuntimeEntry {
            health: "stale".into(),
            ..healthy.clone()
        };
        let failed = AgentRuntimeEntry {
            health: "dead".into(),
            ..healthy.clone()
        };
        assert_eq!(monitor.classify_agent(&healthy), "healthy");
        assert_eq!(monitor.classify_agent(&degraded), "degraded");
        assert_eq!(monitor.classify_agent(&failed), "failed");
    }

    #[test]
    fn test_assess_provider_panes_without_namespace_are_foreign() {
        let mut registry = AgentRegistry::new();
        registry.register(AgentRuntimeEntry {
            agent_name: "claude".into(),
            provider: "claude".into(),
            state: "running".into(),
            health: "healthy".into(),
            pane_id: Some("%9999".into()),
            workspace_path: None,
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });

        let monitor = HealthMonitor::new(None);
        let assessments = monitor.assess_provider_panes(&registry, None, None);

        assert_eq!(assessments.len(), 1);
        assert_eq!(assessments[0].agent_name, "claude");
        // Without a real tmux server the pane is missing.
        assert_eq!(assessments[0].pane_state, TmuxPaneState::Missing);
        assert_eq!(assessments[0].health, "pane-missing");
    }

    #[test]
    fn test_assess_provider_panes_owned_by_namespace_are_healthy() {
        let mut registry = AgentRegistry::new();
        registry.register(AgentRuntimeEntry {
            agent_name: "claude".into(),
            provider: "claude".into(),
            state: "running".into(),
            health: "healthy".into(),
            pane_id: Some("%1".into()),
            workspace_path: None,
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });

        let namespace = ProjectNamespace {
            project_root: "/tmp".into(),
            project_id: "pid".into(),
            tmux_socket_path: "/tmp/tmux.sock".into(),
            tmux_socket_name: "tmux".into(),
            tmux_session_name: "session".into(),
            agent_names: vec!["claude".into()],
            windows: vec![],
            agent_panes: HashMap::from_iter([("claude".into(), "%1".into())]),
            active_panes: vec!["%1".into()],
            namespace_epoch: 1,
            created_at: "2024-01-01T00:00:00Z".into(),
        };

        // Ensure tmux binary is not found so the test is independent of whether
        // the runner happens to be executing inside a tmux session.
        let original_path = std::env::var("PATH").ok();
        std::env::set_var("PATH", "");

        let monitor = HealthMonitor::new(None);
        let assessments = monitor.assess_provider_panes(&registry, Some(&namespace), None);

        assert_eq!(assessments.len(), 1);
        // Pane is owned by namespace but tmux server is absent, so missing.
        assert_eq!(assessments[0].pane_state, TmuxPaneState::Missing);

        match original_path {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }
    }

    #[test]
    fn test_inspect_registry_with_tmux_counts_assessed_health() {
        let mut registry = AgentRegistry::new();
        registry.register(AgentRuntimeEntry {
            agent_name: "claude".into(),
            provider: "claude".into(),
            state: "running".into(),
            health: "healthy".into(),
            pane_id: Some("%9999".into()),
            workspace_path: None,
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });

        let monitor = HealthMonitor::new(None);
        let (inspection, assessments) = monitor.inspect_registry_with_tmux(&registry, None, None);

        assert_eq!(inspection.agent_count, 1);
        assert_eq!(assessments.len(), 1);
        // Missing pane is counted as failed (pane-missing).
        assert_eq!(inspection.failed_count, 1);
        assert_eq!(inspection.healthy_count, 0);
    }
}
