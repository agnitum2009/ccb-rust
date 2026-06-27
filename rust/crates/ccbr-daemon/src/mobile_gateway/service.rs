use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde_json::{json, Value};
use thiserror::Error;

use super::pairing::MobileGatewayPairingStore;

const SCHEMA_VERSION: i64 = 1;
const BASE_CAPABILITIES: &[&str] = &["http_json", "project_view"];
const PAIRING_CAPABILITIES: &[&str] = &[
    "pairing",
    "device_tokens",
    "lifecycle",
    "focus",
    "terminal_open",
    "websocket_terminal",
    "terminal_history",
    "file_upload",
    "file_download",
];
const REDACTED_NAMESPACE_KEYS: &[&str] = &["socket_path", "session_name"];

#[derive(Debug, Error)]
#[error("{message}")]
pub struct MobileGatewayError {
    pub message: String,
    pub status_code: u16,
}

impl MobileGatewayError {
    pub fn new(message: impl Into<String>, status_code: u16) -> Self {
        Self {
            message: message.into(),
            status_code,
        }
    }
}

pub type Result<T> = std::result::Result<T, MobileGatewayError>;

pub trait MobileGatewayProjectClient: Clone {
    fn ping(&self) -> std::result::Result<Value, String>;
    fn project_view(&self) -> std::result::Result<Value, String>;
}

#[derive(Debug, Clone)]
pub struct MobileGatewayProject<C> {
    pub project_id: String,
    pub project_root: PathBuf,
    pub display_name: Option<String>,
    pub client: C,
}

impl<C> MobileGatewayProject<C> {
    pub fn new(
        project_id: impl Into<String>,
        project_root: impl Into<PathBuf>,
        display_name: Option<String>,
        client: C,
    ) -> Result<Self> {
        let project_id = project_id.into().trim().to_string();
        if project_id.is_empty() {
            return Err(MobileGatewayError::new("project_id cannot be empty", 400));
        }
        Ok(Self {
            project_id,
            project_root: project_root.into(),
            display_name: display_name.and_then(|value| {
                let trimmed = value.trim().to_string();
                (!trimmed.is_empty()).then_some(trimmed)
            }),
            client,
        })
    }

    pub fn public_display_name(&self) -> String {
        self.display_name.clone().unwrap_or_else(|| {
            self.project_root
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or(&self.project_id)
                .to_string()
        })
    }
}

#[derive(Debug, Clone)]
pub struct MobileGatewayProjectRegistry<C> {
    projects: Vec<MobileGatewayProject<C>>,
}

impl<C: Clone> MobileGatewayProjectRegistry<C> {
    pub fn new(projects: Vec<MobileGatewayProject<C>>) -> Result<Self> {
        if projects.is_empty() {
            return Err(MobileGatewayError::new(
                "mobile gateway project registry cannot be empty",
                400,
            ));
        }
        for (index, project) in projects.iter().enumerate() {
            if projects
                .iter()
                .skip(index + 1)
                .any(|other| other.project_id == project.project_id)
            {
                return Err(MobileGatewayError::new(
                    format!("duplicate mobile gateway project: {}", project.project_id),
                    400,
                ));
            }
        }
        Ok(Self { projects })
    }

    pub fn current_project(
        project_id: impl Into<String>,
        project_root: impl Into<PathBuf>,
        client: C,
    ) -> Result<Self> {
        Self::new(vec![MobileGatewayProject::new(
            project_id,
            project_root,
            None,
            client,
        )?])
    }

    pub fn projects(&self) -> &[MobileGatewayProject<C>] {
        &self.projects
    }

    pub fn default_project(&self) -> &MobileGatewayProject<C> {
        &self.projects[0]
    }

    pub fn get(&self, project_id: &str) -> Option<&MobileGatewayProject<C>> {
        let requested = project_id.trim();
        self.projects
            .iter()
            .find(|project| project.project_id == requested)
    }
}

#[derive(Debug, Clone)]
pub struct MobileGatewayService<C> {
    project_id: String,
    project_root: PathBuf,
    registry: MobileGatewayProjectRegistry<C>,
    mode: String,
    pairing_store: Option<MobileGatewayPairingStore>,
}

impl<C: MobileGatewayProjectClient> MobileGatewayService<C> {
    pub fn current_project(
        project_id: impl Into<String>,
        project_root: impl Into<PathBuf>,
        client: C,
        mobile_dir: Option<&Path>,
    ) -> Result<Self> {
        let project_id = project_id.into();
        let project_root = project_root.into();
        let registry = MobileGatewayProjectRegistry::current_project(
            project_id.clone(),
            project_root.clone(),
            client,
        )?;
        Ok(Self {
            project_id,
            project_root,
            registry,
            mode: "loopback_current_project".to_string(),
            pairing_store: mobile_dir.map(MobileGatewayPairingStore::new),
        })
    }

