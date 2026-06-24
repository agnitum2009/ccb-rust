use std::collections::HashMap;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectNamespace {
    pub project_root: String,
    pub project_id: String,
    pub tmux_socket_path: String,
    pub tmux_socket_name: String,
    pub tmux_session_name: String,
    pub agent_names: Vec<String>,
    pub windows: Vec<NamespaceWindow>,
    #[serde(default)]
    pub agent_panes: HashMap<String, String>,
    #[serde(default)]
    pub active_panes: Vec<String>,
    #[serde(default)]
    pub namespace_epoch: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceWindow {
    pub name: String,
    #[serde(default)]
    pub window_id: Option<String>,
    pub agents: Vec<String>,
}

impl ProjectNamespace {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "project_root": self.project_root,
            "project_id": self.project_id,
            "tmux_socket_path": self.tmux_socket_path,
            "tmux_socket_name": self.tmux_socket_name,
            "tmux_session_name": self.tmux_session_name,
            "agent_names": self.agent_names,
            "windows": self.windows.iter().map(|w| serde_json::json!({
                "name": w.name,
                "window_id": w.window_id,
                "agents": w.agents,
            })).collect::<Vec<_>>(),
            "agent_panes": self.agent_panes,
            "active_panes": self.active_panes,
            "namespace_epoch": self.namespace_epoch,
            "created_at": self.created_at,
        })
    }
}

/// Controller that owns the in-memory project namespace and persists it to disk.
pub struct ProjectNamespaceController {
    namespace: Option<ProjectNamespace>,
    path: Utf8PathBuf,
}

impl ProjectNamespaceController {
    pub fn new(layout: &ccbr_storage::paths::PathLayout) -> Self {
        Self {
            namespace: None,
            path: layout.ccbd_dir().join("project-namespace.json"),
        }
    }

    pub fn load(&self) -> Option<&ProjectNamespace> {
        self.namespace.as_ref()
    }

    pub fn load_from_disk(&mut self) -> Result<(), String> {
        if !self.path.exists() {
            self.namespace = None;
            return Ok(());
        }
        self.namespace = Some(
            ccbr_storage::json::JsonStore::new()
                .load::<ProjectNamespace>(&self.path)
                .map_err(|e| e.to_string())?,
        );
        Ok(())
    }

    pub fn mount(&mut self, namespace: ProjectNamespace) -> Result<(), String> {
        ccbr_storage::json::JsonStore::new()
            .save(&self.path, &namespace)
            .map_err(|e| e.to_string())?;
        self.namespace = Some(namespace);
        Ok(())
    }

    pub fn unmount(&mut self) -> Result<(), String> {
        if self.path.exists() {
            std::fs::remove_file(&self.path).map_err(|e| e.to_string())?;
        }
        self.namespace = None;
        Ok(())
    }

    pub fn update_panes(
        &mut self,
        agent_panes: HashMap<String, String>,
        active_panes: Vec<String>,
    ) -> Result<(), String> {
        let ns = match &mut self.namespace {
            Some(ns) => ns,
            None => return Ok(()),
        };
        ns.agent_panes = agent_panes;
        ns.active_panes = active_panes;
        let cloned = ns.clone();
        self.mount(cloned)
    }
}
