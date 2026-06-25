use crate::services::health::{HealthMonitor, TmuxPaneState};
use crate::services::project_namespace::ProjectNamespace;
use crate::services::registry::AgentRegistry;
use ccb_terminal::TmuxBackend;

/// Health state of a single agent as observed by the supervision loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentHealth {
    Healthy,
    PaneDead,
    PaneMissing,
    SessionMissing,
    Unhealthy { reason: String },
}

impl AgentHealth {
    fn reason(&self) -> &str {
        match self {
            AgentHealth::Healthy => "healthy",
            AgentHealth::PaneDead => "pane-dead",
            AgentHealth::PaneMissing => "pane-missing",
            AgentHealth::SessionMissing => "session-missing",
            AgentHealth::Unhealthy { reason } => reason.as_str(),
        }
    }
}

/// Abstraction over agent health inspection so the supervision loop can be
/// tested without a live tmux server.
pub trait HealthChecker {
    fn check(&self, agent_name: &str) -> AgentHealth;
}

/// Decision produced by one supervision tick for a single agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisionDecision {
    Restart { agent_name: String, reason: String },
    Escalate { agent_name: String, reason: String },
}

/// Health checker backed by live tmux state and the daemon registry.
pub struct TmuxHealthChecker<'a> {
    registry: &'a AgentRegistry,
    namespace: Option<&'a ProjectNamespace>,
    socket_path: &'a str,
}

impl<'a> TmuxHealthChecker<'a> {
    pub fn new(
        registry: &'a AgentRegistry,
        namespace: Option<&'a ProjectNamespace>,
        socket_path: &'a str,
    ) -> Self {
        Self {
            registry,
            namespace,
            socket_path,
        }
    }
}

impl HealthChecker for TmuxHealthChecker<'_> {
    fn check(&self, agent_name: &str) -> AgentHealth {
        let Some(entry) = self.registry.get(agent_name) else {
            return AgentHealth::Unhealthy {
                reason: "not-registered".into(),
            };
        };

        // Registry-level degraded/failed states require recovery.
        match entry.health.as_str() {
            "degraded" | "failed" | "error" | "stopped" => {
                return AgentHealth::Unhealthy {
                    reason: entry.health.clone(),
                };
            }
            _ => {}
        }

        if entry.pane_id.as_deref().unwrap_or("").trim().is_empty() {
            // No pane binding yet; if the agent is supposed to be running
            // this is treated as a missing session that needs mounting.
            return AgentHealth::SessionMissing;
        }

        // Without a reachable tmux socket the whole session is missing.
        if self.socket_path.is_empty() || !std::path::Path::new(self.socket_path).exists() {
            return AgentHealth::SessionMissing;
        }

        // Check that the project namespace session still exists.
        let session_name = self
            .namespace
            .map(|n| n.tmux_session_name.as_str())
            .unwrap_or("");
        if !session_name.is_empty() {
            let backend = TmuxBackend::new(None, Some(self.socket_path.to_string()));
            let alive = backend
                .tmux_run(
                    &["has-session", "-t", session_name],
                    false,
                    false,
                    None,
                    None,
                )
                .map(|o| o.success())
                .unwrap_or(false);
            if !alive {
                return AgentHealth::SessionMissing;
            }
        }

        // Assess the specific pane using the existing health monitor.
        let monitor = HealthMonitor::new(Some(self.socket_path.to_string()));
        let assessments =
            monitor.assess_provider_panes(self.registry, self.namespace, Some(self.socket_path));
        if let Some(a) = assessments.iter().find(|a| a.agent_name == agent_name) {
            match a.pane_state {
                TmuxPaneState::Dead => AgentHealth::PaneDead,
                TmuxPaneState::Missing => AgentHealth::PaneMissing,
                TmuxPaneState::Foreign => AgentHealth::Unhealthy {
                    reason: "pane-foreign".into(),
                },
                TmuxPaneState::Alive => AgentHealth::Healthy,
            }
        } else {
            AgentHealth::Healthy
        }
    }
}

pub struct SupervisionLoop {
    store: crate::supervision::store::SupervisionStore,
    poll_interval_ms: u64,
    max_retries: u32,
    base_backoff_seconds: u32,
    max_backoff_seconds: u32,
}

impl SupervisionLoop {
    pub fn new(poll_interval_ms: u64, max_retries: u32) -> Self {
        Self {
            store: crate::supervision::store::SupervisionStore::new(),
            poll_interval_ms,
            max_retries,
            base_backoff_seconds: 1,
            max_backoff_seconds: 300,
        }
    }

