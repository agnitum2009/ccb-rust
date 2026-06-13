use chrono::{DateTime, Utc};
use serde_json::Map;
use sha2::{Digest, Sha256};

use crate::error::{CompletionError, Result};

pub fn utc_now_iso() -> String {
    Utc::now()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        .replace("+00:00", "Z")
}

pub fn parse_timestamp(value: &str) -> Result<DateTime<Utc>> {
    let normalized = value.replace('Z', "+00:00");
    normalized
        .parse::<DateTime<Utc>>()
        .map_err(|e| CompletionError::Validation(format!("invalid timestamp {value:?}: {e}")))
}

pub fn seconds_between(start: &str, end: &str) -> Result<f64> {
    let start_dt = parse_timestamp(start)?;
    let end_dt = parse_timestamp(end)?;
    Ok((end_dt - start_dt).num_milliseconds() as f64 / 1000.0)
}

pub fn fingerprint_text(parts: &[&str]) -> String {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update(part.as_bytes());
        digest.update(b"\x1f");
    }
    hex::encode(digest.finalize())
}

pub fn first_non_empty(payload: &Map<String, serde_json::Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(serde_json::Value::String(value)) = payload.get(*key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

pub fn empty_json_object() -> serde_json::Value {
    serde_json::Value::Object(Map::new())
}

pub fn runtime_mode_to_string(mode: &ccb_agents::models::RuntimeMode) -> String {
    match mode {
        ccb_agents::models::RuntimeMode::PaneBacked => "pane-backed".into(),
        ccb_agents::models::RuntimeMode::PtyBacked => "pty-backed".into(),
        ccb_agents::models::RuntimeMode::Headless => "headless".into(),
    }
}
