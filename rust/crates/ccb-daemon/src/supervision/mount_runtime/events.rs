//! Mirrors Python `lib/ccbd/supervision/mount_runtime/events.py`.
//!
//! Records mount lifecycle events to a `MountEventStore`. The event shape mirrors the
//! Python `SupervisionEvent` dataclass so that persisted records remain compatible.

use ccb_agents::models::AgentRuntime;
use serde::{Deserialize, Serialize};

/// A supervision event emitted during mount / recovery flows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisionEvent {
    pub event_kind: String,
    pub project_id: String,
    pub agent_name: String,
    pub occurred_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_generation: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desired_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconcile_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior_health: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_health: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_ref: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub details: serde_json::Map<String, serde_json::Value>,
}

impl SupervisionEvent {
    /// Serialize to a JSON record matching the Python `to_record()` layout.
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "record_type": "ccbd_supervision_event",
            "event_kind": self.event_kind,
            "project_id": self.project_id,
            "agent_name": self.agent_name,
            "occurred_at": self.occurred_at,
            "daemon_generation": self.daemon_generation,
            "desired_state": self.desired_state,
            "reconcile_state": self.reconcile_state,
            "prior_health": self.prior_health,
            "result_health": self.result_health,
            "runtime_state": self.runtime_state,
            "runtime_ref": self.runtime_ref,
            "session_ref": self.session_ref,
            "details": self.details,
        })
    }
}

/// Event sink used by mount orchestration.
pub trait MountEventStore {
    /// Append a single supervision event.
    fn append(&self, event: SupervisionEvent);
}

/// Record that a mount attempt started.
pub fn record_mount_started(
    event_store: &dyn MountEventStore,
    project_id: &str,
    agent_name: &str,
    attempted_at: &str,
    prior_health: &str,
    runtime: &AgentRuntime,
) {
    let mut details = serde_json::Map::new();
    if let Some(id) = runtime.mount_attempt_id.as_deref() {
        details.insert(
            "mount_attempt_id".to_string(),
            serde_json::Value::String(id.to_string()),
        );
    }
    event_store.append(SupervisionEvent {
        event_kind: "mount_started".to_string(),
        project_id: project_id.to_string(),
        agent_name: agent_name.to_string(),
        occurred_at: attempted_at.to_string(),
        daemon_generation: runtime.daemon_generation,
        desired_state: runtime.desired_state.clone(),
        reconcile_state: runtime.reconcile_state.clone(),
        prior_health: Some(prior_health.to_string()),
        result_health: Some(runtime.health.clone()),
        runtime_state: Some(runtime_state_str(runtime)),
        runtime_ref: runtime.runtime_ref.clone(),
        session_ref: runtime.session_ref.clone(),
        details,
    });
}

/// Record that a mount attempt failed.
pub fn record_mount_failed(
    event_store: &dyn MountEventStore,
    project_id: &str,
    agent_name: &str,
    attempted_at: &str,
    prior_health: &str,
    runtime: &AgentRuntime,
    reason: &str,
) {
    let mut details = serde_json::Map::new();
    details.insert(
        "reason".to_string(),
        serde_json::Value::String(reason.to_string()),
    );
    event_store.append(SupervisionEvent {
        event_kind: "mount_failed".to_string(),
        project_id: project_id.to_string(),
        agent_name: agent_name.to_string(),
        occurred_at: attempted_at.to_string(),
        daemon_generation: runtime.daemon_generation,
        desired_state: runtime.desired_state.clone(),
        reconcile_state: runtime.reconcile_state.clone(),
        prior_health: Some(prior_health.to_string()),
        result_health: Some(runtime.health.clone()),
        runtime_state: Some(runtime_state_str(runtime)),
        runtime_ref: runtime.runtime_ref.clone(),
        session_ref: runtime.session_ref.clone(),
        details,
    });
}

/// Record that a mount attempt was superseded by a concurrent operation.
pub fn record_mount_superseded(
    event_store: &dyn MountEventStore,
    project_id: &str,
    agent_name: &str,
    attempted_at: &str,
    prior_health: &str,
    runtime: &AgentRuntime,
    attempt_id: &str,
) {
    let mut details = serde_json::Map::new();
    details.insert(
        "mount_attempt_id".to_string(),
        serde_json::Value::String(attempt_id.to_string()),
    );
    event_store.append(SupervisionEvent {
        event_kind: "mount_superseded".to_string(),
        project_id: project_id.to_string(),
        agent_name: agent_name.to_string(),
        occurred_at: attempted_at.to_string(),
        daemon_generation: runtime.daemon_generation,
        desired_state: runtime.desired_state.clone(),
        reconcile_state: runtime.reconcile_state.clone(),
        prior_health: Some(prior_health.to_string()),
        result_health: Some(runtime.health.clone()),
        runtime_state: Some(runtime_state_str(runtime)),
        runtime_ref: runtime.runtime_ref.clone(),
        session_ref: runtime.session_ref.clone(),
        details,
    });
}

