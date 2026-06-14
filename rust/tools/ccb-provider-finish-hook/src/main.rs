//! `ccb-provider-finish-hook` — writes provider completion artifacts for
//! Claude/Gemini hook events.
//!
//! 1:1 port of `bin/ccb-provider-finish-hook` (Python). Invoked as a standalone
//! binary by provider hook systems with `--provider/--completion-dir/--agent-name
//! --workspace` and a JSON event payload on stdin. Delegates artifact writing to
//! the `ccb_provider_hooks` crate.

use std::collections::HashMap;
use std::io::Read;

use camino::Utf8Path;
use serde_json::Value;

fn main() -> std::process::ExitCode {
    let args = parse_args();
    let payload = read_stdin_payload();
    let completion_dir = expand_user(&args.completion_dir);
    let provider = args.provider.trim().to_lowercase();
    let code = if provider == "claude" {
        handle_claude(&payload, &completion_dir, &args.agent_name, &args.workspace)
    } else if provider == "gemini" {
        handle_gemini(&payload, &completion_dir, &args.agent_name, &args.workspace)
    } else {
        0
    };
    std::process::ExitCode::from(code as u8)
}

struct Args {
    provider: String,
    completion_dir: String,
    agent_name: String,
    workspace: String,
}

fn parse_args() -> Args {
    let mut provider = String::new();
    let mut completion_dir = String::new();
    let mut agent_name = String::new();
    let mut workspace = String::new();
    let mut argv = std::env::args().skip(1);
    while let Some(arg) = argv.next() {
        let value = argv.next().unwrap_or_default();
        match arg.as_str() {
            "--provider" => provider = value,
            "--completion-dir" => completion_dir = value,
            "--agent-name" => agent_name = value,
            "--workspace" => workspace = value,
            _ => {}
        }
    }
    Args {
        provider,
        completion_dir,
        agent_name,
        workspace,
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

fn lookup_path(payload: &Value, path: &str) -> Value {
    let mut current = payload;
    for part in path.split('.') {
        match current.get(part) {
            Some(v) => current = v,
            None => return Value::Null,
        }
    }
    current.clone()
}

fn first_value(payload: &Value, paths: &[&str]) -> Value {
    for path in paths {
        let value = lookup_path(payload, path);
        if value.is_null() {
            continue;
        }
        if let Some(s) = value.as_str() {
            if s.trim().is_empty() {
                continue;
            }
        }
        return value;
    }
    Value::Null
}

fn first_text(payload: &Value, paths: &[&str]) -> String {
    match first_value(payload, paths) {
        Value::Null => String::new(),
        Value::String(s) => s.trim().to_string(),
        v => v.to_string().trim_matches('"').trim().to_string(),
    }
}

fn normalize_status_token(value: &Value) -> Option<&'static str> {
    let raw = value.as_str().unwrap_or("");
    let token: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if token.is_empty() {
        return None;
    }
    match token.as_str() {
        "completed" | "complete" | "success" | "succeeded" | "ok" | "stop" | "stopped"
        | "finished" | "done" => Some("completed"),
        "failed" | "failure" | "error" | "errored" => Some("failed"),
        "cancelled" | "canceled" | "cancel" | "aborted" | "abort" | "interrupted" => {
            Some("cancelled")
        }
        "incomplete" | "timeout" | "timedout" | "max_tokens" | "maxtokens" | "length" => {
            Some("incomplete")
        }
        _ => None,
    }
}

fn gemini_text_failure_details(text: &str) -> Option<(&'static str, String)> {
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    let markers: &[(&str, &str)] = &[
        ("LoginRequired", "code assist login required"),
        ("LoginRequired", "login required"),
        ("NotLoggedIn", "not logged in"),
        ("AuthenticationFailed", "authentication failed"),
        ("PermissionDenied", "permission denied"),
        ("AccessDenied", "access denied"),
        ("Forbidden", "forbidden"),
        ("Unauthorized", "unauthorized"),
        ("InsufficientQuota", "insufficient quota"),
        ("QuotaExceeded", "quota exceeded"),
        ("PaymentRequired", "payment required"),
        ("InsufficientBalance", "insufficient balance"),
        ("CreditBalanceTooLow", "credit balance too low"),
    ];
    for (code, marker) in markers {
        if normalized.contains(marker) {
            return Some((code, text.trim().to_string()));
        }
    }
    None
}

fn empty_reply_diagnostics(reason: &str) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    let diagnosis = "Provider completion hook fired without assistant reply text; inspect the provider transcript, pane state, and authentication/API output.";
    map.insert("reason".into(), Value::String(reason.into()));
    map.insert("empty_reply".into(), Value::Bool(true));
    map.insert(
        "error_type".into(),
        Value::String("empty_provider_reply".into()),
    );
    map.insert("message".into(), Value::String(diagnosis.into()));
    map.insert("diagnosis".into(), Value::String(diagnosis.into()));
    map
}

