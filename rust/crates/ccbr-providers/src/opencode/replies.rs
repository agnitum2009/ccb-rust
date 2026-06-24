use std::collections::HashMap;

use regex::Regex;
use serde_json::Value;

type ReadPartsFn<'a> = &'a dyn Fn(&str) -> Vec<Value>;
type ExtractReqIdFn<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Extract text from a list of message parts.
pub fn extract_text(parts: &[Value], allow_reasoning_fallback: bool) -> String {
    let text = collect_text(parts, &["text"]);
    if !text.is_empty() {
        return text;
    }
    if allow_reasoning_fallback {
        collect_text(parts, &["reasoning"])
    } else {
        String::new()
    }
}

fn collect_text(parts: &[Value], types: &[&str]) -> String {
    let mut out = Vec::new();
    for part in parts {
        if let Some(obj) = part.as_object() {
            let part_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if !types.contains(&part_type) {
                continue;
            }
            if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    out.push(text);
                }
            }
        }
    }
    out.join("").trim().to_string()
}

/// Extract a request id from text using the OpenCode req id regex.
pub fn extract_req_id_from_text(text: &str, re: &Regex) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    re.captures(text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_lowercase())
}

/// Build a list of (question, answer) conversation pairs from messages.
pub fn conversations_from_messages(
    messages: &[Value],
    read_parts: &dyn Fn(&str) -> Vec<Value>,
    n: usize,
) -> Vec<(String, String)> {
    let mut conversations = Vec::new();
    let mut pending_question: Option<String> = None;
    for message in messages {
        let Some(obj) = message.as_object() else {
            continue;
        };
        let message_id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if message_id.is_empty() {
            continue;
        }
        let text = extract_text(&read_parts(message_id), true);
        let role = obj.get("role").and_then(|v| v.as_str()).unwrap_or("");
        match role {
            "user" => {
                pending_question = Some(text);
            }
            "assistant" if !text.is_empty() => {
                conversations.push((pending_question.take().unwrap_or_default(), text));
            }
            _ => {}
        }
    }
    if n == 0 {
        return conversations;
    }
    if conversations.len() > n {
        conversations.split_off(conversations.len() - n)
    } else {
        conversations
    }
}

/// Observe the latest assistant reply from messages.
pub fn observe_latest_assistant(
    messages: &[Value],
    read_parts: ReadPartsFn<'_>,
    extract_req_id: Option<ExtractReqIdFn<'_>>,
) -> Option<HashMap<String, Value>> {
    let assistants = assistant_messages(messages);
    let latest = assistants.last()?;
    Some(observed_assistant_reply(latest, read_parts, extract_req_id))
}

/// Return the latest assistant text, or None if not completed.
pub fn latest_message_from_messages(
    messages: &[Value],
    read_parts: ReadPartsFn<'_>,
) -> Option<String> {
    let observed = observe_latest_assistant(messages, read_parts, None)?;
    observed.get("completed")?;
    observed
        .get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Find a new assistant reply relative to a prior state dictionary.
pub fn find_new_assistant_reply_with_state(
    messages: &[Value],
    state: &HashMap<String, Value>,
    read_parts: ReadPartsFn<'_>,
    extract_req_id: Option<ExtractReqIdFn<'_>>,
) -> (Option<String>, Option<HashMap<String, Value>>) {
    let previous = assistant_state(state);
    let Some(observed) = observe_latest_assistant(messages, read_parts, extract_req_id) else {
        return (None, None);
    };
    let assistants = assistant_messages(messages);
    if !assistant_state_changed(&previous, &observed, assistants.len()) {
        return (None, None);
    }
    let reply_state = HashMap::from_iter([
        (
            "assistant_count".to_string(),
            Value::Number(assistants.len().into()),
        ),
        (
            "last_assistant_id".to_string(),
            observed.get("assistant_id").cloned().unwrap_or(Value::Null),
        ),
        (
            "last_assistant_parent_id".to_string(),
            observed.get("parent_id").cloned().unwrap_or(Value::Null),
        ),
        (
            "last_assistant_completed".to_string(),
            observed.get("completed").cloned().unwrap_or(Value::Null),
        ),
        (
            "last_assistant_req_id".to_string(),
            observed.get("req_id").cloned().unwrap_or(Value::Null),
        ),
        (
            "last_assistant_text_hash".to_string(),
            observed.get("text_hash").cloned().unwrap_or(Value::Null),
        ),
        (
            "last_assistant_aborted".to_string(),
            observed.get("aborted").cloned().unwrap_or(Value::Null),
        ),
    ]);
    let reply = observed
        .get("text")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    (reply, Some(reply_state))
}

fn assistant_messages(messages: &[Value]) -> Vec<&Value> {
    messages
        .iter()
        .filter(|m| {
            m.as_object()
                .is_some_and(|obj| obj.get("role").and_then(|v| v.as_str()) == Some("assistant"))
                && m.as_object()
                    .is_some_and(|obj| obj.get("id").and_then(|v| v.as_str()).is_some())
        })
        .collect()
}

fn assistant_state(state: &HashMap<String, Value>) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    out.insert(
        "assistant_count".to_string(),
        state
            .get("assistant_count")
            .and_then(|v| v.as_u64())
            .map(|n| Value::Number(n.into()))
            .unwrap_or(Value::Number(0.into())),
    );
    out.insert(
        "assistant_id".to_string(),
        state
            .get("last_assistant_id")
            .cloned()
            .unwrap_or(Value::Null),
    );
    out.insert(
        "parent_id".to_string(),
        state
            .get("last_assistant_parent_id")
            .cloned()
            .unwrap_or(Value::Null),
    );
    out.insert(
        "completed".to_string(),
        state
            .get("last_assistant_completed")
            .cloned()
            .unwrap_or(Value::Null),
    );
    out.insert(
        "req_id".to_string(),
        state
            .get("last_assistant_req_id")
            .cloned()
            .unwrap_or(Value::Null),
    );
    out.insert(
        "text_hash".to_string(),
        state
            .get("last_assistant_text_hash")
            .cloned()
            .unwrap_or(Value::Null),
    );
    out.insert(
        "aborted".to_string(),
        state
            .get("last_assistant_aborted")
            .and_then(|v| v.as_bool())
            .map(Value::Bool)
            .unwrap_or(Value::Bool(false)),
    );
    out
}

