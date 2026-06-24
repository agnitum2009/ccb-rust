use std::path::Path;

use serde_json::Value;

/// The result of observing a native CLI provider's output.
#[derive(Debug, Clone, Default)]
pub struct NativeCliObservation {
    pub text: String,
    pub finished: bool,
    pub finish_reason: String,
    pub turn_ref: Option<String>,
    pub completed_at: Option<Value>,
    pub error: String,
    pub intermediate: bool,
}

/// Trait for observing provider output files.
pub trait NativeCliObserver: Send + Sync {
    fn observe(&self, path: &Path) -> NativeCliObservation;
}

impl<F> NativeCliObserver for F
where
    F: Fn(&Path) -> NativeCliObservation + Send + Sync,
{
    fn observe(&self, path: &Path) -> NativeCliObservation {
        (self)(path)
    }
}

/// Observe a plain stdout output file.
pub fn observe_stdout_output(path: &Path) -> NativeCliObservation {
    if path.as_os_str().is_empty() || !path.is_file() {
        return NativeCliObservation::default();
    }
    match std::fs::read_to_string(path) {
        Ok(text) => NativeCliObservation {
            text,
            ..Default::default()
        },
        Err(exc) => NativeCliObservation {
            error: format!("read_stdout_failed:{}", exc),
            ..Default::default()
        },
    }
}

/// Observe a JSONL event stream output file.
pub fn observe_jsonl_output(path: &Path) -> NativeCliObservation {
    if path.as_os_str().is_empty() || !path.is_file() {
        return NativeCliObservation::default();
    }
    let lines = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(exc) => {
            return NativeCliObservation {
                error: format!("read_stdout_failed:{}", exc),
                ..Default::default()
            }
        }
    };

    let mut chunks: Vec<String> = Vec::new();
    let mut finished = false;
    let mut finish_reason = String::new();
    let mut turn_ref: Option<String> = None;
    let mut completed_at: Option<Value> = None;
    let mut error = String::new();
    let mut intermediate = false;

    for line in lines.lines() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        let event: serde_json::Map<String, Value> = match serde_json::from_str::<Value>(stripped) {
            Ok(Value::Object(event)) => event,
            _ => continue,
        };

        if is_error_event(&event) {
            error = event_text(&event)
                .or_else(|| event_reason(&event))
                .unwrap_or_else(|| "native_cli_error".to_string());
            continue;
        }
        if is_tool_event(&event) {
            intermediate = true;
            if let Some(reason) = event_reason(&event) {
                finish_reason = reason;
            }
            continue;
        }

        if let Some(text) = assistant_text(&event) {
            chunks.push(text);
            turn_ref = turn_ref.or_else(|| event_ref(&event));
            completed_at = completed_at.clone().or_else(|| event_time(&event));
        }
        if is_final_event(&event) {
            finished = true;
            finish_reason = event_reason(&event)
                .or_else(|| {
                    if finish_reason.is_empty() {
                        Some("completed".to_string())
                    } else {
                        Some(finish_reason.clone())
                    }
                })
                .unwrap_or_default();
            turn_ref = turn_ref.or_else(|| event_ref(&event));
            completed_at = completed_at.clone().or_else(|| event_time(&event));
        }
    }

    NativeCliObservation {
        text: chunks.join(""),
        finished,
        finish_reason,
        turn_ref,
        completed_at,
        error,
        intermediate,
    }
}

fn assistant_text(event: &serde_json::Map<String, Value>) -> Option<String> {
    if is_user_event(event) {
        return None;
    }
    if !(is_assistant_event(event) || is_final_event(event)) {
        return None;
    }
    event_text(event)
}

fn is_user_event(event: &serde_json::Map<String, Value>) -> bool {
    nested_text_value(event, &["role", "sender", "author"])
        .trim()
        .to_lowercase()
        == "user"
}