/// Record that a mount attempt succeeded.
pub fn record_mount_succeeded(
    event_store: &dyn MountEventStore,
    project_id: &str,
    agent_name: &str,
    attempted_at: &str,
    prior_health: &str,
    runtime: &AgentRuntime,
) {
    let mut details = serde_json::Map::new();
    details.insert(
        "restart_count".to_string(),
        serde_json::Value::Number(runtime.restart_count.into()),
    );
    event_store.append(SupervisionEvent {
        event_kind: "mount_succeeded".to_string(),
        project_id: project_id.to_string(),
        agent_name: agent_name.to_string(),
        occurred_at: attempted_at.to_string(),
        daemon_generation: runtime.daemon_generation,
        desired_state: runtime.desired_state.clone(),
        reconcile_state: runtime.reconcile_state.clone(),
        prior_health: Some(prior_health.to_string()),
        result_health: Some(runtime.health.clone()),
        runtime_state: Some(runtime_state_str(runtime)),
        runtime_ref: runtime.runtime_ref.clone(),
        session_ref: runtime.session_ref.clone(),
        details,
    });
}

fn runtime_state_str(runtime: &AgentRuntime) -> String {
    serde_json::to_value(runtime.state)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| format!("{:?}", runtime.state).to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccb_agents::models::AgentState;

    fn test_runtime() -> AgentRuntime {
        AgentRuntime {
            agent_name: "claude".to_string(),
            state: AgentState::Starting,
            health: "starting".to_string(),
            project_id: "p1".to_string(),
            backend_type: "pane-backed".to_string(),
            queue_depth: 0,
            mount_attempt_id: Some("attempt-1".to_string()),
            daemon_generation: Some(7),
            desired_state: Some("mounted".to_string()),
            reconcile_state: Some("starting".to_string()),
            runtime_ref: Some("ref-1".to_string()),
            session_ref: Some("sess-1".to_string()),
            restart_count: 3,
            ..AgentRuntime::default()
        }
    }

    struct VecEventStore {
        events: std::cell::RefCell<Vec<SupervisionEvent>>,
    }

    impl Default for VecEventStore {
        fn default() -> Self {
            Self {
                events: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl MountEventStore for VecEventStore {
        fn append(&self, event: SupervisionEvent) {
            self.events.borrow_mut().push(event);
        }
    }

    #[test]
    fn test_record_mount_started() {
        let store = VecEventStore::default();
        let runtime = test_runtime();
        record_mount_started(
            &store,
            "p1",
            "claude",
            "2024-01-01T00:00:00Z",
            "unmounted",
            &runtime,
        );
        assert_eq!(store.events.borrow().len(), 1);
        let event = &store.events.borrow()[0];
        assert_eq!(event.event_kind, "mount_started");
        assert_eq!(event.project_id, "p1");
        assert_eq!(event.agent_name, "claude");
        assert_eq!(event.prior_health, Some("unmounted".to_string()));
        assert_eq!(event.result_health, Some("starting".to_string()));
        assert_eq!(event.runtime_state, Some("starting".to_string()));
        assert_eq!(
            event.details.get("mount_attempt_id").unwrap(),
            &serde_json::Value::String("attempt-1".to_string())
        );
    }

    #[test]
    fn test_record_mount_failed() {
        let store = VecEventStore::default();
        let runtime = test_runtime();
        record_mount_failed(
            &store,
            "p1",
            "claude",
            "2024-01-01T00:00:00Z",
            "unmounted",
            &runtime,
            "boom",
        );
        assert_eq!(store.events.borrow().len(), 1);
        let event = &store.events.borrow()[0];
        assert_eq!(event.event_kind, "mount_failed");
        assert_eq!(
            event.details.get("reason").unwrap(),
            &serde_json::Value::String("boom".to_string())
        );
    }

    #[test]
    fn test_record_mount_superseded() {
        let store = VecEventStore::default();
        let runtime = test_runtime();
        record_mount_superseded(
            &store,
            "p1",
            "claude",
            "2024-01-01T00:00:00Z",
            "unmounted",
            &runtime,
            "attempt-1",
        );
        assert_eq!(store.events.borrow().len(), 1);
        let event = &store.events.borrow()[0];
        assert_eq!(event.event_kind, "mount_superseded");
        assert_eq!(
            event.details.get("mount_attempt_id").unwrap(),
            &serde_json::Value::String("attempt-1".to_string())
        );
    }

    #[test]
    fn test_record_mount_succeeded() {
        let store = VecEventStore::default();
        let runtime = test_runtime();
        record_mount_succeeded(
            &store,
            "p1",
            "claude",
            "2024-01-01T00:00:00Z",
            "unmounted",
            &runtime,
        );
        assert_eq!(store.events.borrow().len(), 1);
        let event = &store.events.borrow()[0];
        assert_eq!(event.event_kind, "mount_succeeded");
        assert_eq!(
            event.details.get("restart_count").unwrap(),
            &serde_json::Value::Number(3.into())
        );
    }
}
