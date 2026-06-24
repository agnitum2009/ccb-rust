//! Mirrors Python `lib/ccbrd/supervision/mount_runtime/service.py`.
//!
//! Main entry point: `ensure_mounted` orchestrates a single mount attempt for an
//! agent, including backoff checks, state transitions, transient/fatal error
//! handling, and superseded-attempt cleanup.

use ccbr_agents::models::{AgentRuntime, AgentState};

use super::events::MountEventStore;
use super::starting::AgentSpec;
use super::transitions::{
    in_backoff_window, missing_mount_action_health, mount_actions_missing, mount_or_reflow,
    persist_mount_exception, persist_mount_success, persist_mount_superseded,
    persist_mount_transient, start_mount_attempt, MountActionError, SUCCESS_RUNTIME_HEALTHS,
};

/// Registry abstraction used by mount orchestration.
pub trait MountRegistry {
    /// Return the current authoritative runtime for `agent_name`, if any.
    fn get(&self, agent_name: &str) -> Option<AgentRuntime>;
}

/// Runtime-service abstraction used by mount orchestration.
pub trait MountRuntimeService {
    /// Finalize a mount attempt that failed.
    ///
    /// Returns `(finalized_runtime, applied)`. `applied` is `false` when a
    /// concurrent operation superseded this attempt.
    #[allow(clippy::too_many_arguments)]
    fn finalize_mount_attempt_failure(
        &self,
        agent_name: &str,
        attempt_id: &str,
        attempted_at: &str,
        state: AgentState,
        health: &str,
        reconcile_state: &str,
        restart_count: u32,
        reason: &str,
        lifecycle_state: &str,
    ) -> crate::Result<(Option<AgentRuntime>, bool)>;

    /// Finalize a mount attempt that succeeded.
    ///
    /// Returns `(mounted_runtime, applied)`. `applied` is `false` when a
    /// concurrent operation superseded this attempt.
    fn finalize_mount_attempt_success(
        &self,
        agent_name: &str,
        attempt_id: &str,
        attempted_at: &str,
        restart_count: u32,
    ) -> crate::Result<(Option<AgentRuntime>, bool)>;

    /// Patch a runtime record with steady-state fields.
    fn patch_runtime_state(
        &self,
        runtime: &AgentRuntime,
        reconcile_state: &str,
        last_reconcile_at: &str,
        last_failure_reason: Option<&str>,
        lifecycle_state: &str,
    ) -> crate::Result<AgentRuntime>;
}

/// Layout abstraction used to resolve per-agent workspace paths.
pub trait MountLayout {
    fn workspace_path(&self, agent_name: &str, workspace_root: Option<&str>) -> String;
}

/// Registry that can also supply agent specs for mount-start preparation.
pub trait MountRegistryWithSpec: MountRegistry {
    fn spec_for(&self, agent_name: &str) -> Option<AgentSpec>;
}

/// Runtime-service abstraction used during mount-start preparation.
pub trait MountRuntimeServiceStart {
    /// Arity mirrors the Python `mount_runtime.service` attach helper.
    #[allow(clippy::too_many_arguments)]
    fn attach(
        &self,
        agent_name: &str,
        workspace_path: &str,
        backend_type: &str,
        health: &str,
        provider: &str,
        lifecycle_state: &str,
        managed_by: &str,
        binding_source: &str,
    ) -> crate::Result<AgentRuntime>;

    fn adopt_runtime_authority(
        &self,
        runtime: &AgentRuntime,
        daemon_generation: i64,
    ) -> crate::Result<AgentRuntime>;

    fn begin_mount_attempt(
        &self,
        runtime: &AgentRuntime,
        attempted_at: &str,
    ) -> crate::Result<(AgentRuntime, bool)>;
}

