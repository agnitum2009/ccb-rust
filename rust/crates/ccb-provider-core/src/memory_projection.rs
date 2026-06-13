use std::path::Path;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{ProviderCoreError, Result};

/// Result of a memory projection operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryProjectionResult {
    pub status: String,
    pub reason: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub source_count: i64,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub error_detail: String,
}

/// Build a normalized memory projection result.
#[allow(clippy::too_many_arguments)]
pub fn memory_projection_result(
    status: &str,
    reason: &str,
    path: &Path,
    sha256: Option<&str>,
    source_count: Option<i64>,
    warnings: Option<&[String]>,
    error_detail: Option<&str>,
) -> MemoryProjectionResult {
    MemoryProjectionResult {
        status: status.to_string(),
        reason: reason.to_string(),
        path: path.to_string_lossy().to_string(),
        sha256: sha256.unwrap_or("").to_string(),
        source_count: source_count.unwrap_or(0),
        warnings: warnings
            .unwrap_or(&[])
            .iter()
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .collect(),
        error_detail: error_detail.unwrap_or("").to_string(),
    }
}

/// Record a memory-projection agent event, deduplicating against the marker.
pub fn record_memory_projection_event(
    result: &MemoryProjectionResult,
    provider: &str,
    event_path: Option<&Path>,
    marker_path: Option<&Path>,
    agent_name: Option<&str>,
) -> Result<()> {
    let event_path = match event_path {
        Some(p) => p,
        None => return Ok(()),
    };
    let marker_path = match marker_path {
        Some(p) => p,
        None => return Ok(()),
    };
    let provider_name = provider.trim();
    if provider_name.is_empty() {
        return Ok(());
    }
    let agent_name = match agent_name {
        Some(a) if !a.trim().is_empty() => a,
        _ => return Ok(()),
    };

    let status = result.status.clone();
    let reason = result.reason.clone();
    let signature = json!({
        "status": status.clone(),
        "reason": reason.clone(),
        "path": result.path.clone(),
        "sha256": result.sha256.clone(),
        "warnings": result.warnings.clone(),
    });

    if same_memory_projection_signature(marker_path, &signature) {
        return Ok(());
    }

    let event = json!({
        "record_type": "agent_event",
        "event_type": format!("{}_memory_projection_{}", provider_name, status),
        "provider": provider_name,
        "agent_name": agent_name,
        "status": status,
        "reason": reason,
        "projection_path": result.path,
        "sha256": result.sha256,
        "source_count": result.source_count,
        "warnings": result.warnings,
        "error_detail": result.error_detail,
        "created_at": now_utc_rfc3339(),
    });

    write_projection_event_and_marker(&event, &signature, event_path, marker_path)
}

/// Append an event to a JSONL file and write its signature marker.
pub fn write_projection_event_and_marker(
    event: &serde_json::Value,
    signature: &serde_json::Value,
    event_path: &Path,
    marker_path: &Path,
) -> Result<()> {
    let event_utf8 = Utf8Path::from_path(event_path).ok_or_else(|| {
        ProviderCoreError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "event path is not valid utf-8",
        ))
    })?;
    let marker_utf8 = Utf8Path::from_path(marker_path).ok_or_else(|| {
        ProviderCoreError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "marker path is not valid utf-8",
        ))
    })?;

    let store = ccb_storage::jsonl::JsonlStore::new();
    store.append(event_utf8, event)?;
    ccb_storage::atomic::atomic_write_json(marker_utf8, signature)?;
    Ok(())
}

/// Check whether `payload` matches the signature stored at `marker_path`.
pub fn same_memory_projection_signature(marker_path: &Path, payload: &serde_json::Value) -> bool {
    let existing = match std::fs::read_to_string(marker_path) {
        Ok(text) => text,
        Err(_) => return false,
    };
    let existing: serde_json::Value = match serde_json::from_str(&existing) {
        Ok(serde_json::Value::Object(m)) => serde_json::Value::Object(m),
        _ => return false,
    };

    if existing == *payload {
        return true;
    }

    let payload_obj = match payload.as_object() {
        Some(o) => o,
        None => return false,
    };

    let status = payload_obj
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let reason = payload_obj
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if status == "skipped" && reason == "unchanged" {
        let sha256 = payload_obj
            .get("sha256")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        return !sha256.is_empty()
            && field_eq(&existing, payload, "path")
            && field_eq(&existing, payload, "sha256")
            && field_eq(&existing, payload, "warnings");
    }

    if status == "skipped" {
        return field_eq(&existing, payload, "reason")
            && field_eq(&existing, payload, "path")
            && field_eq(&existing, payload, "sha256")
            && field_eq(&existing, payload, "warnings");
    }

    false
}

fn field_eq(left: &serde_json::Value, right: &serde_json::Value, key: &str) -> bool {
    left.get(key) == right.get(key)
}

/// Compute the SHA-256 hex digest of an existing text file.
pub fn text_file_sha256(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(text) => sha256_text(&text),
        Err(_) => String::new(),
    }
}

fn sha256_text(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

fn now_utc_rfc3339() -> String {
    chrono::Utc::now()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        .replace("+00:00", "Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_projection_result_normalizes() {
        let tmp = std::env::temp_dir();
        let result = memory_projection_result(
            "failed",
            "missing_project_context",
            &tmp.join("AGENTS.md"),
            None,
            None,
            Some(&["warn".to_string(), "".to_string(), "also-warn".to_string()]),
            None,
        );
        assert_eq!(result.status, "failed");
        assert_eq!(result.warnings, vec!["warn", "also-warn"]);
        assert_eq!(result.error_detail, "");
    }
}
