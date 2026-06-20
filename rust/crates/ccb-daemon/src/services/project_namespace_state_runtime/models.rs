use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::common::{
    clean_text, require_record_type, require_schema_version, NAMESPACE_EVENT_RECORD_TYPE,
    NAMESPACE_STATE_RECORD_TYPE,
};
use crate::models::api_models::common::SCHEMA_VERSION;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectNamespaceState {
    pub project_id: String,
    pub namespace_epoch: u64,
    pub tmux_socket_path: String,
    pub tmux_session_name: String,
    #[serde(default = "default_layout_version")]
    pub layout_version: u64,
    pub layout_signature: Option<String>,
    pub control_window_name: Option<String>,
    pub control_window_id: Option<String>,
    pub workspace_window_name: Option<String>,
    pub workspace_window_id: Option<String>,
    #[serde(default = "default_workspace_epoch")]
    pub workspace_epoch: u64,
    #[serde(default = "default_ui_attachable")]
    pub ui_attachable: bool,
    pub last_started_at: Option<String>,
    pub last_destroyed_at: Option<String>,
    pub last_destroy_reason: Option<String>,
}

fn default_layout_version() -> u64 {
    1
}

fn default_workspace_epoch() -> u64 {
    1
}

fn default_ui_attachable() -> bool {
    true
}

fn require_non_empty_text(value: &str, field_name: &str) -> anyhow::Result<()> {
    if value.trim().is_empty() {
        anyhow::bail!("{field_name} cannot be empty");
    }
    Ok(())
}

fn require_positive_int(value: u64, field_name: &str) -> anyhow::Result<()> {
    if value == 0 {
        anyhow::bail!("{field_name} must be positive");
    }
    Ok(())
}

fn require_optional_non_empty_text(value: &Option<String>, field_name: &str) -> anyhow::Result<()> {
    if let Some(v) = value {
        require_non_empty_text(v, field_name)?;
    }
    Ok(())
}

impl ProjectNamespaceState {
    pub fn new(
        project_id: &str,
        namespace_epoch: u64,
        tmux_socket_path: &str,
        tmux_session_name: &str,
    ) -> anyhow::Result<Self> {
        let state = Self {
            project_id: project_id.to_string(),
            namespace_epoch,
            tmux_socket_path: tmux_socket_path.to_string(),
            tmux_session_name: tmux_session_name.to_string(),
            layout_version: 1,
            layout_signature: None,
            control_window_name: None,
            control_window_id: None,
            workspace_window_name: None,
            workspace_window_id: None,
            workspace_epoch: 1,
            ui_attachable: true,
            last_started_at: None,
            last_destroyed_at: None,
            last_destroy_reason: None,
        };
        state.validate()?;
        Ok(state)
    }

    fn validate(&self) -> anyhow::Result<()> {
        require_non_empty_text(&self.project_id, "project_id")?;
        require_positive_int(self.namespace_epoch, "namespace_epoch")?;
        require_non_empty_text(&self.tmux_socket_path, "tmux_socket_path")?;
        require_non_empty_text(&self.tmux_session_name, "tmux_session_name")?;
        require_positive_int(self.layout_version, "layout_version")?;
        require_optional_non_empty_text(&self.layout_signature, "layout_signature")?;
        require_optional_non_empty_text(&self.control_window_name, "control_window_name")?;
        require_optional_non_empty_text(&self.control_window_id, "control_window_id")?;
        require_optional_non_empty_text(&self.workspace_window_name, "workspace_window_name")?;
        require_optional_non_empty_text(&self.workspace_window_id, "workspace_window_id")?;
        require_positive_int(self.workspace_epoch, "workspace_epoch")?;
        Ok(())
    }

    pub fn with_layout_version(mut self, version: u64) -> Self {
        self.layout_version = version;
        self
    }

    pub fn with_layout_signature(mut self, signature: &str) -> Self {
        self.layout_signature = Some(signature.to_string());
        self
    }

    pub fn with_control_window(mut self, window_name: &str, window_id: &str) -> Self {
        self.control_window_name = Some(window_name.to_string());
        self.control_window_id = Some(window_id.to_string());
        self
    }

    pub fn with_workspace_window(mut self, window_name: &str, window_id: &str) -> Self {
        self.workspace_window_name = Some(window_name.to_string());
        self.workspace_window_id = Some(window_id.to_string());
        self
    }

    pub fn with_workspace_epoch(mut self, epoch: u64) -> Self {
        self.workspace_epoch = epoch;
        self
    }