/// Bundle of inputs for `ensure_mounted`.
///
/// This mirrors the Python `ensure_mounted` keyword arguments. Callers provide
/// the runtime, registry, service, action closures, and various policy hooks.
#[allow(clippy::type_complexity)]
pub struct EnsureMountedRequest<'a> {
    pub project_id: &'a str,
    pub agent_name: &'a str,
    pub runtime: Option<AgentRuntime>,
    pub registry: &'a dyn MountRegistry,
    pub runtime_service: &'a dyn MountRuntimeService,
    pub mount_agent_fn: Option<Box<dyn Fn(&str) -> Result<(), MountActionError> + 'a>>,
    pub remount_project_fn: Option<Box<dyn Fn(&str) -> Result<(), MountActionError> + 'a>>,
    pub clock: Box<dyn Fn() -> String + 'a>,
    pub event_store: &'a dyn MountEventStore,
    pub upsert_if_changed_fn: Box<dyn Fn(AgentRuntime) -> crate::Result<AgentRuntime> + 'a>,
    pub build_starting_runtime_fn:
        Box<dyn Fn(&str, Option<&AgentRuntime>, &str) -> crate::Result<AgentRuntime> + 'a>,
    pub persist_mount_failure_fn:
        Box<dyn Fn(&AgentRuntime, &str, &str, &str, &str, u32, &str) -> crate::Result<String> + 'a>,
    pub is_in_backoff_window_fn: Box<dyn Fn(&AgentRuntime, &str) -> bool + 'a>,
    pub should_reflow_project_mount_fn: Box<dyn Fn(&str) -> bool + 'a>,
    pub align_runtime_authority_fn: Box<dyn Fn(AgentRuntime) -> AgentRuntime + 'a>,
    pub normalized_runtime_health_fn: Box<dyn Fn(&AgentRuntime) -> Option<String> + 'a>,
}

/// Ensure `agent_name` is mounted, performing a single mount attempt.
///
/// Mirrors Python `ensure_mounted`. Returns the resulting runtime health.
pub fn ensure_mounted(req: &mut EnsureMountedRequest<'_>) -> crate::Result<String> {
    let runtime_opt = req.runtime.clone();
    let runtime_ref = runtime_opt.as_ref();

    if mount_actions_missing(
        req.mount_agent_fn.as_deref(),
        req.remount_project_fn.as_deref(),
    ) {
        return Ok(missing_mount_action_health(runtime_ref));
    }

    let attempted_at = (req.clock)();
    if in_backoff_window(runtime_ref, &attempted_at, &*req.is_in_backoff_window_fn) {
        return Ok(runtime_ref
            .map(|r| r.health.clone())
            .unwrap_or_else(|| "unmounted".to_string()));
    }

    let (starting, prior_health, next_restart_count) = start_mount_attempt(
        req.agent_name,
        runtime_ref,
        &attempted_at,
        &*req.build_starting_runtime_fn,
    )?;
    let attempt_id = starting
        .mount_attempt_id
        .clone()
        .unwrap_or_else(|| attempted_at.clone());

    use super::events::record_mount_started;
    record_mount_started(
        req.event_store,
        req.project_id,
        req.agent_name,
        &attempted_at,
        &prior_health,
        &starting,
    );

    let mount_result = mount_or_reflow(
        req.agent_name,
        req.mount_agent_fn.as_deref(),
        req.remount_project_fn.as_deref(),
        &*req.should_reflow_project_mount_fn,
    );

    if let Err(err) = mount_result {
        return handle_mount_action_error(
            req,
            &starting,
            &attempt_id,
            &attempted_at,
            &prior_health,
            next_restart_count,
            runtime_ref,
            err,
        );
    }

    let refreshed = req.registry.get(req.agent_name);
    if refreshed.is_none() {
        return handle_runtime_missing_after_mount(
            req,
            &starting,
            &attempt_id,
            &attempted_at,
            &prior_health,
            next_restart_count,
        );
    }

    let refreshed = (req.align_runtime_authority_fn)(refreshed.unwrap());
    let refreshed_health =
        (req.normalized_runtime_health_fn)(&refreshed).unwrap_or_else(|| refreshed.health.clone());

    if !SUCCESS_RUNTIME_HEALTHS.contains(&refreshed_health.as_str()) {
        return handle_unhealthy_after_mount(
            req,
            &refreshed,
            &attempt_id,
            &attempted_at,
            &prior_health,
            next_restart_count,
            &refreshed_health,
        );
    }

    let (mounted, applied) = req.runtime_service.finalize_mount_attempt_success(
        req.agent_name,
        &attempt_id,
        &attempted_at,
        next_restart_count,
    )?;

    if !applied {
        let current = stabilize_superseded_runtime(
            &(req.align_runtime_authority_fn)(mounted.unwrap_or(refreshed)),
            &attempted_at,
            req.runtime_service,
        )?;
        return Ok(persist_mount_superseded(
            &current,
            req.project_id,
            req.agent_name,
            &attempted_at,
            &prior_health,
            req.event_store,
            &attempt_id,
        ));
    }

    let mounted = persist_mount_success(mounted.unwrap_or(refreshed));
    use super::events::record_mount_succeeded;
    record_mount_succeeded(
        req.event_store,
        req.project_id,
        req.agent_name,
        &attempted_at,
        &prior_health,
        &mounted,
    );
    Ok(mounted.health)
}