/// Compute (status, diagnostics) for a Gemini event payload.
fn gemini_event_status_and_diagnostics(
    payload: &Value,
    reply: &str,
) -> (String, HashMap<String, Value>) {
    let mut diagnostics: HashMap<String, Value> = HashMap::new();
    let hook_event = first_text(payload, &["hook_event_name"]);
    diagnostics.insert(
        "hook_event_name".into(),
        Value::String(if hook_event.is_empty() {
            "AfterAgent".into()
        } else {
            hook_event
        }),
    );

    let finish_reason = first_text(
        payload,
        &[
            "finishReason",
            "finish_reason",
            "result.finishReason",
            "result.finish_reason",
        ],
    );
    if !finish_reason.is_empty() {
        diagnostics.insert("finish_reason".into(), Value::String(finish_reason.clone()));
    }

    let raw_error_value = first_value(
        payload,
        &[
            "error",
            "result.error",
            "response.error",
            "agent.error",
            "failure",
            "exception",
            "cause",
        ],
    );

    let mut error_code = first_text(
        payload,
        &[
            "error.code",
            "error.error_code",
            "error.errorCode",
            "result.error.code",
            "response.error.code",
            "agent.error.code",
            "failure.code",
            "exception.code",
            "cause.code",
            "error_code",
            "errorCode",
        ],
    );
    let error_type = first_text(
        payload,
        &[
            "error.type",
            "error.error_type",
            "error.errorType",
            "result.error.type",
            "response.error.type",
            "agent.error.type",
            "failure.type",
            "exception.type",
            "cause.type",
            "error_type",
            "errorType",
        ],
    );
    let mut error_message = first_text(
        payload,
        &[
            "error.message",
            "error.error_message",
            "error.errorMessage",
            "result.error.message",
            "response.error.message",
            "agent.error.message",
            "failure.message",
            "exception.message",
            "cause.message",
            "error_message",
            "errorMessage",
        ],
    );
    if error_message.is_empty() && !raw_error_value.is_null() && !raw_error_value.is_object() {
        error_message = raw_error_value
            .to_string()
            .trim_matches('"')
            .trim()
            .to_string();
    }

    let explicit_status = normalize_status_token(&first_value(
        payload,
        &[
            "status",
            "result.status",
            "response.status",
            "agent.status",
            "state",
            "result.state",
            "response.state",
        ],
    ));
    let finish_status = normalize_status_token(&Value::String(finish_reason.clone()));
    let text_failure = gemini_text_failure_details(reply);
    let has_explicit_error = !error_code.is_empty()
        || !error_type.is_empty()
        || !error_message.is_empty()
        || !raw_error_value.is_null();

    if let Some((code, msg)) = &text_failure {
        if error_code.is_empty() {
            error_code = code.to_string();
        }
        if error_message.is_empty() {
            error_message = msg.clone();
        }
    }

    let mut status = explicit_status
        .map(String::from)
        .or_else(|| {
            if has_explicit_error || text_failure.is_some() {
                Some("failed".into())
            } else {
                None
            }
        })
        .or_else(|| finish_status.map(String::from))
        .unwrap_or_else(|| "completed".into());

    if status == "completed" && (has_explicit_error || text_failure.is_some()) {
        status = "failed".into();
    }

    if status == "failed" {
        diagnostics.insert(
            "error_type".into(),
            Value::String(if error_type.is_empty() {
                "provider_api_error".into()
            } else {
                error_type.clone()
            }),
        );
        diagnostics.insert("reason".into(), Value::String("api_error".into()));
    } else if !error_type.is_empty() {
        diagnostics.insert("error_type".into(), Value::String(error_type.clone()));
    }
    if !error_code.is_empty() {
        diagnostics.insert("error_code".into(), Value::String(error_code));
    }
    if !error_message.is_empty() {
        diagnostics.insert("error_message".into(), Value::String(error_message.clone()));
        diagnostics.insert("text".into(), Value::String(error_message));
    }
    (status, diagnostics)
}

