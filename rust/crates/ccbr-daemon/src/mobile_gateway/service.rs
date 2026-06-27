use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde_json::{json, Value};
use thiserror::Error;

use super::pairing::{MobileGatewayPairingError, MobileGatewayPairingStore};

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

    fn project_focus_agent(
        &self,
        _agent: &str,
        _namespace_epoch: Option<i64>,
    ) -> std::result::Result<Value, String> {
        Err("project_focus_agent is not implemented".to_string())
    }

    fn project_focus_window(
        &self,
        _window: &str,
        _namespace_epoch: Option<i64>,
    ) -> std::result::Result<Value, String> {
        Err("project_focus_window is not implemented".to_string())
    }

    fn stop_all(&self, _force: bool) -> std::result::Result<Value, String> {
        Err("stop_all is not implemented".to_string())
    }
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

    pub fn create_pairing_payload(
        &self,
        gateway_url: &str,
        route_provider: Option<&str>,
        scopes: impl IntoIterator<Item = impl AsRef<str>>,
        expires_seconds: Option<i64>,
    ) -> Result<Value> {
        let store = self.require_pairing_store()?;
        store
            .write_gateway_state(
                &self.project_id,
                gateway_url,
                route_provider.unwrap_or("lan"),
                self.capabilities(),
            )
            .map_err(pairing_error)?;
        store
            .create_pairing_payload(
                &self.project_id,
                gateway_url,
                route_provider,
                scopes,
                expires_seconds,
            )
            .map_err(pairing_error)
    }

    pub fn dispatch_get(&self, path: &str, bearer_token: Option<&str>) -> Result<(u16, Value)> {
        let route = route_path(path);
        if route == "/v1/health" {
            let payload = self.health_payload();
            let status = if payload.get("status").and_then(Value::as_str) == Some("degraded") {
                503
            } else {
                200
            };
            return Ok((status, payload));
        }
        if route == "/v1/projects" {
            return Ok((200, self.projects_payload()));
        }
        if let Some(project_id) = project_view_route(&route) {
            self.authenticate(bearer_token, ["view"])?;
            return Ok((200, self.project_view_payload(&project_id)?));
        }
        if route == "/v1/devices/me" {
            let device = self.authenticate(bearer_token, ["view"])?;
            return Ok((
                200,
                json!({
                    "schema_version": SCHEMA_VERSION,
                    "status": "ok",
                    "device": device.public_payload(),
                }),
            ));
        }
        Err(MobileGatewayError::new("not found", 404))
    }

    pub fn dispatch_post(
        &self,
        path: &str,
        body: &Value,
        bearer_token: Option<&str>,
    ) -> Result<(u16, Value)> {
        let route = route_path(path);
        if route == "/v1/pairing/claim" {
            let payload = body.as_object();
            let pairing_code = payload
                .and_then(|obj| obj.get("pairing_code"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let device_name = payload
                .and_then(|obj| obj.get("device_name"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let device_id = payload
                .and_then(|obj| obj.get("device_id"))
                .and_then(Value::as_str);
            let result = self
                .require_pairing_store()?
                .claim_pairing(pairing_code, device_name, device_id)
                .map_err(pairing_error)?;
            return Ok((201, result));
        }
        if let Some((project_id, action)) = project_action_route(&route) {
            let payload = match action.as_str() {
                "focus-agent" => self.focus_agent(
                    &project_id,
                    body_text(body, "agent"),
                    body_i64(body, "namespace_epoch"),
                    bearer_token,
                )?,
                "focus-window" => self.focus_window(
                    &project_id,
                    body_text(body, "window"),
                    body_i64(body, "namespace_epoch"),
                    bearer_token,
                )?,
                "lifecycle" => self.project_lifecycle(&project_id, body, bearer_token)?,
                _ => return Err(MobileGatewayError::new("not found", 404)),
            };
            return Ok((200, payload));
        }
        if let Some(device_id) = device_revoke_route(&route) {
            let result = self
                .require_pairing_store()?
                .revoke_device(&device_id, bearer_token.unwrap_or(""))
                .map_err(pairing_error)?;
            return Ok((200, result));
        }
        Err(MobileGatewayError::new("not found", 404))
    }

    fn focus_agent(
        &self,
        project_id: &str,
        agent: String,
        namespace_epoch: Option<i64>,
        bearer_token: Option<&str>,
    ) -> Result<Value> {
        let project = self.require_project(project_id)?;
        self.authenticate(bearer_token, ["focus"])?;
        if agent.trim().is_empty() {
            return Err(MobileGatewayError::new("agent is required", 400));
        }
        let focus = project
            .client
            .project_focus_agent(&agent, namespace_epoch)
            .map_err(|error| MobileGatewayError::new(error_text(&error), 503))?;
        self.focused_project_view_payload(project, &focus)
    }

    fn focus_window(
        &self,
        project_id: &str,
        window: String,
        namespace_epoch: Option<i64>,
        bearer_token: Option<&str>,
    ) -> Result<Value> {
        let project = self.require_project(project_id)?;
        self.authenticate(bearer_token, ["focus"])?;
        if window.trim().is_empty() {
            return Err(MobileGatewayError::new("window is required", 400));
        }
        let focus = project
            .client
            .project_focus_window(&window, namespace_epoch)
            .map_err(|error| MobileGatewayError::new(error_text(&error), 503))?;
        self.focused_project_view_payload(project, &focus)
    }

    fn project_lifecycle(
        &self,
        project_id: &str,
        body: &Value,
        bearer_token: Option<&str>,
    ) -> Result<Value> {
        let project = self.require_project(project_id)?;
        self.authenticate(bearer_token, ["lifecycle"])?;
        let body_project_id = body_text(body, "project_id");
        if !body_project_id.is_empty() && body_project_id != project.project_id {
            return Err(MobileGatewayError::new(
                "request project_id does not match route",
                400,
            ));
        }
        let action = body_text(body, "action").to_ascii_lowercase();
        if !matches!(action.as_str(), "wake" | "open" | "close" | "stop") {
            return Err(MobileGatewayError::new("unsupported lifecycle action", 400));
        }
        if action == "wake" || action == "open" {
            let mut response = self.project_view_payload(&project.project_id)?;
            response["schema_version"] = json!(SCHEMA_VERSION);
            response["status"] = json!("ok");
            response["project_id"] = json!(project.project_id);
            response["lifecycle"] = lifecycle_result(
                &action,
                "running",
                if action == "wake" {
                    "already_running"
                } else {
                    "opened"
                },
                false,
                None,
            );
            return Ok(response);
        }
        if action == "close" {
            return Ok(json!({
                "schema_version": SCHEMA_VERSION,
                "status": "ok",
                "project_id": project.project_id,
                "lifecycle": lifecycle_result("close", "running", "mobile_view_closed", false, None),
            }));
        }
        let stop_result = project
            .client
            .stop_all(false)
            .map_err(|error| MobileGatewayError::new(error_text(&error), 503))?;
        Ok(json!({
            "schema_version": SCHEMA_VERSION,
            "status": "ok",
            "project_id": project.project_id,
            "lifecycle": lifecycle_result("stop", "stopping", "ccbd_stop_requested", false, Some(stop_result)),
        }))
    }

    fn focused_project_view_payload(
        &self,
        project: &MobileGatewayProject<C>,
        focus: &Value,
    ) -> Result<Value> {
        let mut payload = self.project_view_payload(&project.project_id)?;
        payload["focus"] = focus.clone();
        Ok(payload)
    }

    fn authenticate(
        &self,
        bearer_token: Option<&str>,
        required_scopes: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<super::pairing::AuthenticatedDevice> {
        self.require_pairing_store()?
            .authenticate_device(bearer_token.unwrap_or(""), required_scopes)
            .map_err(pairing_error)
    }

    fn require_pairing_store(&self) -> Result<&MobileGatewayPairingStore> {
        self.pairing_store
            .as_ref()
            .ok_or_else(|| MobileGatewayError::new("mobile pairing store is not configured", 503))
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

fn route_path(path: &str) -> String {
    let before_query = path.split('?').next().unwrap_or(path).trim();
    let route = before_query.trim_end_matches('/');
    if route.is_empty() {
        "/".to_string()
    } else {
        route.to_string()
    }
}

fn project_view_route(route: &str) -> Option<String> {
    let prefix = "/v1/projects/";
    let suffix = "/view";
    route
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_suffix(suffix))
        .map(|value| value.trim_matches('/').to_string())
        .filter(|value| !value.is_empty())
}

fn project_action_route(route: &str) -> Option<(String, String)> {
    let prefix = "/v1/projects/";
    let rest = route.strip_prefix(prefix)?.trim_matches('/');
    let parts = rest.split('/').collect::<Vec<_>>();
    if parts.len() != 2 {
        return None;
    }
    let action = parts[1].to_string();
    if !matches!(
        action.as_str(),
        "focus-agent" | "focus-window" | "lifecycle" | "terminals"
    ) {
        return None;
    }
    let project_id = parts[0].trim().to_string();
    (!project_id.is_empty()).then_some((project_id, action))
}

fn body_text(body: &Value, key: &str) -> String {
    body.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn body_i64(body: &Value, key: &str) -> Option<i64> {
    body.get(key).and_then(Value::as_i64)
}

fn lifecycle_result(
    action: &str,
    state: &str,
    effect: &str,
    forced: bool,
    result: Option<Value>,
) -> Value {
    let mut payload = json!({
        "action": action,
        "state": state,
        "effect": effect,
        "forced": forced,
        "ccb_authority": true,
        "tmux_kill_server": false,
        "updated_at": utc_now(),
    });
    if let Some(result) = result {
        payload["result"] = result;
    }
    payload
}

fn device_revoke_route(route: &str) -> Option<String> {
    let prefix = "/v1/devices/";
    let suffix = "/revoke";
    route
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_suffix(suffix))
        .map(|value| value.trim_matches('/').to_string())
        .filter(|value| !value.is_empty())
}

fn pairing_error(error: MobileGatewayPairingError) -> MobileGatewayError {
    MobileGatewayError::new(error.message, error.status_code)
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
