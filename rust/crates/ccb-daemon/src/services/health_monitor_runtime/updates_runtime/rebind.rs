//! Mirrors Python `lib/ccbd/services/health_monitor_runtime/updates_runtime/rebind.py`.

use ccb_agents::models::{AgentRuntime, AgentState};
use ccb_provider_core::session_binding::{
    session_ref, Session,
};

use crate::services::health_assessment::models::SessionBinding;
use crate::services::provider_runtime_facts::ProviderRuntimeFacts;

use super::common::{
    drop_explicit_runtime_fields, runtime_fields_from_facts,
};

/// Service that can mutate the authoritative portion of a runtime record.
pub trait RuntimeMutationService: std::fmt::Debug {
    #[allow(clippy::too_many_arguments)]
    fn mutate_runtime_authority(
        &self,
        runtime: &AgentRuntime,
        pid: Option<i64>,
        session_ref: Option<String>,
        health: String,
        pane_id: Option<String>,
        active_pane_id: Option<String>,
        pane_state: String,
        extra_fields: &[(String, String)],
    ) -> AgentRuntime;

    fn patch_runtime_state(
        &self,
        runtime: &AgentRuntime,
        state: AgentState,
        last_seen_at: String,
    ) -> AgentRuntime;
}

/// Registry that stores updated runtime records when no runtime service is
/// available.
pub trait AgentRegistry: std::fmt::Debug {
    fn upsert(&self, runtime: AgentRuntime) -> AgentRuntime;
    fn upsert_authority(&self, runtime: AgentRuntime) -> AgentRuntime {
        self.upsert(runtime)
    }
}

/// Monitor context used by `rebind_runtime` to gather facts and persist updates.
pub trait RebindMonitor: std::fmt::Debug {
    fn provider_runtime_facts(
        &self,
        runtime: &AgentRuntime,
        session: &Session,
        binding: &dyn SessionBinding,
        pane_id_override: Option<&str>,
    ) -> Option<ProviderRuntimeFacts>;
    fn clock(&self) -> String;
    fn runtime_service(&self) -> Option<&dyn RuntimeMutationService>;
    fn registry(&self) -> Option<&dyn AgentRegistry>;
}

/// Rebind a runtime to the live facts gathered from its provider session.
///
/// Mirrors Python `rebind_runtime`.
pub fn rebind_runtime(
    monitor: &dyn RebindMonitor,
    runtime: &AgentRuntime,
    session: &Session,
    binding: &dyn SessionBinding,
    pane_id_override: Option<&str>,
    force_session_ref_update: bool,
) -> AgentRuntime {
    let facts = monitor.provider_runtime_facts(runtime, session, binding, pane_id_override);
    let pane_id = bound_pane_id(&facts, pane_id_override, session);
    let bound_session_ref = bound_session_ref(&facts, session, binding);
    let next_session_ref = next_session_ref(runtime, bound_session_ref, force_session_ref_update);
    let updated_fields = updated_runtime_fields(runtime, facts.as_ref());

    if let Some(service) = monitor.runtime_service() {
        let rebound = service.mutate_runtime_authority(
            runtime,
            next_pid(runtime, facts.as_ref()),
            next_session_ref,
            next_health(runtime),
            pane_id.clone().or_else(|| runtime.pane_id.clone()),
            pane_id.clone().or_else(|| runtime.active_pane_id.clone()),
            "alive".to_string(),
            &updated_fields,
        );
        return service.patch_runtime_state(
            &rebound,
            next_state(runtime),
            monitor.clock(),
        );
    }

    let mut updated = runtime.clone();
    updated.state = next_state(runtime);
    updated.pid = next_pid(runtime, facts.as_ref());
    updated.session_ref = next_session_ref;
    updated.health = next_health(runtime);
    updated.pane_id = pane_id.clone().or_else(|| runtime.pane_id.clone());
    updated.active_pane_id = pane_id.or_else(|| runtime.active_pane_id.clone());
    updated.pane_state = Some("alive".to_string());
    updated.last_seen_at = Some(monitor.clock());
    apply_runtime_fields(&mut updated, &updated_fields);

    if let Some(registry) = monitor.registry() {
        return registry.upsert_authority(updated);
    }
    updated
}

