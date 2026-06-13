//! MCP stdio server for CCB agent-first delegation.
//!
//! This crate implements the Model Context Protocol (MCP) stdio transport for
//! CCB. It exposes three tools (`ccb_ask_agent`, `ccb_pend_agent`,
//! `ccb_ping_agent`) that delegate to the CCB daemon via its Unix socket RPC
//! interface.
//!
//! # Differences from the Python MCP server
//!
//! - Tool dispatch is a static `match` rather than a dynamic dictionary of
//!   handlers. New tools require adding a variant to the match arm and a
//!   handler function.
//! - `ccb_pend_agent` uses the daemon's `get` RPC (job_id or agent_name)
//!   instead of the Python `cli.services.pend.pend_target`, which had no Rust
//!   equivalent.
//! - `ccb_ask_agent` computes `project_id` from the resolved project root via
//!   `ccb_storage::paths::PathLayout` rather than from a Python CLI context.
//! - Optional `wait` and `timeout_s` arguments are accepted but not advertised
//!   in the tool schema, matching the Python server's behavior.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

pub const PROTOCOL_VERSION: &str = "2024-11-05";
pub const SERVER_NAME: &str = "ccb-delegation";
pub const SERVER_VERSION: &str = "0.2.0";

/// MCP JSON-RPC request received over stdin.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// MCP JSON-RPC response sent over stdout.
#[derive(Debug, Clone, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
}

impl McpResponse {
    pub fn result(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(McpError {
                code,
                message: message.into(),
            }),
        }
    }
}

/// Trait abstracting daemon RPC calls so handlers can be unit-tested.
pub trait DaemonClient: Send + Sync {
    fn call(&self, method: &str, params: Value) -> Result<Value, String>;
}

impl<T: ccb_cli::services::DaemonClient> DaemonClient for T {
    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        ccb_cli::services::DaemonClient::call(self, method, params)
    }
}

/// Build a daemon client for the given optional work directory.
///
/// If `work_dir` is `None`, the current working directory is used. The project
/// root is discovered by walking up until a `.ccb` directory is found.
pub fn build_client(
    work_dir: Option<&str>,
) -> Result<(PathBuf, ccb_cli::services::UnixDaemonClient), String> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let base = match work_dir {
        Some(wd) => expand_user_path(wd),
        None => cwd.to_string_lossy().to_string(),
    };
    let base_path = PathBuf::from(&base);
    let project_root = if base_path.is_absolute() {
        base_path
    } else {
        cwd.join(base_path)
    };
    let project_root = ccb_cli::services::resolve_project_root(&project_root, None)?;
    let socket_path = ccb_cli::services::socket_path_for_project(&project_root);
    Ok((
        project_root,
        ccb_cli::services::UnixDaemonClient::new(socket_path),
    ))
}

fn expand_user_path(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    raw.to_string()
}

/// Return the tool definitions advertised by `tools/list`.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        serde_json::json!({
            "name": "ccb_ask_agent",
            "description": "Submit a request to a named CCB agent.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent_name": {
                        "type": "string",
                        "description": "Target agent name from .ccb/ccb.config.",
                    },
                    "message": {
                        "type": "string",
                        "description": "Request text to send to the target agent.",
                    },
                    "work_dir": {
                        "type": "string",
                        "description": "Project work directory that contains .ccb/ccb.config.",
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Optional logical task id for correlation.",
                    },
                    "reply_to": {
                        "type": "string",
                        "description": "Optional job id to use as reply_to correlation.",
                    },
                },
                "required": ["agent_name", "message"],
            },
        }),
        serde_json::json!({
            "name": "ccb_pend_agent",
            "description": "Inspect the latest state/reply for a named agent or job.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "A job_id or agent name to inspect.",
                    },
                    "work_dir": {
                        "type": "string",
                        "description": "Project work directory that contains .ccb/ccb.config.",
                    },
                },
                "required": ["target"],
            },
        }),
        serde_json::json!({
            "name": "ccb_ping_agent",
            "description": "Check ccbd or mounted-agent health inside the current project.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "Agent name, all, or ccbd.",
                        "default": "ccbd",
                    },
                    "work_dir": {
                        "type": "string",
                        "description": "Project work directory that contains .ccb/ccb.config.",
                    },
                },
                "required": [],
            },
        }),
    ]
}

