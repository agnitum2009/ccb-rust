//! `ccbr-provider-activity-hook` — writes CCB provider activity status artifacts.
//!
//! 1:1 port of `bin/ccbr-provider-activity-hook` (Python). Invoked as a standalone
//! binary with `--provider/--project-id [--agent-name --runtime-dir --workspace
//! --state]` and a JSON event payload on stdin. Delegates artifact writing to
//! `ccbr_provider_hooks::write_activity`.

use std::collections::HashMap;
use std::io::Read;

use camino::Utf8Path;
use serde_json::Value;

fn main() -> std::process::ExitCode {
    let args = parse_args();
    let payload = read_stdin_payload();
    let provider = args.provider.trim().to_lowercase();
    let state = activity_state(&provider, &payload, &args.state);
    let runtime_dir = if !args.runtime_dir.is_empty() {
        args.runtime_dir.clone()
    } else {
        std::env::var("CCB_CALLER_RUNTIME_DIR").unwrap_or_default()
    };
    let runtime_dir = runtime_dir.trim().to_string();
    let agent_name = if !args.agent_name.is_empty() {
        args.agent_name.clone()
    } else {
        std::env::var("CCB_CALLER_ACTOR").unwrap_or_default()
    };
    let agent_name = agent_name.trim().to_string();
    if state.is_empty() || runtime_dir.is_empty() || agent_name.is_empty() {
        return std::process::ExitCode::SUCCESS;
    }
    let ccbr_session_id = std::env::var("CCB_SESSION_ID").ok();
    let pane_id = std::env::var("TMUX_PANE").ok();
    let event_name = event_name(&payload);
    let provider_session_id = first_text(&payload, &["session_id", "sessionId", "session.id"]);
    let provider_turn_id = first_text(
        &payload,
        &["turn_id", "turnId", "turn.id", "request_id", "requestId"],
    );
    let model = first_text(&payload, &["model", "model_id", "modelId", "request.model"]);
    let diagnostics = diagnostics(&payload);
    let _ = ccbr_provider_hooks::write_activity(
        &provider,
        &args.project_id,
        &agent_name,
        Utf8Path::new(&runtime_dir),
        &state,
        &format!("{provider}_hook"),
        if event_name.is_empty() {
            None
        } else {
            Some(&event_name)
        },
        ccbr_session_id.as_deref(),
        pane_id.as_deref(),
        if args.workspace.is_empty() {
            None
        } else {
            Some(&args.workspace)
        },
        if provider_session_id.is_empty() {
            None
        } else {
            Some(&provider_session_id)
        },
        if provider_turn_id.is_empty() {
            None
        } else {
            Some(&provider_turn_id)
        },
        if model.is_empty() { None } else { Some(&model) },
        Some(&diagnostics),
        None,
    );
    std::process::ExitCode::SUCCESS
}

struct Args {
    provider: String,
    project_id: String,
    agent_name: String,
    runtime_dir: String,
    workspace: String,
    state: String,
}

fn parse_args() -> Args {
    let mut provider = String::new();
    let mut project_id = String::new();
    let mut agent_name = String::new();
    let mut runtime_dir = String::new();
    let mut workspace = String::new();
    let mut state = String::new();
    let mut argv = std::env::args().skip(1);
    while let Some(arg) = argv.next() {
        let value = argv.next().unwrap_or_default();
        match arg.as_str() {
            "--provider" => provider = value,
            "--project-id" => project_id = value,
            "--agent-name" => agent_name = value,
            "--runtime-dir" => runtime_dir = value,
            "--workspace" => workspace = value,
            "--state" => state = value,
            _ => {}
        }
    }
    Args {
        provider,
        project_id,
        agent_name,
        runtime_dir,
        workspace,
        state,
    }
}

fn read_stdin_payload() -> Value {
    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() {
        return Value::Object(serde_json::Map::new());
    }
    if raw.trim().is_empty() {
        return Value::Object(serde_json::Map::new());
    }
    serde_json::from_str(&raw).unwrap_or_else(|_| Value::Object(serde_json::Map::new()))
}

