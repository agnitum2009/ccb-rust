use serde::{Deserialize, Serialize};

use crate::models::{AgentSpec, AgentState, PermissionMode, RestoreMode, RuntimeBindingSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EffectiveRestoreMode {
    Attach,
    Provider,
    Memory,
    Fresh,
    Auto,
}

#[derive(Debug, Clone)]
pub struct AgentLaunchPolicy {
    pub agent_name: String,
    pub restore_mode: EffectiveRestoreMode,
    pub permission_mode: PermissionMode,
    pub queue_policy: String,
    pub restore_provider_history: bool,
    pub binding_source: RuntimeBindingSource,
}

pub fn resolve_effective_restore_mode(
    spec: &AgentSpec,
    runtime: Option<&crate::models::AgentRuntime>,
    requested: Option<RestoreMode>,
) -> EffectiveRestoreMode {
    let base = requested.unwrap_or(spec.restore_default);
    match base {
        RestoreMode::Fresh => EffectiveRestoreMode::Fresh,
        RestoreMode::Provider => EffectiveRestoreMode::Provider,
        RestoreMode::Auto => {
            if let Some(runtime) = runtime {
                if runtime.state == AgentState::Failed || runtime.state == AgentState::Stopped {
                    return EffectiveRestoreMode::Fresh;
                }
            }
            EffectiveRestoreMode::Auto
        }
    }
}

pub fn should_restore_provider_history(
    _spec: &AgentSpec,
    restore_mode: EffectiveRestoreMode,
) -> bool {
    !matches!(restore_mode, EffectiveRestoreMode::Fresh)
}

pub fn resolve_effective_permission_mode(
    spec: &AgentSpec,
    requested: Option<PermissionMode>,
) -> PermissionMode {
    requested.unwrap_or(spec.permission_default)
}

pub fn resolve_agent_launch_policy(
    spec: &AgentSpec,
    runtime: Option<&crate::models::AgentRuntime>,
    requested_restore: Option<RestoreMode>,
    requested_permission: Option<PermissionMode>,
) -> AgentLaunchPolicy {
    let restore_mode = resolve_effective_restore_mode(spec, runtime, requested_restore);
    AgentLaunchPolicy {
        agent_name: spec.name.clone(),
        restore_mode,
        permission_mode: resolve_effective_permission_mode(spec, requested_permission),
        queue_policy: format!("{:?}", spec.queue_policy),
        restore_provider_history: should_restore_provider_history(spec, restore_mode),
        binding_source: RuntimeBindingSource::ProviderSession,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AgentSpec, PermissionMode, RestoreMode, WorkspaceMode};

    fn spec_with_restore(mode: RestoreMode) -> AgentSpec {
        AgentSpec {
            name: "a".into(),
            provider: "codex".into(),
            target: ".".into(),
            workspace_mode: WorkspaceMode::Inplace,
            restore_default: mode,
            ..AgentSpec::default_with_name("a")
        }
    }

    #[test]
    fn test_resolve_restore_modes() {
        let spec = spec_with_restore(RestoreMode::Fresh);
        assert_eq!(
            resolve_effective_restore_mode(&spec, None, None),
            EffectiveRestoreMode::Fresh
        );
        let spec = spec_with_restore(RestoreMode::Provider);
        assert_eq!(
            resolve_effective_restore_mode(&spec, None, None),
            EffectiveRestoreMode::Provider
        );
    }

    #[test]
    fn test_permission_default() {
        let mut spec = spec_with_restore(RestoreMode::Fresh);
        spec.permission_default = PermissionMode::Auto;
        assert_eq!(
            resolve_effective_permission_mode(&spec, None),
            PermissionMode::Auto
        );
    }
}