    pub fn with_started(mut self, occurred_at: &str, ui_attachable: bool) -> Self {
        self.ui_attachable = ui_attachable;
        self.last_started_at = Some(occurred_at.to_string());
        self
    }

    pub fn with_destroyed(mut self, occurred_at: &str, reason: &str) -> Self {
        self.ui_attachable = false;
        self.last_destroyed_at = Some(occurred_at.to_string());
        self.last_destroy_reason = Some(
            reason
                .to_string()
                .trim()
                .to_string()
                .clone()
                .if_empty_else(|| "destroyed".to_string()),
        );
        self
    }

    pub fn to_record(&self) -> Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": NAMESPACE_STATE_RECORD_TYPE,
            "project_id": self.project_id,
            "namespace_epoch": self.namespace_epoch,
            "tmux_socket_path": self.tmux_socket_path,
            "tmux_session_name": self.tmux_session_name,
            "layout_version": self.layout_version,
            "layout_signature": self.layout_signature,
            "control_window_name": self.control_window_name,
            "control_window_id": self.control_window_id,
            "workspace_window_name": self.workspace_window_name,
            "workspace_window_id": self.workspace_window_id,
            "workspace_epoch": self.workspace_epoch,
            "ui_attachable": self.ui_attachable,
            "last_started_at": self.last_started_at,
            "last_destroyed_at": self.last_destroyed_at,
            "last_destroy_reason": self.last_destroy_reason,
        })
    }

    pub fn from_record(payload: &Value) -> anyhow::Result<Self> {
        require_schema_version(payload)?;
        require_record_type(payload, NAMESPACE_STATE_RECORD_TYPE)?;
        let state = Self {
            project_id: clean_text(payload.get("project_id"))
                .ok_or_else(|| anyhow::anyhow!("project_id is required"))?,
            namespace_epoch: payload
                .get("namespace_epoch")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("namespace_epoch is required"))?,
            tmux_socket_path: clean_text(payload.get("tmux_socket_path"))
                .ok_or_else(|| anyhow::anyhow!("tmux_socket_path is required"))?,
            tmux_session_name: clean_text(payload.get("tmux_session_name"))
                .ok_or_else(|| anyhow::anyhow!("tmux_session_name is required"))?,
            layout_version: payload
                .get("layout_version")
                .and_then(|v| v.as_u64())
                .unwrap_or(1),
            layout_signature: clean_text(payload.get("layout_signature")),
            control_window_name: clean_text(payload.get("control_window_name")),
            control_window_id: clean_text(payload.get("control_window_id")),
            workspace_window_name: clean_text(payload.get("workspace_window_name")),
            workspace_window_id: clean_text(payload.get("workspace_window_id")),
            workspace_epoch: payload
                .get("workspace_epoch")
                .and_then(|v| v.as_u64())
                .unwrap_or(1),
            ui_attachable: payload
                .get("ui_attachable")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            last_started_at: clean_text(payload.get("last_started_at")),
            last_destroyed_at: clean_text(payload.get("last_destroyed_at")),
            last_destroy_reason: clean_text(payload.get("last_destroy_reason")),
        };
        state.validate()?;
        Ok(state)
    }

    pub fn summary_fields(&self) -> HashMap<String, Value> {
        let mut map = HashMap::new();
        map.insert(
            "namespace_epoch".to_string(),
            serde_json::json!(self.namespace_epoch),
        );
        map.insert(
            "namespace_tmux_socket_path".to_string(),
            serde_json::json!(self.tmux_socket_path),
        );
        map.insert(
            "namespace_tmux_session_name".to_string(),
            serde_json::json!(self.tmux_session_name),
        );
        map.insert(
            "namespace_layout_version".to_string(),
            serde_json::json!(self.layout_version),
        );
        map.insert(
            "namespace_control_window_name".to_string(),
            serde_json::json!(self.control_window_name),
        );
        map.insert(
            "namespace_control_window_id".to_string(),
            serde_json::json!(self.control_window_id),
        );
        map.insert(
            "namespace_workspace_window_name".to_string(),
            serde_json::json!(self.workspace_window_name),
        );
        map.insert(
            "namespace_workspace_window_id".to_string(),
            serde_json::json!(self.workspace_window_id),
        );
        map.insert(
            "namespace_workspace_epoch".to_string(),
            serde_json::json!(self.workspace_epoch),
        );
        map.insert(
            "namespace_ui_attachable".to_string(),
            serde_json::json!(self.ui_attachable),
        );
        map.insert(
            "namespace_last_started_at".to_string(),
            serde_json::json!(self.last_started_at),
        );
        map.insert(
            "namespace_last_destroyed_at".to_string(),
            serde_json::json!(self.last_destroyed_at),
        );
        map.insert(
            "namespace_last_destroy_reason".to_string(),
            serde_json::json!(self.last_destroy_reason),
        );
        map
    }
}