/// Result of handling a single MCP request.
#[derive(Debug, Clone)]
pub enum HandleOutcome {
    /// Send this response back to the client.
    Respond(McpResponse),
    /// `initialized` notification: no response needed.
    Ack,
    /// `shutdown`/`exit`: terminate the server after sending the response.
    Exit(McpResponse),
}

/// Dispatch an MCP request to the appropriate handler using a real daemon client.
pub fn handle_request(req: McpRequest, caller: &str) -> HandleOutcome {
    handle_request_with_factory(req, caller, |work_dir| {
        let (project_root, client) = build_client(work_dir)?;
        Ok((project_root, Box::new(client)))
    })
}

/// Dispatch an MCP request using the provided client factory.
///
/// The factory receives the optional `work_dir` extracted from tool arguments
/// and returns the project root plus a boxed daemon client. This allows tests
/// to inject a fake client.
pub fn handle_request_with_factory<F>(
    req: McpRequest,
    caller: &str,
    mut client_factory: F,
) -> HandleOutcome
where
    F: FnMut(Option<&str>) -> Result<(PathBuf, Box<dyn DaemonClient>), String>,
{
    match req.method.as_str() {
        "initialize" => HandleOutcome::Respond(handle_initialize(&req)),
        "initialized" => HandleOutcome::Ack,
        "tools/list" => HandleOutcome::Respond(McpResponse::result(
            req.id,
            serde_json::json!({ "tools": tool_definitions() }),
        )),
        "tools/call" => HandleOutcome::Respond(handle_tool_call(req, caller, &mut client_factory)),
        "shutdown" | "exit" => {
            let response = McpResponse::result(req.id, serde_json::json!({}));
            HandleOutcome::Exit(response)
        }
        _ => {
            if req.id.is_some() {
                HandleOutcome::Respond(McpResponse::error(
                    req.id,
                    -32601,
                    format!("unknown method: {}", req.method),
                ))
            } else {
                HandleOutcome::Ack
            }
        }
    }
}

fn handle_initialize(req: &McpRequest) -> McpResponse {
    let proto = req
        .params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or(PROTOCOL_VERSION)
        .to_string();
    McpResponse::result(
        req.id.clone(),
        serde_json::json!({
            "protocolVersion": proto,
            "capabilities": { "tools": { "list": true } },
            "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
        }),
    )
}

fn handle_tool_call<F>(req: McpRequest, caller: &str, client_factory: &mut F) -> McpResponse
where
    F: FnMut(Option<&str>) -> Result<(PathBuf, Box<dyn DaemonClient>), String>,
{
    let id = req.id.clone();
    let name = req
        .params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        return McpResponse::error(id, -32602, "missing tool name");
    }
    let args = req.params.get("arguments").cloned().unwrap_or_default();
    let args = if args.is_object() {
        args
    } else {
        serde_json::json!({})
    };

    let work_dir = args.get("work_dir").and_then(|v| v.as_str());
    let client_result = client_factory(work_dir);

    let (_project_root, client) = match client_result {
        Ok(c) => c,
        Err(e) => return McpResponse::result(id, tool_error(e)),
    };

    let result = match name.as_str() {
        "ccb_ask_agent" => ask_agent(&args, caller, &_project_root, client.as_ref()),
        "ccb_pend_agent" => pend_agent(&args, client.as_ref()),
        "ccb_ping_agent" => ping_agent(&args, client.as_ref()),
        _ => Err(format!("unknown tool: {name}")),
    };

    match result {
        Ok(payload) => McpResponse::result(id, tool_ok(payload)),
        Err(message) => McpResponse::result(id, tool_error(message)),
    }
}

/// Build a successful tool result payload.
pub fn tool_ok(payload: Value) -> Value {
    serde_json::json!({
        "content": [
            {
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap_or_else(|_| "{}".into()),
            }
        ]
    })
}

/// Build an error tool result payload.
pub fn tool_error(message: impl Into<String>) -> Value {
    serde_json::json!({
        "content": [{"type": "text", "text": message.into()}],
        "isError": true,
    })
}