/// Arity mirrors the Python `mount_runtime.service` error handler.
#[allow(clippy::too_many_arguments)]
fn handle_mount_action_error(
    req: &mut EnsureMountedRequest<'_>,
    starting: &AgentRuntime,
    attempt_id: &str,
    attempted_at: &str,
    prior_health: &str,
    next_restart_count: u32,
    runtime: Option<&AgentRuntime>,
    err: MountActionError,
) -> crate::Result<String> {
    let (state, health, reconcile_state, lifecycle_state, reason) = match err {
        MountActionError::Transient(ref msg) => (
            runtime.map(|r| r.state).unwrap_or(AgentState::Failed),
            runtime
                .map(|r| r.health.clone())
                .unwrap_or_else(|| "start-deferred".to_string()),
            "deferred",
            runtime
                .and_then(|r| r.lifecycle_state.as_deref())
                .unwrap_or("degraded"),
            format!("TmuxTransientServerUnavailable: {msg}"),
        ),
        MountActionError::Fatal(ref msg) => (
            AgentState::Failed,
            "start-failed".to_string(),
            "failed",
            "failed",
            format!("{}: {}", std::any::type_name_of_val(&err), msg),
        ),
    };

    let (finalized, applied) = req.runtime_service.finalize_mount_attempt_failure(
        req.agent_name,
        attempt_id,
        attempted_at,
        state,
        &health,
        reconcile_state,
        next_restart_count,
        &reason,
        lifecycle_state,
    )?;

    if !applied {
        let current = stabilize_superseded_runtime(
            &(req.align_runtime_authority_fn)(finalized.unwrap_or_else(|| {
                req.registry
                    .get(req.agent_name)
                    .unwrap_or_else(|| starting.clone())
            })),
            attempted_at,
            req.runtime_service,
        )?;
        return Ok(persist_mount_superseded(
            &current,
            req.project_id,
            req.agent_name,
            attempted_at,
            prior_health,
            req.event_store,
            attempt_id,
        ));
    }

    let finalized = finalized.unwrap_or_else(|| starting.clone());
    match err {
        MountActionError::Transient(_) => Ok(persist_mount_transient(
            &finalized,
            req.project_id,
            req.agent_name,
            attempted_at,
            prior_health,
            &reason,
            req.event_store,
        )),
        MountActionError::Fatal(_) => Ok(persist_mount_exception(
            &finalized,
            req.project_id,
            req.agent_name,
            attempted_at,
            prior_health,
            req.event_store,
            &reason,
        )),
    }
}