fn is_assistant_event(event: &serde_json::Map<String, Value>) -> bool {
    let role = nested_text_value(event, &["role", "sender", "author"])
        .trim()
        .to_lowercase();
    if ["assistant", "agent", "model"].contains(&role.as_str()) {
        return true;
    }
    let event_type = event_type(event);
    [
        "assistant",
        "agent_message",
        "message_delta",
        "content_delta",
        "text",
    ]
    .iter()
    .any(|token| event_type.contains(token))
}

fn is_final_event(event: &serde_json::Map<String, Value>) -> bool {
    if is_tool_event(event) {
        return false;
    }
    let haystack = [
        event_type(event),
        event_reason(event)
            .unwrap_or_default()
            .trim()
            .to_lowercase()
            .replace('-', "_"),
        nested_text_value(event, &["status", "state"])
            .trim()
            .to_lowercase()
            .replace('-', "_"),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    if haystack.is_empty() {
        return false;
    }
    [
        "final",
        "result",
        "completion",
        "completed",
        "done",
        "finished",
        "turn_end",
        "end_turn",
    ]
    .iter()
    .any(|token| haystack.contains(token))
}

fn is_tool_event(event: &serde_json::Map<String, Value>) -> bool {
    let haystack = [
        event_type(event),
        event_reason(event)
            .unwrap_or_default()
            .trim()
            .to_lowercase()
            .replace('-', "_"),
        nested_text_value(event, &["role", "status", "state", "name"])
            .trim()
            .to_lowercase()
            .replace('-', "_"),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    haystack.contains("tool")
        || haystack.contains("permission")
        || haystack.contains("function_call")
}

fn is_error_event(event: &serde_json::Map<String, Value>) -> bool {
    let haystack = [
        event_type(event),
        event_reason(event)
            .unwrap_or_default()
            .trim()
            .to_lowercase()
            .replace('-', "_"),
        nested_text_value(event, &["status", "state"])
            .trim()
            .to_lowercase()
            .replace('-', "_"),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    [
        "error",
        "failed",
        "failure",
        "permission_denied",
        "unauthorized",
        "auth_failed",
    ]
    .iter()
    .any(|token| haystack.contains(token))
}

fn event_type(event: &serde_json::Map<String, Value>) -> String {
    nested_text_value(event, &["type", "event", "kind", "name"])
        .trim()
        .to_lowercase()
        .replace('-', "_")
}

fn event_text(event: &serde_json::Map<String, Value>) -> Option<String> {
    for key in [
        "merged_text",
        "final_answer",
        "answer",
        "reply",
        "text",
        "output",
        "response",
    ] {
        if let Some(value) = event.get(key) {
            if let Some(text) = extract_text(value) {
                return Some(text);
            }
        }
    }
    if let Some(value) = event.get("content") {
        if let Some(text) = extract_text(value) {
            return Some(text);
        }
    }
    for key in ["payload", "message", "delta", "part", "result", "data"] {
        if let Some(value) = event.get(key) {
            if let Some(text) = extract_text(value) {
                return Some(text);
            }
        }
    }
    None
}

fn extract_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Object(_) | Value::Array(_) => event_text_nested(value),
        _ => None,
    }
}

fn event_text_nested(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Array(arr) => {
            let text: String = arr.iter().filter_map(event_text_nested).collect();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        Value::Object(obj) => {
            for key in [
                "merged_text",
                "final_answer",
                "answer",
                "reply",
                "text",
                "output",
                "response",
                "content",
                "payload",
                "message",
                "delta",
                "part",
                "result",
                "data",
            ] {
                if let Some(v) = obj.get(key) {
                    if let Some(text) = extract_text(v) {
                        return Some(text);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn event_reason(event: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["reason", "finish_reason", "stop_reason", "status", "state"] {
        if let Some(Value::String(s)) = event.get(key) {
            if !s.is_empty() {
                return Some(s.trim().to_string());
            }
        }
    }
    for key in ["payload", "properties", "part", "message", "result", "data"] {
        if let Some(Value::Object(nested)) = event.get(key) {
            if let Some(reason) = event_reason(nested) {
                return Some(reason);
            }
        }
    }
    None
}

fn event_ref(event: &serde_json::Map<String, Value>) -> Option<String> {
    for key in [
        "id",
        "message_id",
        "messageID",
        "session_id",
        "sessionID",
        "turn_id",
        "request_id",
    ] {
        if let Some(Value::String(s)) = event.get(key) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    for key in ["payload", "message", "part", "result", "data"] {
        if let Some(Value::Object(nested)) = event.get(key) {
            if let Some(ref_value) = event_ref(nested) {
                return Some(ref_value);
            }
        }
    }
    None
}

fn event_time(event: &serde_json::Map<String, Value>) -> Option<Value> {
    for key in [
        "completed_at",
        "time",
        "timestamp",
        "created_at",
        "updated_at",
    ] {
        if let Some(value) = event.get(key) {
            if !value.is_null() {
                return Some(value.clone());
            }
        }
    }
    for key in ["payload", "message", "part", "result", "data"] {
        if let Some(Value::Object(nested)) = event.get(key) {
            if let Some(time) = event_time(nested) {
                return Some(time);
            }
        }
    }
    None
}

fn nested_text_value(event: &serde_json::Map<String, Value>, keys: &[&str]) -> String {
    for key in keys {
        if let Some(Value::String(s)) = event.get(*key) {
            if !s.is_empty() {
                return s.clone();
            }
        }
    }
    for key in ["payload", "message", "part", "result", "data"] {
        if let Some(value) = event.get(key) {
            if let Some(nested) = nested_text_value_nested(value, keys) {
                return nested;
            }
        }
    }
    String::new()
}

fn nested_text_value_nested(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Array(arr) => {
            for item in arr {
                if let Some(text) = nested_text_value_nested(item, keys) {
                    return Some(text);
                }
            }
            None
        }
        Value::Object(obj) => {
            for key in keys {
                if let Some(Value::String(s)) = obj.get(*key) {
                    if !s.is_empty() {
                        return Some(s.clone());
                    }
                }
            }
            for key in ["payload", "message", "part", "result", "data"] {
                if let Some(value) = obj.get(key) {
                    if let Some(text) = nested_text_value_nested(value, keys) {
                        return Some(text);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observe_stdout_missing_file() {
        let obs = observe_stdout_output(Path::new("/nonexistent/stdout.out"));
        assert!(obs.text.is_empty());
        assert!(obs.error.is_empty());
    }

    #[test]
    fn test_observe_stdout_text() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("out.txt");
        std::fs::write(&path, "hello world").unwrap();
        let obs = observe_stdout_output(&path);
        assert_eq!(obs.text, "hello world");
    }

    #[test]
    fn test_observe_jsonl_text_and_final() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("out.jsonl");
        std::fs::write(
            &path,
            r#"{"type":"assistant","text":"hello "}
{"type":"final","text":"world","finish_reason":"stop"}
"#,
        )
        .unwrap();
        let obs = observe_jsonl_output(&path);
        assert_eq!(obs.text, "hello world");
        assert!(obs.finished);
        assert_eq!(obs.finish_reason, "stop");
    }

    #[test]
    fn test_observe_jsonl_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("out.jsonl");
        std::fs::write(&path, r#"{"type":"error","message":"boom"}"#).unwrap();
        let obs = observe_jsonl_output(&path);
        assert_eq!(obs.error, "boom");
    }

    #[test]
    fn test_observe_jsonl_tool_intermediate() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("out.jsonl");
        std::fs::write(&path, r#"{"type":"tool_call","reason":"thinking"}"#).unwrap();
        let obs = observe_jsonl_output(&path);
        assert!(obs.intermediate);
        assert_eq!(obs.finish_reason, "thinking");
        assert!(!obs.finished);
    }
}