fn required_text(args: &Value, field: &str) -> Option<String> {
    args.get(field)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn optional_text(args: &Value, field: &str) -> Option<String> {
    args.get(field)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn parse_bool(value: &Value, default: bool) -> bool {
    value.as_bool().unwrap_or_else(|| {
        value
            .as_str()
            .map(|s| {
                matches!(
                    s.trim().to_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(default)
    })
}

fn parse_timeout(value: &Value, default: f64) -> f64 {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
        .filter(|&v| v > 0.0)
        .unwrap_or(default)
}

fn ask_agent(
    args: &Value,
    caller: &str,
    project_root: &std::path::Path,
    client: &dyn DaemonClient,
) -> Result<Value, String> {
    let agent_name = required_text(args, "agent_name")
        .ok_or("agent_name is required")?
        .to_lowercase();
    let message = required_text(args, "message").ok_or("message is required")?;

    let wait = parse_bool(args.get("wait").unwrap_or(&serde_json::Value::Null), false);
    let timeout_s = parse_timeout(
        args.get("timeout_s").unwrap_or(&serde_json::Value::Null),
        120.0,
    );
    let task_id = optional_text(args, "task_id");
    let reply_to = optional_text(args, "reply_to");

    let layout = ccb_storage::paths::PathLayout::new(
        camino::Utf8Path::from_path(project_root).unwrap_or(camino::Utf8Path::new("/")),
    );
    let project_id = layout.project_id().to_string();

    let submit_params = serde_json::json!({
        "project_id": project_id,
        "to_agent": agent_name,
        "from_actor": caller,
        "body": message,
        "task_id": task_id,
        "reply_to": reply_to,
    });
    let receipt = client.call("submit", submit_params)?;

    let job_id = receipt
        .get("job_id")
        .and_then(|v| v.as_str())
        .ok_or("ask submission returned no job_id")?;

    let mut response = serde_json::json!({
        "project_id": project_id,
        "job_id": job_id,
        "agent_name": receipt.get("agent_name").and_then(|v| v.as_str()).unwrap_or(&agent_name),
        "target_kind": receipt.get("target_kind"),
        "target_name": receipt.get("target_name").and_then(|v| v.as_str()).unwrap_or(&agent_name),
        "status": receipt.get("status"),
    });

    if wait {
        let watch_params = serde_json::json!({
            "target": job_id,
            "start_line": 0,
            "timeout_s": timeout_s,
        });
        let terminal = client.call("watch", watch_params)?;
        response["terminal"] = serde_json::json!(true);
        response["status"] = terminal
            .get("status")
            .cloned()
            .unwrap_or_else(|| response["status"].take());
        response["reply"] = terminal
            .get("reply")
            .cloned()
            .unwrap_or(serde_json::json!(""));
    } else {
        response["terminal"] = serde_json::json!(false);
        response["reply_mode"] = serde_json::json!("async");
    }

    Ok(response)
}

fn pend_agent(args: &Value, client: &dyn DaemonClient) -> Result<Value, String> {
    let target = required_text(args, "target")
        .ok_or("target is required")?
        .to_lowercase();

    // The daemon's `get` handler accepts either a job_id or an agent_name.
    let params = if target.starts_with("job_") {
        serde_json::json!({ "job_id": target })
    } else {
        serde_json::json!({ "agent_name": target })
    };

    let payload = client.call("get", params)?;
    Ok(serde_json::json!({
        "job_id": payload.get("job_id"),
        "agent_name": payload.get("agent_name"),
        "target_kind": payload.get("target_kind"),
        "target_name": payload.get("target_name"),
        "status": payload.get("status"),
        "terminal": payload.get("terminal").and_then(|v| v.as_bool()).unwrap_or(false),
        "reply": payload.get("reply").and_then(|v| v.as_str()).unwrap_or(""),
        "cursor": payload.get("cursor"),
    }))
}

fn ping_agent(args: &Value, client: &dyn DaemonClient) -> Result<Value, String> {
    let target = optional_text(args, "target")
        .unwrap_or_else(|| "ccbd".into())
        .to_lowercase();
    client.call("ping", serde_json::json!({ "target": target }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default, Clone)]
    struct FakeClient {
        calls: Arc<Mutex<Vec<(String, Value)>>>,
        responses: Arc<Mutex<HashMap<String, Value>>>,
    }

    impl FakeClient {
        fn with_response(method: &str, response: Value) -> Self {
            let mut map = HashMap::new();
            map.insert(method.to_string(), response);
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(map)),
            }
        }

        fn with_responses(responses: HashMap<String, Value>) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(responses)),
            }
        }
    }

    impl DaemonClient for FakeClient {
        fn call(&self, method: &str, params: Value) -> Result<Value, String> {
            self.calls
                .lock()
                .unwrap()
                .push((method.to_string(), params));
            self.responses
                .lock()
                .unwrap()
                .get(method)
                .cloned()
                .ok_or_else(|| format!("no response for {method}"))
        }
    }

    type FakeFactoryResult = Result<(PathBuf, Box<dyn DaemonClient>), String>;

    fn fake_factory(client: FakeClient) -> impl FnMut(Option<&str>) -> FakeFactoryResult {
        move |_work_dir| Ok((PathBuf::from("/tmp/fake"), Box::new(client.clone())))
    }

    #[test]
    fn test_initialize_preserves_client_protocol_version() {
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(1)),
            method: "initialize".into(),
            params: json!({ "protocolVersion": "2024-11-05" }),
        };
        let outcome =
            handle_request_with_factory(req, "droid", fake_factory(FakeClient::default()));
        let HandleOutcome::Respond(resp) = outcome else {
            panic!("expected Respond");
        };
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
    }

    #[test]
    fn test_tools_list_returns_three_tools() {
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(2)),
            method: "tools/list".into(),
            params: json!({}),
        };
        let HandleOutcome::Respond(resp) =
            handle_request_with_factory(req, "droid", fake_factory(FakeClient::default()))
        else {
            panic!("expected Respond");
        };
        let tools = resp.result.unwrap()["tools"].as_array().unwrap().clone();
        assert_eq!(tools.len(), 3);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            vec!["ccb_ask_agent", "ccb_pend_agent", "ccb_ping_agent"]
        );
    }

    #[test]
    fn test_unknown_method_returns_error() {
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(3)),
            method: "foo/bar".into(),
            params: json!({}),
        };
        let HandleOutcome::Respond(resp) =
            handle_request_with_factory(req, "droid", fake_factory(FakeClient::default()))
        else {
            panic!("expected Respond");
        };
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
    }

    #[test]
    fn test_tool_ok_wraps_payload_as_text() {
        let payload = json!({"status": "ok"});
        let wrapped = tool_ok(payload);
        assert_eq!(wrapped["content"][0]["type"], "text");
        assert_eq!(wrapped["content"][0]["text"], r#"{"status":"ok"}"#);
    }

    #[test]
    fn test_tool_error_marks_error() {
        let wrapped = tool_error("boom");
        assert_eq!(wrapped["isError"], true);
        assert_eq!(wrapped["content"][0]["text"], "boom");
    }

    #[test]
    fn test_ask_agent_async_dispatches_submit() {
        let mut responses = HashMap::new();
        responses.insert(
            "submit".into(),
            json!({
                "job_id": "job_123",
                "agent_name": "claude",
                "target_kind": "agent",
                "target_name": "claude",
                "status": "accepted",
            }),
        );
        let client = FakeClient::with_responses(responses);
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(4)),
            method: "tools/call".into(),
            params: json!({
                "name": "ccb_ask_agent",
                "arguments": {
                    "agent_name": "claude",
                    "message": "hello",
                    "task_id": "task-1",
                }
            }),
        };
        let HandleOutcome::Respond(resp) =
            handle_request_with_factory(req, "droid", fake_factory(client.clone()))
        else {
            panic!("expected Respond");
        };
        let result = resp.result.unwrap();
        let content = result["content"][0]["text"].as_str().unwrap();
        let payload: Value = serde_json::from_str(content).unwrap();
        assert_eq!(payload["job_id"], "job_123");
        assert_eq!(payload["reply_mode"], "async");
        assert_eq!(payload["terminal"], false);
        assert!(!payload["project_id"].as_str().unwrap().is_empty());

        let calls = client.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "submit");
        assert_eq!(calls[0].1["to_agent"], "claude");
        assert_eq!(calls[0].1["body"], "hello");
        assert_eq!(calls[0].1["from_actor"], "droid");
        assert_eq!(calls[0].1["task_id"], "task-1");
    }

    #[test]
    fn test_ask_agent_wait_polls_watch() {
        let mut responses = HashMap::new();
        responses.insert(
            "submit".into(),
            json!({
                "job_id": "job_123",
                "agent_name": "claude",
                "target_kind": "agent",
                "target_name": "claude",
                "status": "accepted",
            }),
        );
        responses.insert(
            "watch".into(),
            json!({
                "status": "completed",
                "reply": "done",
            }),
        );
        let client = FakeClient::with_responses(responses);
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(5)),
            method: "tools/call".into(),
            params: json!({
                "name": "ccb_ask_agent",
                "arguments": {
                    "agent_name": "claude",
                    "message": "hello",
                    "wait": true,
                    "timeout_s": 30.0,
                }
            }),
        };
        let HandleOutcome::Respond(resp) =
            handle_request_with_factory(req, "droid", fake_factory(client.clone()))
        else {
            panic!("expected Respond");
        };
        let result = resp.result.unwrap();
        let content = result["content"][0]["text"].as_str().unwrap();
        let payload: Value = serde_json::from_str(content).unwrap();
        assert_eq!(payload["terminal"], true);
        assert_eq!(payload["status"], "completed");
        assert_eq!(payload["reply"], "done");

        let calls = client.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].0, "watch");
        assert_eq!(calls[1].1["target"], "job_123");
    }

    #[test]
    fn test_pend_agent_uses_get_by_job_id() {
        let client = FakeClient::with_response(
            "get",
            json!({
                "job_id": "job_123",
                "agent_name": "claude",
                "status": "completed",
                "reply": "done",
            }),
        );
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(6)),
            method: "tools/call".into(),
            params: json!({
                "name": "ccb_pend_agent",
                "arguments": { "target": "job_123" }
            }),
        };
        let HandleOutcome::Respond(resp) =
            handle_request_with_factory(req, "droid", fake_factory(client.clone()))
        else {
            panic!("expected Respond");
        };
        let result = resp.result.unwrap();
        let content = result["content"][0]["text"].as_str().unwrap();
        let payload: Value = serde_json::from_str(content).unwrap();
        assert_eq!(payload["job_id"], "job_123");
        assert_eq!(payload["status"], "completed");

        let calls = client.calls.lock().unwrap();
        assert_eq!(calls[0].0, "get");
        assert_eq!(calls[0].1["job_id"], "job_123");
        assert!(calls[0].1["agent_name"].is_null());
    }

    #[test]
    fn test_pend_agent_uses_get_by_agent_name() {
        let client = FakeClient::with_response(
            "get",
            json!({
                "agent_name": "claude",
                "job_id": "job_456",
                "status": "running",
            }),
        );
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(7)),
            method: "tools/call".into(),
            params: json!({
                "name": "ccb_pend_agent",
                "arguments": { "target": "claude" }
            }),
        };
        let HandleOutcome::Respond(resp) =
            handle_request_with_factory(req, "droid", fake_factory(client.clone()))
        else {
            panic!("expected Respond");
        };
        let result = resp.result.unwrap();
        let content = result["content"][0]["text"].as_str().unwrap();
        let payload: Value = serde_json::from_str(content).unwrap();
        assert_eq!(payload["agent_name"], "claude");

        let calls = client.calls.lock().unwrap();
        assert_eq!(calls[0].1["agent_name"], "claude");
    }

    #[test]
    fn test_ping_agent_defaults_to_ccbd() {
        let client = FakeClient::with_response("ping", json!({ "pong": true, "target": "ccbd" }));
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(8)),
            method: "tools/call".into(),
            params: json!({
                "name": "ccb_ping_agent",
                "arguments": {}
            }),
        };
        let HandleOutcome::Respond(resp) =
            handle_request_with_factory(req, "droid", fake_factory(client.clone()))
        else {
            panic!("expected Respond");
        };
        let result = resp.result.unwrap();
        let content = result["content"][0]["text"].as_str().unwrap();
        let payload: Value = serde_json::from_str(content).unwrap();
        assert_eq!(payload["pong"], true);

        let calls = client.calls.lock().unwrap();
        assert_eq!(calls[0].1["target"], "ccbd");
    }

    #[test]
    fn test_missing_tool_name_returns_error() {
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(9)),
            method: "tools/call".into(),
            params: json!({ "name": "", "arguments": {} }),
        };
        let HandleOutcome::Respond(resp) =
            handle_request_with_factory(req, "droid", fake_factory(FakeClient::default()))
        else {
            panic!("expected Respond");
        };
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32602);
    }
}
