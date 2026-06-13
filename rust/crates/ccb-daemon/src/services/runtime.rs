use serde::{Deserialize, Serialize};

use crate::services::registry::{AgentRegistry, AgentRuntimeEntry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeAttachParams {
    pub agent_name: String,
    pub workspace_path: String,
    pub backend_type: String,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub runtime_ref: Option<String>,
    #[serde(default)]
    pub session_ref: Option<String>,
    #[serde(default)]
    pub health: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub pane_id: Option<String>,
    #[serde(default)]
    pub active_pane_id: Option<String>,
    #[serde(default)]
    pub session_file: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub binding_source: Option<String>,
}

/// Runtime service that operates on an externally owned agent registry.
#[derive(Debug, Default)]
pub struct RuntimeService;

impl RuntimeService {
    pub fn new() -> Self {
        Self
    }

    pub fn attach(
        &self,
        registry: &mut AgentRegistry,
        params: RuntimeAttachParams,
    ) -> AgentRuntimeEntry {
        let entry = AgentRuntimeEntry {
            agent_name: params.agent_name.clone(),
            provider: params.provider.unwrap_or_default(),
            state: "idle".into(),
            health: params.health.unwrap_or_else(|| "healthy".into()),
            pane_id: params.pane_id,
            workspace_path: Some(params.workspace_path),
            runtime_pid: params.pid,
            session_id: params.session_id,
            restart_count: 0,
        };
        registry.register(entry.clone());
        entry
    }

    pub fn refresh_provider_binding<'a>(
        &self,
        registry: &'a mut AgentRegistry,
        agent_name: &str,
        recover: bool,
    ) -> Option<&'a AgentRuntimeEntry> {
        if recover {
            if let Some(entry) = registry.get_mut(agent_name) {
                entry.health = "healthy".into();
            }
        }
        registry.get(agent_name)
    }

    pub fn restore(&self, _registry: &mut AgentRegistry, agent_name: &str) -> serde_json::Value {
        serde_json::json!({
            "agent_name": agent_name,
            "status": "restored",
        })
    }
}