fn bound_pane_id(
    facts: &Option<ProviderRuntimeFacts>,
    pane_id_override: Option<&str>,
    session: &Session,
) -> Option<String> {
    if let Some(facts) = facts {
        return facts.pane_id.clone();
    }
    pane_id_override
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            session
                .pane_id
                .as_deref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

fn bound_session_ref(
    facts: &Option<ProviderRuntimeFacts>,
    session: &Session,
    binding: &dyn SessionBinding,
) -> Option<String> {
    if let Some(facts) = facts {
        return facts.session_ref.clone();
    }
    session_ref(session, binding.session_id_attr(), binding.session_path_attr())
}

fn next_session_ref(
    runtime: &AgentRuntime,
    bound_session_ref: Option<String>,
    force_session_ref_update: bool,
) -> Option<String> {
    if force_session_ref_update {
        return bound_session_ref;
    }
    runtime.session_ref.clone().or(bound_session_ref)
}

fn next_state(runtime: &AgentRuntime) -> AgentState {
    if runtime.state == AgentState::Degraded {
        AgentState::Idle
    } else {
        runtime.state
    }
}

fn next_health(runtime: &AgentRuntime) -> String {
    if runtime.state != AgentState::Degraded && runtime.health == "restored" {
        return "restored".to_string();
    }
    "healthy".to_string()
}

fn next_pid(runtime: &AgentRuntime, facts: Option<&ProviderRuntimeFacts>) -> Option<i64> {
    facts
        .and_then(|f| f.runtime_pid)
        .or(runtime.pid)
}

fn updated_runtime_fields(
    runtime: &AgentRuntime,
    facts: Option<&ProviderRuntimeFacts>,
) -> Vec<(String, String)> {
    let Some(facts) = facts else {
        return Vec::new();
    };
    let fields = runtime_fields_from_facts(runtime, facts);
    drop_explicit_runtime_fields(
        &fields,
        &[
            "active_pane_id",
            "health",
            "last_seen_at",
            "pane_id",
            "pane_state",
            "pid",
            "session_ref",
            "state",
        ],
    )
}

fn apply_runtime_fields(runtime: &mut AgentRuntime, fields: &[(String, String)]) {
    for (key, value) in fields {
        match key.as_str() {
            "runtime_ref" => runtime.runtime_ref = Some(value.clone()),
            "runtime_root" => runtime.runtime_root = Some(value.clone()),
            "runtime_pid" => {
                if let Ok(pid) = value.parse::<i64>() {
                    runtime.runtime_pid = Some(pid);
                }
            }
            "terminal_backend" => runtime.terminal_backend = Some(value.clone()),
            "pane_id" => runtime.pane_id = Some(value.clone()),
            "pane_title_marker" => runtime.pane_title_marker = Some(value.clone()),
            "tmux_socket_name" => runtime.tmux_socket_name = Some(value.clone()),
            "tmux_socket_path" => runtime.tmux_socket_path = Some(value.clone()),
            "session_file" => runtime.session_file = Some(value.clone()),
            "session_id" => runtime.session_id = Some(value.clone()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccb_provider_core::session_binding::Session;

    #[derive(Debug, Clone)]
    struct TestBinding {
        session_id_attr: String,
        session_path_attr: String,
    }

    impl SessionBinding for TestBinding {
        fn provider(&self) -> &str {
            "test"
        }
        fn session_id_attr(&self) -> &str {
            &self.session_id_attr
        }
        fn session_path_attr(&self) -> &str {
            &self.session_path_attr
        }
        fn load_session(&self, _root: &std::path::Path, _instance: Option<&str>) -> Option<Session> {
            None
        }
        fn clone_box(&self) -> Box<dyn SessionBinding> {
            Box::new(self.clone())
        }
    }

    #[derive(Debug, Default)]
    struct Capture {
        authority: Option<AgentRuntime>,
        runtime: Option<AgentRuntime>,
    }

    #[derive(Debug)]
    struct TestRuntimeService {
        capture: std::cell::RefCell<Capture>,
    }

    impl RuntimeMutationService for TestRuntimeService {
        #[allow(clippy::too_many_arguments)]
        fn mutate_runtime_authority(
            &self,
            runtime: &AgentRuntime,
            pid: Option<i64>,
            session_ref: Option<String>,
            health: String,
            pane_id: Option<String>,
            active_pane_id: Option<String>,
            pane_state: String,
            extra_fields: &[(String, String)],
        ) -> AgentRuntime {
            let mut updated = runtime.clone();
            updated.pid = pid;
            updated.session_ref = session_ref;
            updated.health = health;
            updated.pane_id = pane_id;
            updated.active_pane_id = active_pane_id;
            updated.pane_state = Some(pane_state);
            apply_runtime_fields(&mut updated, extra_fields);
            self.capture.borrow_mut().authority = Some(updated.clone());
            updated
        }

        fn patch_runtime_state(
            &self,
            runtime: &AgentRuntime,
            state: AgentState,
            last_seen_at: String,
        ) -> AgentRuntime {
            let mut updated = runtime.clone();
            updated.state = state;
            updated.last_seen_at = Some(last_seen_at);
            self.capture.borrow_mut().runtime = Some(updated.clone());
            updated
        }
    }

    #[derive(Debug)]
    struct TestRegistry;

    impl AgentRegistry for TestRegistry {
        fn upsert(&self, runtime: AgentRuntime) -> AgentRuntime {
            runtime
        }
    }

    #[derive(Debug)]
    struct TestMonitor<'a> {
        facts: Option<ProviderRuntimeFacts>,
        clock: String,
        service: Option<&'a dyn RuntimeMutationService>,
        registry: Option<&'a dyn AgentRegistry>,
    }

    impl<'a> RebindMonitor for TestMonitor<'a> {
        fn provider_runtime_facts(
            &self,
            _runtime: &AgentRuntime,
            _session: &Session,
            _binding: &dyn SessionBinding,
            _pane_id_override: Option<&str>,
        ) -> Option<ProviderRuntimeFacts> {
            self.facts.clone()
        }
        fn clock(&self) -> String {
            self.clock.clone()
        }
        fn runtime_service(&self) -> Option<&dyn RuntimeMutationService> {
            self.service
        }
        fn registry(&self) -> Option<&dyn AgentRegistry> {
            self.registry
        }
    }

    fn base_runtime() -> AgentRuntime {
        AgentRuntime {
            agent_name: "agent1".to_string(),
            state: AgentState::Idle,
            pid: Some(11),
            started_at: Some("2026-04-01T00:00:00Z".to_string()),
            last_seen_at: Some("2026-04-01T00:00:01Z".to_string()),
            runtime_ref: Some("tmux:%1".to_string()),
            session_ref: Some("runtime-session".to_string()),
            workspace_path: Some("/tmp/workspace".to_string()),
            project_id: "proj-1".to_string(),
            backend_type: "pane-backed".to_string(),
            queue_depth: 0,
            socket_path: None,
            health: "healthy".to_string(),
            provider: Some("codex".to_string()),
            runtime_root: Some("/tmp/runtime".to_string()),
            runtime_pid: Some(22),
            terminal_backend: Some("tmux".to_string()),
            pane_id: Some("%1".to_string()),
            active_pane_id: Some("%1".to_string()),
            pane_title_marker: Some("agent1".to_string()),
            pane_state: Some("dead".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_rebind_runtime_uses_provider_facts_and_clears_degraded_state() {
        let mut runtime = base_runtime();
        runtime.state = AgentState::Degraded;
        runtime.health = "restored".to_string();
        let facts = ProviderRuntimeFacts {
            runtime_ref: Some("tmux:%9".to_string()),
            session_ref: Some("fact-session".to_string()),
            runtime_root: Some("/new/runtime".to_string()),
            runtime_pid: Some(33),
            terminal_backend: Some("tmux".to_string()),
            pane_id: Some("%9".to_string()),
            pane_title_marker: Some("agent1-new".to_string()),
            pane_state: Some("alive".to_string()),
            tmux_socket_name: Some("sock".to_string()),
            tmux_socket_path: Some("/tmp/tmux.sock".to_string()),
            session_file: Some("/tmp/session.json".to_string()),
            session_id: Some("sid-9".to_string()),
            ccb_session_id: Some("ccb-sid-9".to_string()),
        };
        let service = TestRuntimeService {
            capture: std::cell::RefCell::new(Capture::default()),
        };
        let monitor = TestMonitor {
            facts: Some(facts),
            clock: "2026-04-06T00:00:00Z".to_string(),
            service: Some(&service),
            registry: None,
        };
        let binding = TestBinding {
            session_id_attr: "session_id".to_string(),
            session_path_attr: "session_path".to_string(),
        };
        let session = Session {
            pane_id: Some("%4".to_string()),
            ..Default::default()
        };

        let updated = rebind_runtime(
            &monitor,
            &runtime,
            &session,
            &binding,
            Some("%8"),
            true,
        );

        let authority = service.capture.borrow().authority.clone().unwrap();
        assert_eq!(updated.state, service.capture.borrow().runtime.clone().unwrap().state);
        assert_eq!(authority.state, AgentState::Degraded);
        assert_eq!(authority.runtime_ref, Some("tmux:%9".to_string()));
        assert_eq!(authority.session_ref, Some("fact-session".to_string()));
        assert_eq!(updated.state, AgentState::Idle);
        assert_eq!(updated.health, "healthy");
        assert_eq!(updated.pid, Some(33));
        assert_eq!(updated.session_ref, Some("fact-session".to_string()));
        assert_eq!(updated.pane_id, Some("%9".to_string()));
        assert_eq!(updated.active_pane_id, Some("%9".to_string()));
        assert_eq!(updated.runtime_root, Some("/new/runtime".to_string()));
        assert_eq!(updated.session_file, Some("/tmp/session.json".to_string()));
        assert_eq!(updated.session_id, Some("sid-9".to_string()));
        assert_eq!(updated.pane_state, Some("alive".to_string()));
    }

    #[test]
    fn test_rebind_runtime_falls_back_to_session_binding_when_facts_missing() {
        let mut runtime = base_runtime();
        runtime.session_ref = None;
        runtime.health = "restored".to_string();
        let registry = TestRegistry;
        let monitor = TestMonitor {
            facts: None,
            clock: "2026-04-06T00:00:00Z".to_string(),
            service: None,
            registry: Some(&registry),
        };
        let binding = TestBinding {
            session_id_attr: "session_id".to_string(),
            session_path_attr: "session_path".to_string(),
        };
        let mut session = Session::default();
        session
            .data
            .insert("session_id".to_string(), serde_json::Value::String("bound-session".to_string()));

        let updated = rebind_runtime(&monitor, &runtime, &session, &binding, Some("%7"), false);

        assert_eq!(updated.state, AgentState::Idle);
        assert_eq!(updated.health, "restored");
        assert_eq!(updated.session_ref, Some("bound-session".to_string()));
        assert_eq!(updated.pane_id, Some("%7".to_string()));
        assert_eq!(updated.active_pane_id, Some("%7".to_string()));
        assert_eq!(updated.pane_state, Some("alive".to_string()));
    }
}
