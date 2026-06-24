//! Mirrors Python `lib/ccbd/supervision/mount_runtime/starting.py`.
//!
//! Builds the "starting" runtime record that is used for a mount attempt.

use ccbr_agents::models::{AgentRuntime, AgentState, RuntimeBindingSource, RuntimeMode};

/// Minimal agent spec fields needed to start a mount attempt.
#[derive(Debug, Clone)]
pub struct AgentSpec {
    pub name: String,
    pub provider: String,
    pub workspace_root: Option<String>,
    pub runtime_mode: RuntimeMode,
}

/// Layout abstraction used to resolve the per-agent workspace path.
pub trait StartingLayout {
    fn workspace_path(&self, agent_name: &str, workspace_root: Option<&str>) -> String;
}

/// Registry abstraction used during mount-start preparation.
pub trait StartingRegistry {
    fn spec_for(&self, agent_name: &str) -> Option<AgentSpec>;
    fn get(&self, agent_name: &str) -> Option<AgentRuntime>;
    fn upsert_authority(&mut self, runtime: AgentRuntime) -> AgentRuntime;
}

/// Runtime-service abstraction used during mount-start preparation.
pub trait StartingRuntimeService {
    /// Arity mirrors the Python `mount_runtime.starting` attach helper.
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

/// Build a runtime record in the `starting` reconcile state.
#[allow(clippy::too_many_arguments)]
pub fn build_starting_runtime(
    agent_name: &str,
    runtime: Option<&AgentRuntime>,
    attempted_at: &str,
    layout: &dyn StartingLayout,
    registry: &mut dyn StartingRegistry,
    runtime_service: &dyn StartingRuntimeService,
    generation_getter: &dyn Fn() -> i64,
) -> crate::Result<AgentRuntime> {
    let spec = registry
        .spec_for(agent_name)
        .ok_or_else(|| crate::DaemonError::Config(format!("no spec for agent {agent_name}")))?;
    let workspace_path = layout.workspace_path(agent_name, spec.workspace_root.as_deref());
    let backend_type = runtime_mode_to_backend_type(&spec.runtime_mode);
    let generation = generation_getter();

    let current = if let Some(runtime) = runtime {
        let mut current = runtime.clone();
        if authority_adopt_required(runtime, generation) {
            current = runtime_service.adopt_runtime_authority(runtime, generation)?;
        }
        current
    } else {
        runtime_service.attach(
            agent_name,
            &workspace_path,
            &backend_type,
            "starting",
            &spec.provider,
            "starting",
            "ccbd",
            "provider-session",
        )?
    };

    let mut candidate = current.clone();
    candidate.state = AgentState::Starting;
    candidate.health = "starting".to_string();
    if candidate.workspace_path.is_none() {
        candidate.workspace_path = Some(workspace_path);
    }
    if candidate.backend_type.is_empty() {
        candidate.backend_type = backend_type;
    }
    if candidate.provider.is_none() {
        candidate.provider = Some(spec.provider.clone());
    }
    candidate.lifecycle_state = Some("starting".to_string());
    if runtime.is_none() {
        candidate.daemon_generation = Some(generation);
    }
    candidate.desired_state = Some("mounted".to_string());
    candidate.reconcile_state = Some("starting".to_string());
    candidate.last_reconcile_at = Some(attempted_at.to_string());
    candidate.last_failure_reason = None;

    let current = registry.upsert_authority(candidate);

    let (started, _) = runtime_service.begin_mount_attempt(&current, attempted_at)?;
    Ok(started)
}

/// Decide whether the runtime authority needs to be adopted for the current daemon generation.
pub fn authority_adopt_required(runtime: &AgentRuntime, generation: i64) -> bool {
    if runtime.binding_source == RuntimeBindingSource::ExternalAttach {
        return false;
    }
    if !matches!(
        runtime.state,
        AgentState::Idle | AgentState::Busy | AgentState::Degraded
    ) {
        return false;
    }
    let current_generation = runtime.daemon_generation.unwrap_or(0);
    current_generation != generation
}

fn runtime_mode_to_backend_type(mode: &RuntimeMode) -> String {
    match mode {
        RuntimeMode::PaneBacked => "pane-backed".to_string(),
        RuntimeMode::PtyBacked => "pty-backed".to_string(),
        RuntimeMode::Headless => "headless".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestLayout {
        root: String,
    }

    impl StartingLayout for TestLayout {
        fn workspace_path(&self, agent_name: &str, _workspace_root: Option<&str>) -> String {
            format!("{}/{}", self.root, agent_name)
        }
    }

    struct TestRegistry {
        spec: AgentSpec,
        entries: std::collections::HashMap<String, AgentRuntime>,
    }

    impl StartingRegistry for TestRegistry {
        fn spec_for(&self, _agent_name: &str) -> Option<AgentSpec> {
            Some(self.spec.clone())
        }
        fn get(&self, agent_name: &str) -> Option<AgentRuntime> {
            self.entries.get(agent_name).cloned()
        }
        fn upsert_authority(&mut self, runtime: AgentRuntime) -> AgentRuntime {
            self.entries
                .insert(runtime.agent_name.clone(), runtime.clone());
            runtime
        }
    }

    struct TestRuntimeService;

    impl StartingRuntimeService for TestRuntimeService {
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
        ) -> crate::Result<AgentRuntime> {
            Ok(AgentRuntime {
                agent_name: agent_name.to_string(),
                project_id: "p1".to_string(),
                workspace_path: Some(workspace_path.to_string()),
                backend_type: backend_type.to_string(),
                health: health.to_string(),
                provider: Some(provider.to_string()),
                lifecycle_state: Some(lifecycle_state.to_string()),
                managed_by: managed_by.to_string(),
                binding_source: match binding_source {
                    "external-attach" => RuntimeBindingSource::ExternalAttach,
                    _ => RuntimeBindingSource::ProviderSession,
                },
                ..AgentRuntime::default()
            })
        }

        fn adopt_runtime_authority(
            &self,
            runtime: &AgentRuntime,
            daemon_generation: i64,
        ) -> crate::Result<AgentRuntime> {
            let mut updated = runtime.clone();
            updated.daemon_generation = Some(daemon_generation);
            Ok(updated)
        }

        fn begin_mount_attempt(
            &self,
            runtime: &AgentRuntime,
            attempted_at: &str,
        ) -> crate::Result<(AgentRuntime, bool)> {
            let mut updated = runtime.clone();
            updated.mount_attempt_id = Some(format!("attempt-{attempted_at}"));
            updated.reconcile_state = Some("starting".to_string());
            updated.last_reconcile_at = Some(attempted_at.to_string());
            Ok((updated, true))
        }
    }

    fn test_spec() -> AgentSpec {
        AgentSpec {
            name: "claude".to_string(),
            provider: "claude".to_string(),
            workspace_root: None,
            runtime_mode: RuntimeMode::PaneBacked,
        }
    }

    #[test]
    fn test_build_starting_runtime_from_scratch() {
        let layout = TestLayout {
            root: "/ws".to_string(),
        };
        let mut registry = TestRegistry {
            spec: test_spec(),
            entries: std::collections::HashMap::new(),
        };
        let service = TestRuntimeService;
        let started = build_starting_runtime(
            "claude",
            None,
            "2024-01-01T00:00:00Z",
            &layout,
            &mut registry,
            &service,
            &|| 42,
        )
        .unwrap();
        assert_eq!(started.agent_name, "claude");
        assert!(matches!(started.state, AgentState::Starting));
        assert_eq!(started.health, "starting");
        assert_eq!(started.workspace_path, Some("/ws/claude".to_string()));
        assert_eq!(started.daemon_generation, Some(42));
        assert_eq!(started.desired_state, Some("mounted".to_string()));
        assert_eq!(started.reconcile_state, Some("starting".to_string()));
        assert!(started.mount_attempt_id.is_some());
    }

    #[test]
    fn test_build_starting_runtime_adopts_authority() {
        let layout = TestLayout {
            root: "/ws".to_string(),
        };
        let existing = AgentRuntime {
            agent_name: "claude".to_string(),
            state: AgentState::Idle,
            project_id: "p1".to_string(),
            backend_type: "pane-backed".to_string(),
            health: "healthy".to_string(),
            daemon_generation: Some(1),
            binding_source: RuntimeBindingSource::ProviderSession,
            restart_count: 5,
            ..AgentRuntime::default()
        };
        let mut registry = TestRegistry {
            spec: test_spec(),
            entries: [("claude".to_string(), existing.clone())]
                .into_iter()
                .collect(),
        };
        let service = TestRuntimeService;
        let started = build_starting_runtime(
            "claude",
            Some(&existing),
            "2024-01-01T00:00:00Z",
            &layout,
            &mut registry,
            &service,
            &|| 7,
        )
        .unwrap();
        assert_eq!(started.daemon_generation, Some(7));
        assert_eq!(started.restart_count, 5);
    }

    #[test]
    fn test_authority_adopt_required() {
        let runtime = AgentRuntime {
            state: AgentState::Idle,
            binding_source: RuntimeBindingSource::ProviderSession,
            daemon_generation: Some(1),
            ..AgentRuntime::default()
        };
        assert!(authority_adopt_required(&runtime, 2));
        assert!(!authority_adopt_required(&runtime, 1));

        let external = AgentRuntime {
            binding_source: RuntimeBindingSource::ExternalAttach,
            ..runtime.clone()
        };
        assert!(!authority_adopt_required(&external, 2));

        let failed = AgentRuntime {
            state: AgentState::Failed,
            ..runtime.clone()
        };
        assert!(!authority_adopt_required(&failed, 2));
    }
}