fn lookup(payload: &Value, path: &str) -> Value {
    let mut current = payload;
    for part in path.split('.') {
        match current.get(part) {
            Some(v) => current = v,
            None => return Value::Null,
        }
    }
    current.clone()
}

fn first_text(payload: &Value, paths: &[&str]) -> String {
    for path in paths {
        let value = lookup(payload, path);
        if value.is_null() {
            continue;
        }
        let text = match &value {
            Value::String(s) => s.clone(),
            v => v.to_string().trim_matches('"').to_string(),
        };
        let text = text.trim().to_string();
        if !text.is_empty() {
            return text;
        }
    }
    String::new()
}

fn event_name(payload: &Value) -> String {
    let name = first_text(
        payload,
        &["hook_event_name", "event_name", "event", "type", "name"],
    );
    if name.is_empty() {
        "unknown".into()
    } else {
        name
    }
}

fn has_error(payload: &Value) -> bool {
    for path in &["error", "error.message", "error_type", "errorType"] {
        if !first_text(payload, &[*path]).is_empty() {
            return true;
        }
    }
    let status = first_text(payload, &["status", "state"]).to_lowercase();
    matches!(status.as_str(), "failed" | "failure" | "error" | "errored")
}

fn background_tasks_running(payload: &Value) -> bool {
    let value = lookup(payload, "background_tasks");
    if let Some(arr) = value.as_array() {
        return !arr.is_empty();
    }
    if let Some(obj) = value.as_object() {
        if let Some(running) = obj.get("running") {
            if let Some(arr) = running.as_array() {
                return !arr.is_empty();
            }
            if let Some(b) = running.as_bool() {
                return b;
            }
        }
    }
    payload
        .get("background_tasks_running")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn claude_notification_waiting(payload: &Value) -> bool {
    let mut text = String::new();
    for path in &[
        "message",
        "notification.message",
        "title",
        "notification.title",
        "reason",
    ] {
        let part = first_text(payload, &[*path]).to_lowercase();
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&part);
    }
    for marker in &["permission", "approve", "input", "waiting", "blocked"] {
        if text.contains(marker) {
            return true;
        }
    }
    false
}

fn activity_state(provider: &str, payload: &Value, explicit_state: &str) -> String {
    if !explicit_state.is_empty() {
        return explicit_state.to_string();
    }
    if has_error(payload) {
        return "failed".into();
    }
    let event = event_name(payload);
    let normalized: String = event
        .to_lowercase()
        .chars()
        .filter(|c| *c != '-' && *c != '_')
        .collect();
    match normalized.as_str() {
        "userpromptsubmit" | "posttooluse" => "active".into(),
        "pretooluse" => "tool".into(),
        "permissionrequest" | "notification" => {
            if provider == "claude"
                && normalized == "notification"
                && !claude_notification_waiting(payload)
            {
                String::new()
            } else {
                "waiting".into()
            }
        }
        "sessionstart" | "stop" => {
            if normalized == "stop" && background_tasks_running(payload) {
                "active".into()
            } else {
                "idle".into()
            }
        }
        _ => String::new(),
    }
}

fn diagnostics(payload: &Value) -> HashMap<String, Value> {
    let mut result: HashMap<String, Value> = HashMap::new();
    let tool_name = first_text(payload, &["tool_name", "tool.name", "toolName"]);
    if !tool_name.is_empty() {
        result.insert("tool_name".into(), Value::String(tool_name));
    }
    let error_type = first_text(payload, &["error.type", "error_type", "errorType"]);
    if !error_type.is_empty() {
        result.insert("error_type".into(), Value::String(error_type));
        result.insert("reason".into(), Value::String("api_error".into()));
    }
    let error_code = first_text(payload, &["error.code", "error_code", "errorCode"]);
    if !error_code.is_empty() {
        result.insert("error_code".into(), Value::String(error_code));
    }
    let error_message = first_text(payload, &["error.message", "error_message", "errorMessage"]);
    if !error_message.is_empty() {
        let preview: String = error_message.chars().take(300).collect();
        result.insert("error_message_preview".into(), Value::String(preview));
    }
    result
}
