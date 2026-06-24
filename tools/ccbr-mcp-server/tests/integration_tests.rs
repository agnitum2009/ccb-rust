//! Integration tests for the CCBR MCP delegation server.
//!
//! These tests exercise the public API from outside the crate, using a fake
//! `DaemonClient` to verify tool definitions and ask/pend/ping dispatch without
//! a running daemon.

use ccbr_mcp_server::{DaemonClient, HandleOutcome, McpRequest};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const FAKE_PROJECT_ROOT: &str = "/tmp/fake";

const JOB_ID: &str = "job_id";
const STATUS: &str = "status";
const TERMINAL: &str = "terminal";
const REPLY_MODE: &str = "reply_mode";

#[derive(Debug, Default, Clone)]
struct FakeClient {
    calls: Arc<Mutex<Vec<(String, Value)>>>,
    responses: Arc<Mutex<HashMap<String, Value>>>,
    errors: Arc<Mutex<HashMap<String, String>>>,
}

impl FakeClient {
    fn with_response(method: &str, response: Value) -> Self {
        let mut responses = HashMap::new();
        responses.insert(method.to_string(), response);
        Self::with_responses(responses)
    }

    fn with_responses(responses: HashMap<String, Value>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(responses)),
            errors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn with_error(method: &str, error: &str) -> Self {
        let mut errors = HashMap::new();
        errors.insert(method.to_string(), error.to_string());
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(HashMap::new())),
            errors: Arc::new(Mutex::new(errors)),
        }
    }
}

impl DaemonClient for FakeClient {
    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        self.calls
            .lock()
            .unwrap()
            .push((method.to_string(), params));
        if let Some(err) = self.errors.lock().unwrap().get(method) {
            return Err(err.clone());
        }
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
    move |_work_dir| Ok((PathBuf::from(FAKE_PROJECT_ROOT), Box::new(client.clone())))
}

fn tools_call_request(id: u64, name: &str, arguments: Value) -> McpRequest {
    McpRequest {
        jsonrpc: "2.0".into(),
        id: Some(Value::from(id)),
        method: "tools/call".into(),
        params: serde_json::json!({
            "name": name,
            "arguments": arguments,
        }),
    }
}

fn unwrap_text_payload(resp: &ccbr_mcp_server::McpResponse) -> Value {
    let result = resp.result.as_ref().expect("expected a JSON-RPC result");
    let text = result["content"][0]["text"]
        .as_str()
        .expect("expected text content");
    serde_json::from_str(text).expect("expected JSON text payload")
}

fn accepted_submit_response(job_id: &str) -> Value {
    serde_json::json!({
        JOB_ID: job_id,
        "agent_name": "claude",
        "target_kind": "agent",
        "target_name": "claude",
        STATUS: "accepted",
    })
}

#[test]
fn test_tools_list_returns_three_definitions() {
    let tools = ccbr_mcp_server::tool_definitions();
    assert_eq!(tools.len(), 3);

    let by_name: HashMap<&str, &Value> = tools
        .iter()
        .map(|t| (t["name"].as_str().unwrap(), t))
        .collect();

    let ask = by_name["ccbr_ask_agent"];
    assert_eq!(
        ask["inputSchema"]["required"],
        serde_json::json!(["agent_name", "message"])
    );
    assert!(ask["inputSchema"]["properties"]["agent_name"].is_object());
    assert!(ask["inputSchema"]["properties"]["message"].is_object());

    let pend = by_name["ccbr_pend_agent"];
    assert_eq!(
        pend["inputSchema"]["required"],
        serde_json::json!(["target"])
    );

    let ping = by_name["ccbr_ping_agent"];
    assert_eq!(ping["inputSchema"]["required"], serde_json::json!([]));
}

#[test]
fn test_ask_agent_dispatches_submit() {
    let mut responses = HashMap::new();
    responses.insert("submit".into(), accepted_submit_response("job_123"));
    let client = FakeClient::with_responses(responses);

    let req = tools_call_request(
        1,
        "ccbr_ask_agent",
        serde_json::json!({
            "agent_name": "claude",
            "message": "hello",
            "task_id": "task-1",
        }),
    );

    let outcome =
        ccbr_mcp_server::handle_request_with_factory(req, "droid", fake_factory(client.clone()));
    let HandleOutcome::Respond(resp) = outcome else {
        panic!("expected Respond outcome");
    };
    assert!(resp.error.is_none(), "expected no JSON-RPC error");

    let payload = unwrap_text_payload(&resp);
    assert_eq!(payload[JOB_ID], "job_123");
    assert_eq!(payload[REPLY_MODE], "async");
    assert!(!payload[TERMINAL].as_bool().unwrap());

    let layout = ccbr_storage::paths::PathLayout::new(FAKE_PROJECT_ROOT);
    let expected_project_id = layout.project_id();
    assert_eq!(payload["project_id"], expected_project_id);

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
    responses.insert("submit".into(), accepted_submit_response("job_123"));
    responses.insert(
        "watch".into(),
        serde_json::json!({
            STATUS: "completed",
            "reply": "done",
        }),
    );
    let client = FakeClient::with_responses(responses);

    let req = tools_call_request(
        2,
        "ccbr_ask_agent",
        serde_json::json!({
            "agent_name": "claude",
            "message": "hello",
            "wait": true,
            "timeout_s": 30.0,
        }),
    );

    let outcome =
        ccbr_mcp_server::handle_request_with_factory(req, "droid", fake_factory(client.clone()));
    let HandleOutcome::Respond(resp) = outcome else {
        panic!("expected Respond outcome");
    };
    assert!(resp.error.is_none());

    let payload = unwrap_text_payload(&resp);
    assert!(payload[TERMINAL].as_bool().unwrap());
    assert_eq!(payload[STATUS], "completed");
    assert_eq!(payload["reply"], "done");

    let calls = client.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "submit");
    assert_eq!(calls[1].0, "watch");
    assert_eq!(calls[1].1["target"], "job_123");
    assert_eq!(calls[1].1["timeout_s"], 30.0);
}

