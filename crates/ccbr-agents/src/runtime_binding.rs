use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::models::{AgentRuntime, RuntimeBindingSource};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBinding {
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl RuntimeBinding {
    pub fn from_runtime(runtime: &AgentRuntime) -> Self {
        Self {
            runtime_ref: runtime.runtime_ref.clone(),
            session_ref: runtime.session_ref.clone(),
            workspace_path: runtime.workspace_path.clone(),
            extra: HashMap::new(),
        }
    }

    pub fn source(&self) -> RuntimeBindingSource {
        self.runtime_ref
            .as_ref()
            .map(|_| RuntimeBindingSource::ProviderSession)
            .unwrap_or(RuntimeBindingSource::ExternalAttach)
    }
}

pub fn build_runtime_binding(
    runtime_ref: Option<String>,
    session_ref: Option<String>,
    workspace_path: Option<String>,
) -> RuntimeBinding {
    RuntimeBinding {
        runtime_ref,
        session_ref,
        workspace_path,
        extra: HashMap::new(),
    }
}

pub fn merge_runtime_binding(base: &RuntimeBinding, overlay: &RuntimeBinding) -> RuntimeBinding {
    RuntimeBinding {
        runtime_ref: overlay
            .runtime_ref
            .clone()
            .or_else(|| base.runtime_ref.clone()),
        session_ref: overlay
            .session_ref
            .clone()
            .or_else(|| base.session_ref.clone()),
        workspace_path: overlay
            .workspace_path
            .clone()
            .or_else(|| base.workspace_path.clone()),
        extra: merge_extra(&base.extra, &overlay.extra),
    }
}

fn merge_extra(
    base: &HashMap<String, serde_json::Value>,
    overlay: &HashMap<String, serde_json::Value>,
) -> HashMap<String, serde_json::Value> {
    let mut merged = base.clone();
    for (k, v) in overlay {
        merged.insert(k.clone(), v.clone());
    }
    merged
}

pub fn runtime_binding_from_runtime(runtime: &AgentRuntime) -> RuntimeBinding {
    RuntimeBinding::from_runtime(runtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_runtime() -> AgentRuntime {
        AgentRuntime {
            agent_name: "agent1".into(),
            runtime_ref: Some("ref-1".into()),
            session_ref: Some("sess-1".into()),
            workspace_path: Some("/ws/agent1".into()),
            ..crate::models::AgentRuntime::default()
        }
    }

    #[test]
    fn test_from_runtime() {
        let runtime = sample_runtime();
        let binding = RuntimeBinding::from_runtime(&runtime);
        assert_eq!(binding.runtime_ref.as_deref(), Some("ref-1"));
        assert_eq!(binding.source(), RuntimeBindingSource::ProviderSession);
    }

    #[test]
    fn test_merge_runtime_binding() {
        let base = build_runtime_binding(Some("r1".into()), None, Some("/a".into()));
        let overlay = build_runtime_binding(None, Some("s2".into()), None);
        let merged = merge_runtime_binding(&base, &overlay);
        assert_eq!(merged.runtime_ref.as_deref(), Some("r1"));
        assert_eq!(merged.session_ref.as_deref(), Some("s2"));
        assert_eq!(merged.workspace_path.as_deref(), Some("/a"));
    }
}
