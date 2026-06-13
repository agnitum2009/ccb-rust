use std::collections::HashMap;

use ccb_completion::models::{
    CompletionConfidence, CompletionCursor, CompletionItem, CompletionItemKind,
    CompletionSourceKind, CompletionStatus, JobRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::models::ProviderSubmission;

pub const NO_WRAP_PROVIDER_OPTION: &str = "no_wrap";

/// Build a completion item for a submission.
pub fn build_item(
    submission: &ProviderSubmission,
    kind: CompletionItemKind,
    timestamp: impl Into<String>,
    seq: u64,
    payload: HashMap<String, Value>,
) -> CompletionItem {
    let cursor = CompletionCursor {
        source_kind: submission.source_kind,
        event_seq: Some(seq),
        updated_at: Some(timestamp.into()),
        ..Default::default()
    };
    CompletionItem {
        kind,
        timestamp: cursor.updated_at.clone().unwrap_or_default(),
        cursor,
        provider: submission.provider.clone(),
        agent_name: submission.agent_name.clone(),
        req_id: submission.job_id.clone(),
        payload: payload.into_iter().collect(),
    }
}

/// Extract the request anchor from runtime state.
pub fn request_anchor_from_runtime_state(
    runtime_state: &HashMap<String, Value>,
    fallback: &str,
) -> String {
    runtime_state
        .get("request_anchor")
        .or_else(|| runtime_state.get("req_id"))
        .and_then(|v| v.as_str())
        .unwrap_or(fallback)
        .trim()
        .to_string()
}

/// Create a passive submission when no runtime target is needed.
pub fn passive_submission(
    job: &JobRecord,
    provider: impl Into<String>,
    now: impl Into<String>,
    source_kind: CompletionSourceKind,
    reason: impl Into<String>,
) -> ProviderSubmission {
    let now = now.into();
    let provider = provider.into();
    let reason = reason.into();
    let diagnostics = serde_json::json!({
        "provider": provider,
        "mode": "passive",
        "reason": reason,
    });
    let mut runtime_state = HashMap::new();
    runtime_state.insert("mode".to_string(), Value::String("passive".to_string()));
    runtime_state.insert("reason".to_string(), Value::String(reason.clone()));
    ProviderSubmission {
        job_id: job.job_id.clone(),
        agent_name: job.agent_name.clone(),
        provider,
        accepted_at: now.clone(),
        ready_at: now,
        source_kind,
        reply: String::new(),
        status: CompletionStatus::Incomplete,
        reason,
        confidence: CompletionConfidence::Observed,
        diagnostics: Some(diagnostics),
        runtime_state,
    }
}

/// Create an error submission when something goes wrong during start.
pub fn error_submission(
    job: &JobRecord,
    provider: impl Into<String>,
    now: impl Into<String>,
    source_kind: CompletionSourceKind,
    reason: impl Into<String>,
    error: impl Into<String>,
) -> ProviderSubmission {
    let now = now.into();
    let provider = provider.into();
    let reason = reason.into();
    let error = error.into();
    let diagnostics = serde_json::json!({
        "provider": provider,
        "mode": "error",
        "reason": reason,
        "error": error,
    });
    let mut runtime_state = HashMap::new();
    runtime_state.insert("mode".to_string(), Value::String("error".to_string()));
    runtime_state.insert("reason".to_string(), Value::String(reason));
    runtime_state.insert("error".to_string(), Value::String(error));
    runtime_state.insert("next_seq".to_string(), Value::Number(1.into()));
    ProviderSubmission {
        job_id: job.job_id.clone(),
        agent_name: job.agent_name.clone(),
        provider,
        accepted_at: now.clone(),
        ready_at: now,
        source_kind,
        reply: String::new(),
        status: CompletionStatus::Incomplete,
        reason: "in_progress".to_string(),
        confidence: CompletionConfidence::Observed,
        diagnostics: Some(diagnostics),
        runtime_state,
    }
}

/// Check whether the no_wrap provider option was requested.
pub fn no_wrap_requested(options: Option<&Value>) -> bool {
    options
        .and_then(|v| v.as_object())
        .and_then(|m| m.get(NO_WRAP_PROVIDER_OPTION))
        .is_some_and(|v| v.as_bool().unwrap_or(false))
}

/// Serialize a runtime state value, handling path/bytes/tuple markers.
pub fn serialize_runtime_state(value: &Value) -> Value {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),
        Value::Array(arr) => Value::Array(arr.iter().map(serialize_runtime_state).collect()),
        Value::Object(obj) => {
            let mut out = serde_json::Map::new();
            for (k, v) in obj {
                out.insert(k.clone(), serialize_runtime_state(v));
            }
            Value::Object(out)
        }
    }
}

/// Deserialize a runtime state value. Currently a mirror of serialize.
pub fn deserialize_runtime_state(value: &Value) -> Value {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),
        Value::Array(arr) => Value::Array(arr.iter().map(deserialize_runtime_state).collect()),
        Value::Object(obj) => {
            if let Some(marker) = obj.get("__ccb_type__").and_then(|v| v.as_str()) {
                match marker {
                    "path" => obj
                        .get("value")
                        .and_then(|v| v.as_str())
                        .map(|s| Value::String(shellexpand::tilde(s).to_string()))
                        .unwrap_or(Value::Null),
                    "bytes" => obj
                        .get("value")
                        .and_then(|v| v.as_str())
                        .and_then(base64::decode)
                        .map(|b| Value::String(String::from_utf8_lossy(&b).to_string()))
                        .unwrap_or(Value::String(String::new())),
                    "tuple" => obj
                        .get("items")
                        .cloned()
                        .unwrap_or(Value::Array(Vec::new())),
                    _ => {
                        let mut out = serde_json::Map::new();
                        for (k, v) in obj {
                            out.insert(k.clone(), deserialize_runtime_state(v));
                        }
                        Value::Object(out)
                    }
                }
            } else {
                let mut out = serde_json::Map::new();
                for (k, v) in obj {
                    out.insert(k.clone(), deserialize_runtime_state(v));
                }
                Value::Object(out)
            }
        }
    }
}

// Simple base64 shim to avoid an extra dependency for this migration phase.
mod base64 {
    pub fn decode(input: &str) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(input.len() / 4 * 3);
        let mut buf = 0u32;
        let mut bits = 0u32;
        for ch in input.chars() {
            let v = match ch {
                'A'..='Z' => ch as u32 - 'A' as u32,
                'a'..='z' => ch as u32 - 'a' as u32 + 26,
                '0'..='9' => ch as u32 - '0' as u32 + 52,
                '+' => 62,
                '/' => 63,
                '=' => break,
                _ => return None,
            };
            buf = (buf << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push(((buf >> bits) & 0xFF) as u8);
            }
        }
        Some(out)
    }
}

// Minimal shell expansion helper to avoid pulling in shellexpand.
mod shellexpand {
    use std::env;

    pub fn tilde(input: &str) -> String {
        if let Some(rest) = input.strip_prefix('~') {
            if let Ok(home) = env::var("HOME") {
                return home + rest;
            }
        }
        input.to_string()
    }
}

/// Wrapper used to safely round-trip runtime state through JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStateWrapper(pub HashMap<String, Value>);

impl From<HashMap<String, Value>> for RuntimeStateWrapper {
    fn from(value: HashMap<String, Value>) -> Self {
        Self(value)
    }
}

impl From<RuntimeStateWrapper> for HashMap<String, Value> {
    fn from(value: RuntimeStateWrapper) -> Self {
        value.0
    }
}