#[test]
fn test_ask_agent_daemon_error_propagates_to_tool_error() {
    let client = FakeClient::with_error("submit", "daemon down");

    let req = tools_call_request(
        7,
        "ccbr_ask_agent",
        serde_json::json!({
            "agent_name": "claude",
            "message": "hello",
        }),
    );

    let outcome =
        ccbr_mcp_server::handle_request_with_factory(req, "droid", fake_factory(client.clone()));
    let HandleOutcome::Respond(resp) = outcome else {
        panic!("expected Respond outcome");
    };
    assert!(resp.error.is_none(), "expected no JSON-RPC error");

    let result = resp.result.as_ref().expect("expected a JSON-RPC result");
    assert!(result["isError"].as_bool().unwrap());
    let message = result["content"][0]["text"]
        .as_str()
        .expect("expected error text");
    assert!(message.contains("daemon down"));

    let calls = client.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "submit");
}

#[test]
fn test_pend_agent_dispatches_get_by_job_id() {
    let client = FakeClient::with_response(
        "get",
        serde_json::json!({
            JOB_ID: "job_123",
            "agent_name": "claude",
            STATUS: "completed",
            "reply": "done",
            TERMINAL: true,
        }),
    );

    let req = tools_call_request(
        3,
        "ccbr_pend_agent",
        serde_json::json!({ "target": "job_123" }),
    );

    let outcome =
        ccbr_mcp_server::handle_request_with_factory(req, "droid", fake_factory(client.clone()));
    let HandleOutcome::Respond(resp) = outcome else {
        panic!("expected Respond outcome");
    };
    assert!(resp.error.is_none());

    let payload = unwrap_text_payload(&resp);
    assert_eq!(payload[JOB_ID], "job_123");
    assert_eq!(payload[STATUS], "completed");
    assert!(payload[TERMINAL].as_bool().unwrap());

    let calls = client.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "get");
    assert_eq!(calls[0].1[JOB_ID], "job_123");
}

#[test]
fn test_pend_agent_dispatches_get_by_agent_name() {
    let client = FakeClient::with_response(
        "get",
        serde_json::json!({
            "agent_name": "claude",
            JOB_ID: "job_456",
            STATUS: "running",
            TERMINAL: false,
        }),
    );

    let req = tools_call_request(
        4,
        "ccbr_pend_agent",
        serde_json::json!({ "target": "claude" }),
    );

    let outcome =
        ccbr_mcp_server::handle_request_with_factory(req, "droid", fake_factory(client.clone()));
    let HandleOutcome::Respond(resp) = outcome else {
        panic!("expected Respond outcome");
    };
    assert!(resp.error.is_none());

    let payload = unwrap_text_payload(&resp);
    assert_eq!(payload["agent_name"], "claude");

    let calls = client.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "get");
    assert_eq!(calls[0].1["agent_name"], "claude");
}

#[test]
fn test_ping_agent_defaults_to_ccbrd() {
    let client = FakeClient::with_response(
        "ping",
        serde_json::json!({ "pong": true, "target": "ccbrd" }),
    );

    let req = tools_call_request(5, "ccbr_ping_agent", serde_json::json!({}));

    let outcome =
        ccbr_mcp_server::handle_request_with_factory(req, "droid", fake_factory(client.clone()));
    let HandleOutcome::Respond(resp) = outcome else {
        panic!("expected Respond outcome");
    };
    assert!(resp.error.is_none());

    let payload = unwrap_text_payload(&resp);
    assert!(payload["pong"].as_bool().unwrap());

    let calls = client.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "ping");
    assert_eq!(calls[0].1["target"], "ccbrd");
}

#[test]
fn test_unknown_tool_returns_error() {
    let client = FakeClient::default();

    let req = tools_call_request(6, "ccbr_no_such_tool", serde_json::json!({}));

    let outcome =
        ccbr_mcp_server::handle_request_with_factory(req, "droid", fake_factory(client.clone()));
    let HandleOutcome::Respond(resp) = outcome else {
        panic!("expected Respond outcome");
    };

    // The server returns tool errors as a normal JSON-RPC result wrapping an
    // `isError` payload, so the response should not have a JSON-RPC error.
    assert!(resp.error.is_none());

    let result = resp.result.as_ref().expect("expected a JSON-RPC result");
    assert!(result["isError"].as_bool().unwrap());
    let message = result["content"][0]["text"]
        .as_str()
        .expect("expected error text");
    assert!(message.contains("unknown tool"));
}