    pub fn with_registry(
        project_id: impl Into<String>,
        project_root: impl Into<PathBuf>,
        registry: MobileGatewayProjectRegistry<C>,
        mode: impl Into<String>,
        mobile_dir: Option<&Path>,
    ) -> Self {
        let mode = mode.into();
        Self {
            project_id: project_id.into(),
            project_root: project_root.into(),
            registry,
            mode: if mode.trim().is_empty() {
                "loopback_current_project".to_string()
            } else {
                mode
            },
            pairing_store: mobile_dir.map(MobileGatewayPairingStore::new),
        }
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn health_payload(&self) -> Value {
        match self.registry.default_project().client.ping() {
            Ok(payload) => json!({
                "schema_version": SCHEMA_VERSION,
                "status": "ok",
                "server_time": utc_now(),
                "mode": self.mode,
                "project_id": self.project_id,
                "capabilities": self.capabilities(),
                "ccbd": ccbd_health_summary(&payload),
            }),
            Err(error) => json!({
                "schema_version": SCHEMA_VERSION,
                "status": "degraded",
                "server_time": utc_now(),
                "mode": self.mode,
                "project_id": self.project_id,
                "capabilities": self.capabilities(),
                "ccbd": {
                    "reachable": false,
                    "error": error_text(&error),
                },
            }),
        }
    }

    pub fn projects_payload(&self) -> Value {
        let projects = self
            .registry
            .projects()
            .iter()
            .map(|project| {
                let ccbd = project.client.ping().unwrap_or_else(|_| {
                    json!({
                        "health": "unreachable",
                        "mount_state": "unavailable",
                        "error": "project unavailable",
                    })
                });
                let mut item = json!({
                    "id": project.project_id,
                    "display_name": project.public_display_name(),
                    "root": project.project_root.to_string_lossy(),
                    "health": ccbd.get("health").and_then(Value::as_str).unwrap_or("unknown"),
                    "mount_state": ccbd.get("mount_state").and_then(Value::as_str).unwrap_or(""),
                    "capabilities": self.capabilities(),
                });
                if let Some(error) = ccbd.get("error").and_then(Value::as_str) {
                    if !error.is_empty() {
                        item["error"] = Value::String(error.to_string());
                    }
                }
                item
            })
            .collect::<Vec<_>>();
        json!({"schema_version": SCHEMA_VERSION, "projects": projects})
    }

    pub fn project_view_payload(&self, project_id: &str) -> Result<Value> {
        let project = self.require_project(project_id)?;
        let payload = project
            .client
            .project_view()
            .map_err(|error| MobileGatewayError::new(error_text(&error), 503))?;
        Ok(redact_project_view_payload(&payload))
    }

    fn require_project(&self, project_id: &str) -> Result<&MobileGatewayProject<C>> {
        self.registry
            .get(project_id)
            .ok_or_else(|| MobileGatewayError::new("unknown project", 404))
    }

    fn capabilities(&self) -> Vec<&'static str> {
        let mut values = BASE_CAPABILITIES.to_vec();
        if self.pairing_store.is_some() {
            values.extend(PAIRING_CAPABILITIES);
        }
        values
    }
}

fn redact_project_view_payload(payload: &Value) -> Value {
    let mut redacted = payload.clone();
    if let Some(namespace) = redacted
        .get_mut("view")
        .and_then(|view| view.get_mut("namespace"))
        .and_then(Value::as_object_mut)
    {
        for key in REDACTED_NAMESPACE_KEYS {
            namespace.remove(*key);
        }
    }
    redacted
}

fn ccbd_health_summary(payload: &Value) -> Value {
    json!({
        "reachable": true,
        "project_id": payload.get("project_id").cloned().unwrap_or(Value::Null),
        "mount_state": payload.get("mount_state").cloned().unwrap_or(Value::Null),
        "health": payload.get("health").cloned().unwrap_or(Value::Null),
        "namespace_epoch": payload.get("namespace_epoch").cloned().unwrap_or(Value::Null),
        "namespace_ui_attachable": payload.get("namespace_ui_attachable").cloned().unwrap_or(Value::Null),
    })
}

fn utc_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)
}

fn error_text(error: &str) -> String {
    let trimmed = error.trim();
    if trimmed.is_empty() {
        "Error".to_string()
    } else {
        trimmed.to_string()
    }
}
