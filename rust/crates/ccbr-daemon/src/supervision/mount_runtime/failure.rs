//! Mirrors Python `lib/ccbd/supervision/mount_runtime/failure.py`.
//!
//! Persists a mount failure by updating the runtime record and emitting a
//! `mount_failed` supervision event.

use ccbr_agents::models::{AgentRuntime, AgentState};

use super::events::{record_mount_failed, MountEventStore};

/// Update `runtime` to a failed state, persist it, and emit a `mount_failed` event.
#[allow(clippy::too_many_arguments)]
pub fn persist_mount_failure(
    runtime: &AgentRuntime,
    agent_name: &str,
    project_id: &str,
    attempted_at: &str,
    prior_health: &str,
    next_restart_count: u32,
    reason: &str,
    event_store: &dyn MountEventStore,
    upsert_if_changed_fn: &dyn Fn(AgentRuntime) -> crate::Result<AgentRuntime>,
) -> crate::Result<String> {
    let mut failed = runtime.clone();
    failed.state = AgentState::Failed;
    failed.health = "start-failed".to_string();
    failed.lifecycle_state = Some("failed".to_string());
    failed.reconcile_state = Some("failed".to_string());
    failed.restart_count = next_restart_count;
    failed.last_reconcile_at = Some(attempted_at.to_string());
    failed.last_failure_reason = Some(reason.to_string());

    let failed = upsert_if_changed_fn(failed)?;
    record_mount_failed(
        event_store,
        project_id,
        agent_name,
        attempted_at,
        prior_health,
        &failed,
        failed.last_failure_reason.as_deref().unwrap_or(reason),
    );
    Ok(failed.health)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct VecEventStore {
        events: std::cell::RefCell<Vec<super::super::events::SupervisionEvent>>,
    }

    impl Default for VecEventStore {
        fn default() -> Self {
            Self {
                events: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl MountEventStore for VecEventStore {
        fn append(&self, event: super::super::events::SupervisionEvent) {
            self.events.borrow_mut().push(event);
        }
    }

    #[test]
    fn test_persist_mount_failure() {
        let runtime = AgentRuntime {
            agent_name: "claude".to_string(),
            state: AgentState::Starting,
            project_id: "p1".to_string(),
            backend_type: "pane-backed".to_string(),
            health: "starting".to_string(),
            restart_count: 2,
            ..AgentRuntime::default()
        };
        let store = VecEventStore::default();
        let upsert = |r: AgentRuntime| Ok::<_, crate::DaemonError>(r);
        let health = persist_mount_failure(
            &runtime,
            "claude",
            "p1",
            "2024-01-01T00:00:00Z",
            "unmounted",
            3,
            "runtime-missing-after-mount",
            &store,
            &upsert,
        )
        .unwrap();
        assert_eq!(health, "start-failed");
        assert_eq!(store.events.borrow().len(), 1);
        assert_eq!(store.events.borrow()[0].event_kind, "mount_failed");
        assert_eq!(
            store.events.borrow()[0].details.get("reason").unwrap(),
            &serde_json::Value::String("runtime-missing-after-mount".to_string())
        );
    }
}