    pub fn with_backoff(
        poll_interval_ms: u64,
        max_retries: u32,
        base_backoff_seconds: u32,
        max_backoff_seconds: u32,
    ) -> Self {
        Self {
            store: crate::supervision::store::SupervisionStore::new(),
            poll_interval_ms,
            max_retries,
            base_backoff_seconds,
            max_backoff_seconds,
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

    /// Run one supervision pass over the supplied agents.
    ///
    /// Healthy agents reset their backoff/retry state. Unhealthy agents produce
    /// a `Restart` decision when retries remain and backoff has expired, or an
    /// `Escalate` decision once max retries have been exhausted.
    pub fn tick(
        &mut self,
        agents: &[String],
        checker: &dyn HealthChecker,
    ) -> Vec<SupervisionDecision> {
        let now = chrono::Utc::now();
        let mut decisions = Vec::new();
        for agent_name in agents {
            let health = checker.check(agent_name);
            if health == AgentHealth::Healthy {
                self.store.record_success(agent_name);
                continue;
            }
            let reason = health.reason().to_string();
            let record = self.store.get(agent_name);
            let retries_exhausted = record.is_some_and(|r| r.restart_count >= self.max_retries);
            let already_escalated = record.is_some_and(|r| r.escalated || r.state == "escalated");

            if self.store.can_restart(agent_name, self.max_retries, &now) {
                decisions.push(SupervisionDecision::Restart {
                    agent_name: agent_name.clone(),
                    reason,
                });
            } else if !already_escalated && retries_exhausted {
                // Escalate only once per agent; repeated ticks skip already
                // escalated records so the failure is recorded but we do not
                // spin in an infinite restart loop.
                self.store.record_escalation(agent_name, &reason);
                decisions.push(SupervisionDecision::Escalate {
                    agent_name: agent_name.clone(),
                    reason,
                });
            }
        }
        decisions
    }

    pub fn record_restart(&mut self, agent_name: &str, reason: &str) {
        self.store.record_restart(
            agent_name,
            reason,
            self.base_backoff_seconds,
            self.max_backoff_seconds,
        );
    }

    pub fn record_success(&mut self, agent_name: &str) {
        self.store.record_success(agent_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FakeChecker {
        states: HashMap<String, AgentHealth>,
    }

    impl HealthChecker for FakeChecker {
        fn check(&self, agent_name: &str) -> AgentHealth {
            self.states
                .get(agent_name)
                .cloned()
                .unwrap_or(AgentHealth::Healthy)
        }
    }

    #[test]
    fn test_healthy_agent_does_not_trigger_restart() {
        let mut loop_ = SupervisionLoop::new(1000, 5);
        let checker = FakeChecker {
            states: HashMap::from([("a".into(), AgentHealth::Healthy)]),
        };
        let decisions = loop_.tick(&["a".into()], &checker);
        assert!(decisions.is_empty());
    }

    #[test]
    fn test_dead_pane_triggers_restart() {
        let mut loop_ = SupervisionLoop::new(1000, 5);
        let checker = FakeChecker {
            states: HashMap::from([("a".into(), AgentHealth::PaneDead)]),
        };
        let decisions = loop_.tick(&["a".into()], &checker);
        assert_eq!(decisions.len(), 1);
        assert_eq!(
            decisions[0],
            SupervisionDecision::Restart {
                agent_name: "a".into(),
                reason: "pane-dead".into(),
            }
        );
    }

    #[test]
    fn test_missing_session_triggers_restart() {
        let mut loop_ = SupervisionLoop::new(1000, 5);
        let checker = FakeChecker {
            states: HashMap::from([("a".into(), AgentHealth::SessionMissing)]),
        };
        let decisions = loop_.tick(&["a".into()], &checker);
        assert_eq!(decisions.len(), 1);
        assert_eq!(
            decisions[0],
            SupervisionDecision::Restart {
                agent_name: "a".into(),
                reason: "session-missing".into(),
            }
        );
    }

    #[test]
    fn test_restart_respects_backoff() {
        // Use the default base/max backoff so the first restart creates a
        // non-zero backoff window.
        let mut loop_ = SupervisionLoop::new(1000, 5);
        let checker = FakeChecker {
            states: HashMap::from([("a".into(), AgentHealth::PaneDead)]),
        };
        // First tick triggers restart; record it immediately.
        let _ = loop_.tick(&["a".into()], &checker);
        loop_.record_restart("a", "pane-dead");

        // Second tick while still inside the backoff window is suppressed.
        let decisions = loop_.tick(&["a".into()], &checker);
        assert!(
            decisions.is_empty(),
            "backoff should suppress second restart"
        );
    }

    #[test]
    fn test_max_retries_escalate_once() {
        // Disable backoff so retries can be exhausted immediately in this test.
        let mut loop_ = SupervisionLoop::with_backoff(1000, 2, 0, 0);
        let checker = FakeChecker {
            states: HashMap::from([("a".into(), AgentHealth::PaneDead)]),
        };

        // Exhaust retries quickly by faking immediate restarts.
        for _ in 0..2 {
            let decisions = loop_.tick(&["a".into()], &checker);
            assert!(
                matches!(decisions.first(), Some(SupervisionDecision::Restart { .. })),
                "expected restart decision"
            );
            loop_.record_restart("a", "pane-dead");
        }

        // Next tick should escalate, not restart, and should only escalate once.
        let decisions = loop_.tick(&["a".into()], &checker);
        assert_eq!(decisions.len(), 1);
        assert!(
            matches!(
                &decisions[0],
                SupervisionDecision::Escalate { agent_name, .. } if agent_name == "a"
            ),
            "expected escalation after max retries"
        );

        // Repeated ticks must not produce further decisions.
        let decisions = loop_.tick(&["a".into()], &checker);
        assert!(
            decisions.is_empty(),
            "escalated agent should not produce repeated decisions"
        );
    }
}
