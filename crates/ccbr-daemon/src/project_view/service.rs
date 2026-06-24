use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectViewState {
    pub project_root: String,
    pub project_slug: String,
    pub agents: Vec<ProjectViewAgent>,
    pub windows: Vec<ProjectViewWindow>,
    pub comms: Vec<ProjectViewComm>,
    pub daemon_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectViewAgent {
    pub name: String,
    pub provider: String,
    pub state: String,
    pub health: String,
    pub pane_id: Option<String>,
    pub workspace_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectViewWindow {
    pub name: String,
    pub window_id: Option<String>,
    pub agents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectViewComm {
    pub id: String,
    pub from_actor: String,
    pub message_type: String,
    pub body_preview: String,
    pub status: String,
    pub created_at: String,
}

pub struct ProjectViewService {
    layout: ccbr_storage::paths::PathLayout,
}

impl ProjectViewService {
    pub fn new(layout: ccbr_storage::paths::PathLayout) -> Self {
        Self { layout }
    }

    pub fn build_response(
        &self,
        agents: &[crate::services::registry::AgentRuntimeEntry],
        comms: &[ProjectViewComm],
    ) -> ProjectViewState {
        ProjectViewState {
            project_root: self.layout.project_root.to_string(),
            project_slug: self.layout.project_slug(),
            agents: agents
                .iter()
                .map(|a| ProjectViewAgent {
                    name: a.agent_name.clone(),
                    provider: a.provider.clone(),
                    state: a.state.clone(),
                    health: a.health.clone(),
                    pane_id: a.pane_id.clone(),
                    workspace_path: a.workspace_path.clone(),
                })
                .collect(),
            windows: vec![],
            comms: comms.to_vec(),
            daemon_status: "running".into(),
        }
    }

    pub fn to_record(&self, state: &ProjectViewState) -> serde_json::Value {
        serde_json::json!({
            "project_root": state.project_root,
            "project_slug": state.project_slug,
            "agents": state.agents.iter().map(|a| serde_json::json!({
                "name": a.name,
                "provider": a.provider,
                "state": a.state,
                "health": a.health,
                "pane_id": a.pane_id,
                "workspace_path": a.workspace_path,
            })).collect::<Vec<_>>(),
            "windows": state.windows.iter().map(|w| serde_json::json!({
                "name": w.name,
                "window_id": w.window_id,
                "agents": w.agents,
            })).collect::<Vec<_>>(),
            "comms": state.comms.iter().map(|c| serde_json::json!({
                "id": c.id,
                "from_actor": c.from_actor,
                "message_type": c.message_type,
                "body_preview": c.body_preview,
                "status": c.status,
                "created_at": c.created_at,
            })).collect::<Vec<_>>(),
            "daemon_status": state.daemon_status,
        })
    }
}