fn handle_claude(payload: &Value, completion_dir: &str, agent_name: &str, workspace: &str) -> i32 {
    let event_name = first_text(payload, &["hook_event_name"]);
    let event_name = if event_name.is_empty() {
        "Stop".to_string()
    } else {
        event_name
    };
    let transcript_path = first_text(payload, &["transcript_path"]);
    let reply = first_text(payload, &["last_assistant_message"]);
    let req_id = ccb_provider_hooks::current_turn_req_id_from_transcript(
        if transcript_path.is_empty() {
            None
        } else {
            Some(&transcript_path)
        },
        if reply.is_empty() { None } else { Some(&reply) },
    );
    let Some(req_id) = req_id else {
        return 0;
    };
    let mut status = if event_name == "Stop" {
        "completed"
    } else {
        "failed"
    }
    .to_string();
    let mut diagnostics: HashMap<String, Value> = HashMap::new();
    diagnostics.insert("hook_event_name".into(), Value::String(event_name.clone()));
    diagnostics.insert(
        "stop_hook_active".into(),
        Value::Bool(
            payload
                .get("stop_hook_active")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        ),
    );
    if status == "completed" && reply.trim().is_empty() {
        status = "incomplete".into();
        for (k, v) in empty_reply_diagnostics("hook_stop_empty_reply") {
            diagnostics.insert(k, v);
        }
    }
    let session_id = first_text(payload, &["session_id"]);
    let _ = ccb_provider_hooks::write_event(
        "claude",
        Utf8Path::new(&completion_dir),
        agent_name,
        workspace,
        &req_id,
        &status,
        &reply,
        if session_id.is_empty() {
            None
        } else {
            Some(&session_id)
        },
        Some(&event_name),
        if transcript_path.is_empty() {
            None
        } else {
            Some(&transcript_path)
        },
        Some(&diagnostics),
    );
    0
}

fn handle_gemini(payload: &Value, completion_dir: &str, agent_name: &str, workspace: &str) -> i32 {
    let prompt = first_text(payload, &["prompt", "request.prompt", "input.prompt"]);
    let req_id = ccb_provider_hooks::extract_req_id(&prompt);
    let Some(req_id) = req_id else {
        return 0;
    };
    let raw_reply = first_value(
        payload,
        &[
            "prompt_response",
            "response",
            "result.response",
            "reply",
            "message",
        ],
    );
    let reply = if raw_reply.is_null() {
        String::new()
    } else {
        raw_reply.to_string().trim_matches('"').trim().to_string()
    };
    let (mut status, mut diagnostics) = gemini_event_status_and_diagnostics(payload, &reply);
    if status == "completed" && reply.is_empty() {
        status = "incomplete".into();
        for (k, v) in empty_reply_diagnostics("hook_after_agent_incomplete") {
            diagnostics.insert(k, v);
        }
    }
    let session_id = first_text(payload, &["session_id", "sessionId", "session.id"]);
    let hook_event_name = first_text(payload, &["hook_event_name"]);
    let hook_event_name = if hook_event_name.is_empty() {
        "AfterAgent".to_string()
    } else {
        hook_event_name
    };
    let _ = ccb_provider_hooks::write_event(
        "gemini",
        Utf8Path::new(&completion_dir),
        agent_name,
        workspace,
        &req_id,
        &status,
        &reply,
        if session_id.is_empty() {
            None
        } else {
            Some(&session_id)
        },
        Some(&hook_event_name),
        None,
        Some(&diagnostics),
    );
    0
}

fn expand_user(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}{}", home, rest);
        }
    }
    path.to_string()
}
