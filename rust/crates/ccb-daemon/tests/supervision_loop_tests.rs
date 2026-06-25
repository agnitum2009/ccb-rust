//! Integration tests for the runtime supervision loop.

use ccb_daemon::supervision::loop_runner::{
    AgentHealth, HealthChecker, SupervisionDecision, SupervisionLoop,
};
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
        states: HashMap::from([("agent1".into(), AgentHealth::Healthy)]),
    };

    let decisions = loop_.tick(&["agent1".into()], &checker);

    assert!(decisions.is_empty());
}

#[test]
fn test_dead_pane_triggers_respawn_with_backoff() {
    let mut loop_ = SupervisionLoop::new(1000, 5);
    let checker = FakeChecker {
        states: HashMap::from([("agent1".into(), AgentHealth::PaneDead)]),
    };

    // First tick decides a restart.
    let decisions = loop_.tick(&["agent1".into()], &checker);
    assert_eq!(decisions.len(), 1);
    assert!(
        matches!(
            &decisions[0],
            SupervisionDecision::Restart { agent_name, reason } if agent_name == "agent1" && reason == "pane-dead"
        ),
        "expected pane-dead restart decision, got {:?}",
        decisions
    );

    // Simulate the restart being applied.
    loop_.record_restart("agent1", "pane-dead");

    // Immediate re-tick is suppressed by backoff.
    let decisions = loop_.tick(&["agent1".into()], &checker);
    assert!(
        decisions.is_empty(),
        "backoff must suppress repeated restart decisions: {:?}",
        decisions
    );
}

#[test]
fn test_missing_session_triggers_restart() {
    let mut loop_ = SupervisionLoop::new(1000, 5);
    let checker = FakeChecker {
        states: HashMap::from([("agent1".into(), AgentHealth::SessionMissing)]),
    };

    let decisions = loop_.tick(&["agent1".into()], &checker);

    assert_eq!(decisions.len(), 1);
    assert!(matches!(
        &decisions[0],
        SupervisionDecision::Restart { agent_name, reason } if agent_name == "agent1" && reason == "session-missing"
    ));
}

#[test]
fn test_max_retries_escalate_without_infinite_loop() {
    // Disable backoff so retries can be exhausted immediately.
    let mut loop_ = SupervisionLoop::with_backoff(1000, 2, 0, 0);
    let checker = FakeChecker {
        states: HashMap::from([("agent1".into(), AgentHealth::PaneMissing)]),
    };

    for _ in 0..2 {
        let decisions = loop_.tick(&["agent1".into()], &checker);
        assert!(
            matches!(&decisions[0], SupervisionDecision::Restart { .. }),
            "expected restart before max retries"
        );
        loop_.record_restart("agent1", "pane-missing");
    }

    // After max retries the next tick escalates exactly once.
    let decisions = loop_.tick(&["agent1".into()], &checker);
    assert_eq!(decisions.len(), 1);
    assert!(
        matches!(
            &decisions[0],
            SupervisionDecision::Escalate { agent_name, .. } if agent_name == "agent1"
        ),
        "expected escalation after max retries"
    );

    // Subsequent ticks must not keep restarting or re-escalating.
    for _ in 0..5 {
        let decisions = loop_.tick(&["agent1".into()], &checker);
        assert!(
            decisions.is_empty(),
            "escalated agent must not produce further decisions: {:?}",
            decisions
        );
    }
}