fn observed_assistant_reply(
    latest: &Value,
    read_parts: ReadPartsFn<'_>,
    extract_req_id: Option<ExtractReqIdFn<'_>>,
) -> HashMap<String, Value> {
    let obj = latest.as_object().unwrap();
    let assistant_id = obj
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let completed = completed_marker(latest);
    let text = assistant_text(&assistant_id, read_parts);
    let parent_id = parent_message_id(latest);
    let req_id = parent_id
        .as_deref()
        .and_then(|pid| extract_req_id.map(|f| f(&assistant_text(pid, &|_id| read_parts(pid)))))
        .flatten();
    let aborted = obj.get("error").map(is_aborted_error).unwrap_or(false);
    let text_hash_value = text_hash(&text).map(Value::String).unwrap_or(Value::Null);
    HashMap::from_iter([
        ("assistant_id".to_string(), Value::String(assistant_id)),
        (
            "parent_id".to_string(),
            parent_id.map(Value::String).unwrap_or(Value::Null),
        ),
        (
            "completed".to_string(),
            completed
                .map(|v| Value::Number(v.into()))
                .unwrap_or(Value::Null),
        ),
        ("text".to_string(), Value::String(text)),
        (
            "req_id".to_string(),
            req_id.map(Value::String).unwrap_or(Value::Null),
        ),
        ("text_hash".to_string(), text_hash_value),
        ("aborted".to_string(), Value::Bool(aborted)),
    ])
}

fn assistant_text(assistant_id: &str, read_parts: ReadPartsFn<'_>) -> String {
    extract_text(&read_parts(assistant_id), false)
}

fn parent_message_id(message: &Value) -> Option<String> {
    let obj = message.as_object()?;
    obj.get("parentID")
        .or_else(|| obj.get("parent_id"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn completed_marker(message: &Value) -> Option<i64> {
    message
        .as_object()
        .and_then(|obj| obj.get("time"))
        .and_then(|t| t.as_object())
        .and_then(|t| t.get("completed"))
        .and_then(|v| v.as_i64())
}

fn text_hash(text: &str) -> Option<String> {
    let normalized = text.trim();
    if normalized.is_empty() {
        return None;
    }
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    Some(format!("{:x}", hasher.finish()))
}

fn assistant_state_changed(
    previous: &HashMap<String, Value>,
    observed: &HashMap<String, Value>,
    assistant_count: usize,
) -> bool {
    let prev_count = previous
        .get("assistant_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    assistant_count > prev_count
        || previous.get("assistant_id") != observed.get("assistant_id")
        || previous.get("parent_id") != observed.get("parent_id")
        || previous.get("completed") != observed.get("completed")
        || previous.get("req_id") != observed.get("req_id")
        || previous.get("text_hash") != observed.get("text_hash")
        || previous.get("aborted") != observed.get("aborted")
}

/// Check whether an error object represents an aborted/cancelled error.
pub fn is_aborted_error(error_obj: &Value) -> bool {
    let Some(obj) = error_obj.as_object() else {
        return false;
    };
    if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
        let lower = name.to_lowercase();
        if lower.contains("aborted") || lower.contains("abort") {
            return true;
        }
    }
    if let Some(data) = obj.get("data").and_then(|v| v.as_object()) {
        if let Some(message) = data.get("message").and_then(|v| v.as_str()) {
            let lower = message.to_lowercase();
            if lower.contains("aborted") || lower.contains("cancel") {
                return true;
            }
        }
    }
    false
}