fn handle_runtime_missing_after_mount(
    req: &mut EnsureMountedRequest<'_>,
    starting: &AgentRuntime,
    attempt_id: &str,
    attempted_at: &str,
    prior_health: &str,
    next_restart_count: u32,
) -> crate::Result<String> {
    let (finalized, applied) = req.runtime_service.finalize_mount_attempt_failure(
        req.agent_name,
        attempt_id,
        attempted_at,
        AgentState::Failed,
        "start-failed",
        "failed",
        next_restart_count,
        "runtime-missing-after-mount",
        "failed",
    )?;

    if !applied {
        let current = stabilize_superseded_runtime(
            &(req.align_runtime_authority_fn)(finalized.unwrap_or_else(|| starting.clone())),
            attempted_at,
            req.runtime_service,
        )?;
        return Ok(persist_mount_superseded(
            &current,
            req.project_id,
            req.agent_name,
            attempted_at,
            prior_health,
            req.event_store,
            attempt_id,
        ));
    }

    (req.persist_mount_failure_fn)(
        &finalized.unwrap_or_else(|| starting.clone()),
        req.agent_name,
        req.project_id,
        attempted_at,
        prior_health,
        next_restart_count,
        "runtime-missing-after-mount",
    )
}

fn handle_unhealthy_after_mount(
    req: &mut EnsureMountedRequest<'_>,
    refreshed: &AgentRuntime,
    attempt_id: &str,
    attempted_at: &str,
    prior_health: &str,
    next_restart_count: u32,
    refreshed_health: &str,
) -> crate::Result<String> {
    let reason = if refreshed_health.is_empty() {
        "mount-produced-unhealthy-runtime"
    } else {
        refreshed_health
    };
    let health = if refreshed_health.is_empty() {
        "start-failed"
    } else {
        refreshed_health
    };
    let lifecycle_state = refreshed.lifecycle_state.as_deref().unwrap_or("failed");

    let (finalized, applied) = req.runtime_service.finalize_mount_attempt_failure(
        req.agent_name,
        attempt_id,
        attempted_at,
        AgentState::Failed,
        health,
        "failed",
        next_restart_count,
        reason,
        lifecycle_state,
    )?;

    if !applied {
        let current = stabilize_superseded_runtime(
            &(req.align_runtime_authority_fn)(finalized.unwrap_or_else(|| refreshed.clone())),
            attempted_at,
            req.runtime_service,
        )?;
        return Ok(persist_mount_superseded(
            &current,
            req.project_id,
            req.agent_name,
            attempted_at,
            prior_health,
            req.event_store,
            attempt_id,
        ));
    }

    (req.persist_mount_failure_fn)(
        &finalized.unwrap_or_else(|| refreshed.clone()),
        req.agent_name,
        req.project_id,
        attempted_at,
        prior_health,
        next_restart_count,
        reason,
    )
}

