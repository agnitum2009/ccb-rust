use serde_json::Value;

use crate::app::CcbdApp;
use crate::handlers::HandlerRegistry;
use crate::models::api_models::rpc::{RpcRequest, RpcResponse};

pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;
pub const REQUEST_READ_TIMEOUT_MS: u64 = 500;

pub const MUTATING_OPS: &[&str] = &[
    "submit",
    "cancel",
    "attach",
    "start",
    "restore",
    "ack",
    "resubmit",
    "retry",
    "comms_recover",
    "project_restart_agent",
    "project_restart_panes",
    "project_clear_context",
    "stop-all",
];

pub fn is_mutating_op(op: &str) -> bool {
    MUTATING_OPS.contains(&op)
}

/// Handle a single RPC request string and return the JSON response string.
pub fn handle_request(app: &mut CcbdApp, handlers: &HandlerRegistry, raw: &str) -> String {
    let uses_cli = RpcRequest::uses_cli_shape(raw);

    let request = match RpcRequest::from_json(raw) {
        Ok(r) => r,
        Err(e) => return RpcResponse::failure(e).to_json(),
    };

    let response = match handlers.dispatch(&request.op, app, &request.request) {
        Ok(payload) => {
            if uses_cli {
                RpcResponse::success(payload)
            } else {
                RpcResponse::python_success(payload)
            }
        }
        Err(e) => RpcResponse::failure(e),
    };

    response.to_json()
}

/// Parse a single newline-delimited JSON object from a byte buffer.
pub fn parse_request_line(buf: &[u8]) -> Option<(Value, usize)> {
    if let Some(pos) = buf.iter().position(|&b| b == b'\n') {
        let line = &buf[..pos];
        if line.len() > MAX_REQUEST_BYTES {
            return None;
        }
        serde_json::from_slice(line).ok().map(|v| (v, pos + 1))
    } else {
        None
    }
}