trait IfEmptyElse {
    fn if_empty_else<F: FnOnce() -> String>(self, f: F) -> String;
}

impl IfEmptyElse for String {
    fn if_empty_else<F: FnOnce() -> String>(self, f: F) -> String {
        if self.trim().is_empty() {
            f()
        } else {
            self
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectNamespaceEvent {
    pub event_kind: String,
    pub project_id: String,
    pub occurred_at: String,
    pub namespace_epoch: Option<u64>,
    pub tmux_socket_path: Option<String>,
    pub tmux_session_name: Option<String>,
    #[serde(default)]
    pub details: HashMap<String, Value>,
}

impl ProjectNamespaceEvent {
    pub fn new(event_kind: &str, project_id: &str, occurred_at: &str) -> anyhow::Result<Self> {
        let event = Self {
            event_kind: event_kind.to_string(),
            project_id: project_id.to_string(),
            occurred_at: occurred_at.to_string(),
            namespace_epoch: None,
            tmux_socket_path: None,
            tmux_session_name: None,
            details: HashMap::new(),
        };
        event.validate()?;
        Ok(event)
    }

    fn validate(&self) -> anyhow::Result<()> {
        require_non_empty_text(&self.event_kind, "event_kind")?;
        require_non_empty_text(&self.project_id, "project_id")?;
        require_non_empty_text(&self.occurred_at, "occurred_at")?;
        if let Some(epoch) = self.namespace_epoch {
            require_positive_int(epoch, "namespace_epoch")?;
        }
        Ok(())
    }

    pub fn with_namespace_epoch(mut self, epoch: u64) -> Self {
        self.namespace_epoch = Some(epoch);
        self
    }

    pub fn with_socket_path(mut self, path: &str) -> Self {
        self.tmux_socket_path = Some(path.to_string());
        self
    }

    pub fn with_session_name(mut self, name: &str) -> Self {
        self.tmux_session_name = Some(name.to_string());
        self
    }

    pub fn with_details(mut self, details: HashMap<String, Value>) -> Self {
        self.details = details;
        self
    }

    pub fn to_record(&self) -> Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": NAMESPACE_EVENT_RECORD_TYPE,
            "event_kind": self.event_kind,
            "project_id": self.project_id,
            "occurred_at": self.occurred_at,
            "namespace_epoch": self.namespace_epoch,
            "tmux_socket_path": self.tmux_socket_path,
            "tmux_session_name": self.tmux_session_name,
            "details": self.details,
        })
    }

    pub fn from_record(payload: &Value) -> anyhow::Result<Self> {
        require_schema_version(payload)?;
        require_record_type(payload, NAMESPACE_EVENT_RECORD_TYPE)?;
        let details = match payload.get("details") {
            Some(Value::Object(m)) => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            Some(_) => anyhow::bail!("details must be an object"),
            None => HashMap::new(),
        };
        let event = Self {
            event_kind: clean_text(payload.get("event_kind"))
                .ok_or_else(|| anyhow::anyhow!("event_kind is required"))?,
            project_id: clean_text(payload.get("project_id"))
                .ok_or_else(|| anyhow::anyhow!("project_id is required"))?,
            occurred_at: clean_text(payload.get("occurred_at"))
                .ok_or_else(|| anyhow::anyhow!("occurred_at is required"))?,
            namespace_epoch: payload.get("namespace_epoch").and_then(|v| v.as_u64()),
            tmux_socket_path: clean_text(payload.get("tmux_socket_path")),
            tmux_session_name: clean_text(payload.get("tmux_session_name")),
            details,
        };
        event.validate()?;
        Ok(event)
    }

    pub fn summary_fields(&self) -> HashMap<String, Value> {
        let mut map = HashMap::new();
        map.insert(
            "namespace_last_event_kind".to_string(),
            serde_json::json!(self.event_kind),
        );
        map.insert(
            "namespace_last_event_at".to_string(),
            serde_json::json!(self.occurred_at),
        );
        map.insert(
            "namespace_last_event_epoch".to_string(),
            serde_json::json!(self.namespace_epoch),
        );
        map.insert(
            "namespace_last_event_socket_path".to_string(),
            serde_json::json!(self.tmux_socket_path),
        );
        map.insert(
            "namespace_last_event_session_name".to_string(),
            serde_json::json!(self.tmux_session_name),
        );
        map
    }
}