/// Stabilize a runtime record that was superseded while in the `starting` reconcile state.
///
/// Mirrors Python `stabilize_superseded_runtime`. Returns `Ok(None)` when the input
/// is `None`; otherwise patches an IDLE/starting runtime to steady/idle.
pub fn stabilize_superseded_runtime(
    runtime: &AgentRuntime,
    attempted_at: &str,
    runtime_service: &dyn MountRuntimeService,
) -> crate::Result<AgentRuntime> {
    if runtime.state == AgentState::Idle && runtime.reconcile_state.as_deref() == Some("starting") {
        return runtime_service.patch_runtime_state(runtime, "steady", attempted_at, None, "idle");
    }
    Ok(runtime.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccbr_agents::models::{AgentRuntime, AgentState};

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

    struct TestRegistry {
        entries: std::collections::HashMap<String, AgentRuntime>,
    }

    impl MountRegistry for TestRegistry {
        fn get(&self, agent_name: &str) -> Option<AgentRuntime> {
            self.entries.get(agent_name).cloned()
        }
    }

    struct TestRuntimeService {
        apply_success: bool,
    }

    impl MountRuntimeService for TestRuntimeService {
        fn finalize_mount_attempt_failure(
            &self,
            _agent_name: &str,
            attempt_id: &str,
            attempted_at: &str,
            state: AgentState,
            health: &str,
            reconcile_state: &str,
            restart_count: u32,
            reason: &str,
            lifecycle_state: &str,
        ) -> crate::Result<(Option<AgentRuntime>, bool)> {
            let mut runtime = AgentRuntime {
                agent_name: _agent_name.to_string(),
                state,
                health: health.to_string(),
                project_id: "p1".to_string(),
                backend_type: "pane-backed".to_string(),
                reconcile_state: Some(reconcile_state.to_string()),
                lifecycle_state: Some(lifecycle_state.to_string()),
                restart_count,
                last_failure_reason: Some(reason.to_string()),
                mount_attempt_id: Some(attempt_id.to_string()),
                last_reconcile_at: Some(attempted_at.to_string()),
                ..AgentRuntime::default()
            };
            if reason.contains("superseded") {
                runtime.health = "healthy".to_string();
                runtime.state = AgentState::Idle;
                runtime.reconcile_state = Some("starting".to_string());
                runtime.lifecycle_state = Some("idle".to_string());
                return Ok((Some(runtime), false));
            }
            Ok((Some(runtime), true))
        }

        fn finalize_mount_attempt_success(
            &self,
            agent_name: &str,
            attempt_id: &str,
            attempted_at: &str,
            restart_count: u32,
        ) -> crate::Result<(Option<AgentRuntime>, bool)> {
            let mut runtime = AgentRuntime {
                agent_name: agent_name.to_string(),
                state: AgentState::Idle,
                health: "healthy".to_string(),
                project_id: "p1".to_string(),
                backend_type: "pane-backed".to_string(),
                reconcile_state: Some("steady".to_string()),
                lifecycle_state: Some("idle".to_string()),
                restart_count,
                mount_attempt_id: Some(attempt_id.to_string()),
                last_reconcile_at: Some(attempted_at.to_string()),
                ..AgentRuntime::default()
            };
            if !self.apply_success {
                runtime.reconcile_state = Some("starting".to_string());
                return Ok((Some(runtime), false));
            }
            Ok((Some(runtime), true))
        }

        fn patch_runtime_state(
            &self,
            runtime: &AgentRuntime,
            reconcile_state: &str,
            last_reconcile_at: &str,
            last_failure_reason: Option<&str>,
            lifecycle_state: &str,
        ) -> crate::Result<AgentRuntime> {
            let mut updated = runtime.clone();
            updated.reconcile_state = Some(reconcile_state.to_string());
            updated.last_reconcile_at = Some(last_reconcile_at.to_string());
            updated.last_failure_reason = last_failure_reason.map(|s| s.to_string());
            updated.lifecycle_state = Some(lifecycle_state.to_string());
            Ok(updated)
        }
    }

    fn test_runtime() -> AgentRuntime {
        AgentRuntime {
            agent_name: "claude".to_string(),
            state: AgentState::Idle,
            project_id: "p1".to_string(),
            backend_type: "pane-backed".to_string(),
            health: "healthy".to_string(),
            provider: Some("claude".to_string()),
            reconcile_state: Some("steady".to_string()),
            lifecycle_state: Some("idle".to_string()),
            restart_count: 1,
            ..AgentRuntime::default()
        }
    }

    #[allow(clippy::type_complexity)]
    fn build_request<'a>(
        registry: &'a TestRegistry,
        runtime_service: &'a TestRuntimeService,
        event_store: &'a VecEventStore,
        mount_agent_fn: Option<Box<dyn Fn(&str) -> Result<(), MountActionError> + 'a>>,
    ) -> EnsureMountedRequest<'a> {
        EnsureMountedRequest {
            project_id: "p1",
            agent_name: "claude",
            runtime: None,
            registry,
            runtime_service,
            mount_agent_fn,
            remount_project_fn: None,
            clock: Box::new(|| "2024-01-01T00:00:00Z".to_string()),
            event_store,
            upsert_if_changed_fn: Box::new(Ok::<AgentRuntime, crate::DaemonError>),
            build_starting_runtime_fn: Box::new(|agent_name, runtime, attempted_at| {
                let mut starting = runtime.cloned().unwrap_or_else(|| AgentRuntime {
                    agent_name: agent_name.to_string(),
                    state: AgentState::Starting,
                    project_id: "p1".to_string(),
                    backend_type: "pane-backed".to_string(),
                    health: "starting".to_string(),
                    reconcile_state: Some("starting".to_string()),
                    lifecycle_state: Some("starting".to_string()),
                    desired_state: Some("mounted".to_string()),
                    ..AgentRuntime::default()
                });
                starting.state = AgentState::Starting;
                starting.health = "starting".to_string();
                starting.reconcile_state = Some("starting".to_string());
                starting.lifecycle_state = Some("starting".to_string());
                starting.last_reconcile_at = Some(attempted_at.to_string());
                starting.mount_attempt_id = Some(format!("attempt-{attempted_at}"));
                Ok(starting)
            }),
            persist_mount_failure_fn: Box::new(
                |runtime,
                 agent_name,
                 project_id,
                 attempted_at,
                 prior_health,
                 next_restart_count,
                 reason| {
                    let mut failed = runtime.clone();
                    failed.state = AgentState::Failed;
                    failed.health = "start-failed".to_string();
                    failed.restart_count = next_restart_count;
                    failed.last_reconcile_at = Some(attempted_at.to_string());
                    failed.last_failure_reason = Some(reason.to_string());
                    use super::super::events::record_mount_failed;
                    record_mount_failed(
                        event_store,
                        project_id,
                        agent_name,
                        attempted_at,
                        prior_health,
                        &failed,
                        reason,
                    );
                    Ok(failed.health)
                },
            ),
            is_in_backoff_window_fn: Box::new(|_, _| false),
            should_reflow_project_mount_fn: Box::new(|_| false),
            align_runtime_authority_fn: Box::new(|r| r),
            normalized_runtime_health_fn: Box::new(|r| Some(r.health.clone())),
        }
    }

    #[test]
    fn test_ensure_mounted_missing_actions() {
        let registry = TestRegistry {
            entries: std::collections::HashMap::new(),
        };
        let service = TestRuntimeService {
            apply_success: true,
        };
        let store = VecEventStore::default();
        let mut req = build_request(&registry, &service, &store, None);
        let health = ensure_mounted(&mut req).unwrap();
        assert_eq!(health, "unmounted");
        assert!(store.events.borrow().is_empty());
    }

    #[test]
    fn test_ensure_mounted_success() {
        let mut registry = TestRegistry {
            entries: std::collections::HashMap::new(),
        };
        registry
            .entries
            .insert("claude".to_string(), test_runtime());
        let service = TestRuntimeService {
            apply_success: true,
        };
        let store = VecEventStore::default();
        let mount = |_name: &str| Ok(());
        let mut req = build_request(&registry, &service, &store, Some(Box::new(mount)));
        let health = ensure_mounted(&mut req).unwrap();
        assert_eq!(health, "healthy");
        assert_eq!(store.events.borrow().len(), 2);
        assert_eq!(store.events.borrow()[0].event_kind, "mount_started");
        assert_eq!(store.events.borrow()[1].event_kind, "mount_succeeded");
    }

    #[test]
    fn test_ensure_mounted_unhealthy_after_mount() {
        let mut registry = TestRegistry {
            entries: std::collections::HashMap::new(),
        };
        registry.entries.insert(
            "claude".to_string(),
            AgentRuntime {
                health: "failed".to_string(),
                ..test_runtime()
            },
        );
        let service = TestRuntimeService {
            apply_success: true,
        };
        let store = VecEventStore::default();
        let mount = |_name: &str| Ok(());
        let mut req = build_request(&registry, &service, &store, Some(Box::new(mount)));
        let health = ensure_mounted(&mut req).unwrap();
        assert_eq!(health, "start-failed");
        assert_eq!(store.events.borrow().len(), 2);
        assert_eq!(store.events.borrow()[0].event_kind, "mount_started");
        assert_eq!(store.events.borrow()[1].event_kind, "mount_failed");
    }

    #[test]
    fn test_ensure_mounted_runtime_missing() {
        let registry = TestRegistry {
            entries: std::collections::HashMap::new(),
        };
        let service = TestRuntimeService {
            apply_success: true,
        };
        let store = VecEventStore::default();
        let mount = |_name: &str| Ok(());
        let mut req = build_request(&registry, &service, &store, Some(Box::new(mount)));
        let health = ensure_mounted(&mut req).unwrap();
        assert_eq!(health, "start-failed");
        assert_eq!(store.events.borrow().len(), 2);
        assert_eq!(store.events.borrow()[1].event_kind, "mount_failed");
    }

    #[test]
    fn test_ensure_mounted_transient_error() {
        let mut registry = TestRegistry {
            entries: std::collections::HashMap::new(),
        };
        registry
            .entries
            .insert("claude".to_string(), test_runtime());
        let service = TestRuntimeService {
            apply_success: true,
        };
        let store = VecEventStore::default();
        let mount = |_name: &str| Err(MountActionError::Transient("tmux away".to_string()));
        let mut req = build_request(&registry, &service, &store, Some(Box::new(mount)));
        let health = ensure_mounted(&mut req).unwrap();
        // No prior runtime was supplied, so the transient failure is deferred.
        assert_eq!(health, "start-deferred");
        assert_eq!(store.events.borrow().len(), 2);
        assert_eq!(store.events.borrow()[1].event_kind, "mount_failed");
    }

    #[test]
    fn test_ensure_mounted_fatal_error() {
        let mut registry = TestRegistry {
            entries: std::collections::HashMap::new(),
        };
        registry
            .entries
            .insert("claude".to_string(), test_runtime());
        let service = TestRuntimeService {
            apply_success: true,
        };
        let store = VecEventStore::default();
        let mount = |_name: &str| Err(MountActionError::Fatal("kaboom".to_string()));
        let mut req = build_request(&registry, &service, &store, Some(Box::new(mount)));
        let health = ensure_mounted(&mut req).unwrap();
        assert_eq!(health, "start-failed");
        assert_eq!(store.events.borrow().len(), 2);
        assert_eq!(store.events.borrow()[1].event_kind, "mount_failed");
    }

    #[test]
    fn test_ensure_mounted_superseded_success() {
        let mut registry = TestRegistry {
            entries: std::collections::HashMap::new(),
        };
        registry
            .entries
            .insert("claude".to_string(), test_runtime());
        let service = TestRuntimeService {
            apply_success: false,
        };
        let store = VecEventStore::default();
        let mount = |_name: &str| Ok(());
        let mut req = build_request(&registry, &service, &store, Some(Box::new(mount)));
        let health = ensure_mounted(&mut req).unwrap();
        assert_eq!(health, "healthy");
        assert_eq!(store.events.borrow().len(), 2);
        assert_eq!(store.events.borrow()[1].event_kind, "mount_superseded");
    }

    #[test]
    fn test_stabilize_superseded_runtime() {
        let service = TestRuntimeService {
            apply_success: true,
        };
        let runtime = AgentRuntime {
            state: AgentState::Idle,
            reconcile_state: Some("starting".to_string()),
            ..test_runtime()
        };
        let stabilized = stabilize_superseded_runtime(&runtime, "now", &service).unwrap();
        assert_eq!(stabilized.reconcile_state, Some("steady".to_string()));
        assert_eq!(stabilized.lifecycle_state, Some("idle".to_string()));
        assert_eq!(stabilized.last_failure_reason, None);
    }

    #[test]
    fn test_stabilize_superseded_runtime_noop() {
        let service = TestRuntimeService {
            apply_success: true,
        };
        let runtime = test_runtime();
        let stabilized = stabilize_superseded_runtime(&runtime, "now", &service).unwrap();
        assert_eq!(stabilized.reconcile_state, Some("steady".to_string()));
    }
}
