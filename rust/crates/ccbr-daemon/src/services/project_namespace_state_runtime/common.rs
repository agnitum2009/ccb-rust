use crate::models::api_models::common::SCHEMA_VERSION;

pub const NAMESPACE_STATE_RECORD_TYPE: &str = "ccbrd_project_namespace_state";
pub const NAMESPACE_EVENT_RECORD_TYPE: &str = "ccbrd_project_namespace_event";

/// Convert a JSON value to a trimmed, non-empty string or `None`.
pub fn clean_text(value: Option<&serde_json::Value>) -> Option<String> {
    value.and_then(|v| match v {
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        }
        _ => None,
    })
}

pub fn require_schema_version(payload: &serde_json::Value) -> anyhow::Result<()> {
    if payload
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .map(|v| v == SCHEMA_VERSION as u64)
        != Some(true)
    {
        anyhow::bail!("schema_version must be {SCHEMA_VERSION}");
    }
    Ok(())
}

pub fn require_record_type(payload: &serde_json::Value, record_type: &str) -> anyhow::Result<()> {
    if payload.get("record_type").and_then(|v| v.as_str()) != Some(record_type) {
        anyhow::bail!("record_type must be '{record_type}'");
    }
    Ok(())
}
