use camino::{Utf8Path, Utf8PathBuf};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Map;
use std::collections::HashMap;

use ccb_storage::atomic::atomic_write_json;

pub const SCHEMA_VERSION: i32 = 1;

pub const ACTIVITY_ACTIVE: &str = "active";
pub const ACTIVITY_PENDING: &str = "pending";
pub const ACTIVITY_IDLE: &str = "idle";
pub const ACTIVITY_FAILED: &str = "failed";

pub const ACTIVITY_STATES: &[&str] = &[
    ACTIVITY_ACTIVE,
    ACTIVITY_PENDING,
    ACTIVITY_IDLE,
    ACTIVITY_FAILED,
];

const STATE_ALIASES: &[(&str, &str)] = &[
    ("active", ACTIVITY_ACTIVE),
    ("running", ACTIVITY_ACTIVE),
    ("tool", ACTIVITY_ACTIVE),
    ("working", ACTIVITY_ACTIVE),
    ("thinking", ACTIVITY_ACTIVE),
    ("pending", ACTIVITY_PENDING),
    ("waiting", ACTIVITY_PENDING),
    ("blocked", ACTIVITY_PENDING),
    ("permission", ACTIVITY_PENDING),
    ("idle", ACTIVITY_IDLE),
    ("done", ACTIVITY_IDLE),
    ("stop", ACTIVITY_IDLE),
    ("stopped", ACTIVITY_IDLE),
    ("failed", ACTIVITY_FAILED),
    ("failure", ACTIVITY_FAILED),
    ("error", ACTIVITY_FAILED),
    ("errored", ACTIVITY_FAILED),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderActivityEvidence {
    pub state: String,
    pub source: String,
    pub reason: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActivityPayload {
    schema_version: i32,
    record_type: String,
    project_id: String,
    agent_name: String,
    provider: String,
    state: String,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ccb_session_id: Option<String>,
    runtime_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pane_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    updated_at: String,
    diagnostics: Map<String, serde_json::Value>,
}

pub fn activity_path(runtime_dir: impl AsRef<Utf8Path>) -> Utf8PathBuf {
    expand_user_path(runtime_dir.as_ref()).join("activity.json")
}

pub fn normalize_activity_state(value: impl AsRef<str>) -> Option<&'static str> {
    let token: String = value
        .as_ref()
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if token.is_empty() {
        return None;
    }
    STATE_ALIASES
        .iter()
        .find(|(alias, _)| *alias == token)
        .map(|(_, state)| *state)
}

#[allow(clippy::too_many_arguments)]
pub fn write_activity(
    provider: &str,
    project_id: &str,
    agent_name: &str,
    runtime_dir: impl AsRef<Utf8Path>,
    state: &str,
    source: &str,
    event_name: Option<&str>,
    ccb_session_id: Option<&str>,
    pane_id: Option<&str>,
    workspace_path: Option<&str>,
    provider_session_id: Option<&str>,
    provider_turn_id: Option<&str>,
    model: Option<&str>,
    diagnostics: Option<&HashMap<String, serde_json::Value>>,
    updated_at: Option<&str>,
) -> Result<Utf8PathBuf, crate::HookError> {
    let normalized_state = normalize_activity_state(state).ok_or_else(|| {
        crate::HookError::UnsupportedState(format!(
            "unsupported provider activity state: {state:?}"
        ))
    })?;
    let runtime = expand_user_path(runtime_dir.as_ref());
    let payload = ActivityPayload {
        schema_version: SCHEMA_VERSION,
        record_type: "provider_activity".into(),
        project_id: project_id.trim().into(),
        agent_name: agent_name.trim().into(),
        provider: provider.trim().to_lowercase(),
        state: normalized_state.into(),
        source: {
            let trimmed = source.trim();
            if trimmed.is_empty() {
                "provider_hook".into()
            } else {
                trimmed.into()
            }
        },
        event_name: optional_text(event_name),
        ccb_session_id: optional_text(ccb_session_id),
        runtime_dir: runtime.to_string(),
        pane_id: optional_text(pane_id),
        workspace_path: workspace_path.map(expand_user_path_str),
        provider_session_id: optional_text(provider_session_id),
        provider_turn_id: optional_text(provider_turn_id),
        model: optional_text(model),
        updated_at: updated_at.map_or_else(utc_now_z, |s| s.trim().into()),
        diagnostics: safe_diagnostics(diagnostics),
    };
    let path = activity_path(&runtime);
    if normalized_state == ACTIVITY_IDLE && existing_failed_same_identity(&path, &payload)? {
        return Ok(path);
    }
    atomic_write_json(&path, &payload)?;
    Ok(path)
}

pub fn load_activity(
    runtime_dir: impl AsRef<Utf8Path>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let path = activity_path(runtime_dir);
    let text = std::fs::read_to_string(&path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.as_object().cloned()
}

#[allow(clippy::too_many_arguments)]
pub fn read_activity_evidence(
    runtime_dir: impl AsRef<Utf8Path>,
    project_id: &str,
    agent_name: &str,
    provider: &str,
    ccb_session_id: Option<&str>,
    provider_session_id: Option<&str>,
    pane_id: Option<&str>,
    workspace_path: Option<&str>,
    now: Option<&str>,
    max_future_skew_s: f64,
) -> Option<ProviderActivityEvidence> {
    let payload = load_activity(runtime_dir)?;
    if !matches_identity(
        &payload,
        project_id,
        agent_name,
        provider,
        ccb_session_id,
        provider_session_id,
        pane_id,
        workspace_path,
    ) {
        return None;
    }
    let state = normalize_activity_state(payload.get("state")?.as_str()?).map(|s| s.to_string())?;
    let updated_at = payload.get("updated_at")?.as_str()?.trim().to_string();
    if !timestamp_usable(&updated_at, now, max_future_skew_s) {
        return None;
    }
    let source = payload
        .get("source")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "provider_activity".into());
    let reason = reason(&payload, &state);
    let diagnostics = payload
        .get("diagnostics")
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_else(Map::new);
    Some(ProviderActivityEvidence {
        state,
        source,
        reason,
        updated_at,
        event_name: payload
            .get("event_name")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        provider_session_id: payload
            .get("provider_session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        provider_turn_id: payload
            .get("provider_turn_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        model: payload
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        diagnostics: Some(diagnostics),
    })
}

#[allow(clippy::too_many_arguments)]
fn matches_identity(
    payload: &serde_json::Map<String, serde_json::Value>,
    project_id: &str,
    agent_name: &str,
    provider: &str,
    ccb_session_id: Option<&str>,
    provider_session_id: Option<&str>,
    pane_id: Option<&str>,
    workspace_path: Option<&str>,
) -> bool {
    if payload.get("schema_version").and_then(|v| v.as_i64()) != Some(i64::from(SCHEMA_VERSION)) {
        return false;
    }
    if payload
        .get("record_type")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        != Some("provider_activity")
    {
        return false;
    }
    if payload
        .get("project_id")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        != Some(project_id.trim())
    {
        return false;
    }
    if payload
        .get("agent_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        != Some(agent_name.trim())
    {
        return false;
    }
    if payload
        .get("provider")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        != Some(provider.trim().to_lowercase())
    {
        return false;
    }
    if payload
        .get("runtime_dir")
        .and_then(|v| v.as_str())
        .map(path_text)
        != Some(path_text(runtime_dir_from_payload(payload).as_str()))
    {
        return false;
    }
    if !optional_matches(payload.get("ccb_session_id"), ccb_session_id) {
        return false;
    }
    if !optional_matches(payload.get("pane_id"), pane_id) {
        return false;
    }
    let recorded_workspace = payload
        .get("workspace_path")
        .and_then(|v| v.as_str())
        .map(path_text);
    let expected_workspace = workspace_path.map(path_text);
    if let (Some(r), Some(e)) = (recorded_workspace, expected_workspace) {
        if r != e {
            return false;
        }
    }
    // provider_session_id is intentionally stored as a diagnostic identity clue but not used
    // as a hard matching requirement, matching Python behavior.
    let _ = provider_session_id;
    true
}

fn runtime_dir_from_payload(payload: &serde_json::Map<String, serde_json::Value>) -> String {
    payload
        .get("runtime_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn existing_failed_same_identity(
    path: &Utf8Path,
    next_payload: &ActivityPayload,
) -> Result<bool, crate::HookError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    let current: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&text) {
        Ok(serde_json::Value::Object(m)) => m,
        _ => return Ok(false),
    };
    if normalize_activity_state(current.get("state").and_then(|v| v.as_str()).unwrap_or(""))
        != Some(ACTIVITY_FAILED)
    {
        return Ok(false);
    }
    for key in &[
        "project_id",
        "agent_name",
        "provider",
        "runtime_dir",
        "ccb_session_id",
        "provider_session_id",
        "pane_id",
    ] {
        let current_text = current
            .get(*key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let next_text = match *key {
            "project_id" => next_payload.project_id.as_str(),
            "agent_name" => next_payload.agent_name.as_str(),
            "provider" => next_payload.provider.as_str(),
            "runtime_dir" => next_payload.runtime_dir.as_str(),
            "ccb_session_id" => next_payload.ccb_session_id.as_deref().unwrap_or(""),
            "provider_session_id" => next_payload.provider_session_id.as_deref().unwrap_or(""),
            "pane_id" => next_payload.pane_id.as_deref().unwrap_or(""),
            _ => "",
        }
        .trim();
        if !current_text.is_empty() && !next_text.is_empty() && current_text != next_text {
            return Ok(false);
        }
    }
    Ok(true)
}

fn timestamp_usable(updated_at: &str, now: Option<&str>, max_future_skew_s: f64) -> bool {
    let observed = match parse_timestamp(updated_at) {
        Some(t) => t,
        None => return false,
    };
    let current = match now {
        None => return true,
        Some(n) => match parse_timestamp(n) {
            Some(t) => t,
            None => return false,
        },
    };
    (observed - current).num_milliseconds() as f64 / 1000.0 <= max_future_skew_s
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    let text = if let Some(stripped) = text.strip_suffix('Z') {
        format!("{stripped}+00:00")
    } else {
        text.to_string()
    };
    let parsed = text.parse::<DateTime<chrono::FixedOffset>>().ok()?;
    Some(parsed.with_timezone(&Utc))
}

fn utc_now_z() -> String {
    Utc::now()
        .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
        .replace("+00:00", "Z")
}

fn optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn expand_user_path_str(value: &str) -> String {
    expand_user_path(Utf8Path::new(value)).to_string()
}

fn expand_user_path(path: &Utf8Path) -> Utf8PathBuf {
    if let Some(rest) = path.as_str().strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return Utf8PathBuf::from(format!("{home}{rest}"));
        }
    }
    path.to_path_buf()
}

fn path_text(value: &str) -> String {
    let text = value.trim();
    if text.is_empty() {
        return String::new();
    }
    expand_user_path(Utf8Path::new(text))
        .canonicalize()
        .ok()
        .and_then(|p| Utf8PathBuf::from_path_buf(p).ok())
        .map(|p| p.to_string())
        .unwrap_or_else(|| expand_user_path(Utf8Path::new(text)).to_string())
}

fn optional_matches(recorded: Option<&serde_json::Value>, expected: Option<&str>) -> bool {
    let recorded_text = recorded.and_then(|v| v.as_str()).unwrap_or("").trim();
    let expected_text = expected.unwrap_or("").trim();
    recorded_text.is_empty() || expected_text.is_empty() || recorded_text == expected_text
}

fn reason(payload: &serde_json::Map<String, serde_json::Value>, state: &str) -> String {
    if let Some(diagnostics) = payload.get("diagnostics").and_then(|v| v.as_object()) {
        if let Some(reason) = diagnostics.get("reason").and_then(|v| v.as_str()) {
            let trimmed = reason.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    if let Some(event_name) = payload.get("event_name").and_then(|v| v.as_str()) {
        let trimmed = event_name.trim();
        if !trimmed.is_empty() {
            return format!("provider_{trimmed}");
        }
    }
    format!("provider_activity_{state}")
}

fn safe_diagnostics(
    diagnostics: Option<&HashMap<String, serde_json::Value>>,
) -> Map<String, serde_json::Value> {
    let mut result = Map::new();
    let Some(diagnostics) = diagnostics else {
        return result;
    };
    for (key, value) in diagnostics {
        let name = key.trim();
        if name.is_empty() {
            continue;
        }
        let lowered = name.to_lowercase();
        if lowered.contains("key")
            || lowered.contains("token")
            || lowered.contains("secret")
            || lowered.contains("password")
        {
            continue;
        }
        if value.is_boolean() || value.is_i64() || value.is_u64() || value.is_f64() {
            result.insert(name.to_string(), value.clone());
        } else if !value.is_null() {
            let text = value
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| value.to_string())
                .trim()
                .to_string();
            result.insert(
                name.to_string(),
                serde_json::Value::String(text.chars().take(300).collect()),
            );
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn test_normalize_activity_state() {
        assert_eq!(normalize_activity_state("tool"), Some(ACTIVITY_ACTIVE));
        assert_eq!(normalize_activity_state("waiting"), Some(ACTIVITY_PENDING));
        assert_eq!(normalize_activity_state("stopped"), Some(ACTIVITY_IDLE));
        assert_eq!(normalize_activity_state("failure"), Some(ACTIVITY_FAILED));
        assert_eq!(normalize_activity_state("nope"), None);
        assert_eq!(normalize_activity_state(""), None);
    }

    #[test]
    fn test_write_and_load_activity() {
        let dir = TempDir::new().unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap();
        let runtime = path.join("runtime");

        let result = write_activity(
            "codex",
            "project-1",
            "agent2",
            &runtime,
            "tool",
            "codex_hook",
            Some("PreToolUse"),
            Some("ccb-agent2-1"),
            Some("%42"),
            Some("/tmp/workspace"),
            None,
            None,
            None,
            None,
            Some("2026-05-27T00:00:00Z"),
        )
        .unwrap();

        assert_eq!(result, runtime.join("activity.json"));
        let payload = load_activity(&runtime).unwrap();
        assert_eq!(payload["record_type"], "provider_activity");
        assert_eq!(payload["state"], "active");
        assert_eq!(payload["provider"], "codex");
        assert_eq!(payload["agent_name"], "agent2");
        assert_eq!(payload["event_name"], "PreToolUse");
    }

    #[test]
    fn test_failed_activity_is_sticky() {
        let dir = TempDir::new().unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap();
        let runtime = path.join("runtime");

        write_activity(
            "codex",
            "project-1",
            "agent2",
            &runtime,
            "failed",
            "codex_hook",
            None,
            Some("ccb-1"),
            Some("%1"),
            None,
            None,
            None,
            None,
            None,
            Some("2026-05-27T00:00:00Z"),
        )
        .unwrap();

        write_activity(
            "codex",
            "project-1",
            "agent2",
            &runtime,
            "idle",
            "codex_hook",
            None,
            Some("ccb-1"),
            Some("%1"),
            None,
            None,
            None,
            None,
            None,
            Some("2026-05-27T00:00:01Z"),
        )
        .unwrap();

        let payload = load_activity(&runtime).unwrap();
        assert_eq!(payload["state"], "failed");

        write_activity(
            "codex",
            "project-1",
            "agent2",
            &runtime,
            "active",
            "codex_hook",
            None,
            Some("ccb-1"),
            Some("%1"),
            None,
            None,
            None,
            None,
            None,
            Some("2026-05-27T00:00:02Z"),
        )
        .unwrap();

        let payload = load_activity(&runtime).unwrap();
        assert_eq!(payload["state"], "active");
    }

    #[test]
    fn test_read_activity_evidence_matching_identity() {
        let dir = TempDir::new().unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap();
        let runtime = path.join("runtime");
        let workspace = path.join("workspace");

        write_activity(
            "claude",
            "project-1",
            "agent3",
            &runtime,
            "waiting",
            "claude_hook",
            Some("Notification"),
            Some("ccb-agent3-1"),
            Some("%5"),
            Some(workspace.as_str()),
            None,
            None,
            None,
            None,
            Some("2026-05-27T00:00:00Z"),
        )
        .unwrap();

        let evidence = read_activity_evidence(
            &runtime,
            "project-1",
            "agent3",
            "claude",
            Some("ccb-agent3-1"),
            None,
            Some("%5"),
            Some(workspace.as_str()),
            Some("2026-05-27T00:00:05Z"),
            30.0,
        )
        .unwrap();

        assert_eq!(evidence.state, "pending");
        assert_eq!(evidence.source, "claude_hook");
        assert_eq!(evidence.reason, "provider_Notification");
    }

    #[test]
    fn test_safe_diagnostics_strips_secrets() {
        let mut diagnostics = HashMap::new();
        diagnostics.insert("tool_name".into(), json!("shell"));
        diagnostics.insert("api_key".into(), json!("secret"));
        diagnostics.insert("token".into(), json!("secret"));
        diagnostics.insert("reason".into(), json!("api_error"));
        let safe = safe_diagnostics(Some(&diagnostics));
        assert!(safe.contains_key("tool_name"));
        assert!(!safe.contains_key("api_key"));
        assert!(!safe.contains_key("token"));
        assert_eq!(safe["reason"], "api_error");
    }

    #[test]
    fn test_write_activity_defaults_empty_source_to_provider_hook() {
        let dir = TempDir::new().unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap();
        let runtime = path.join("runtime");

        write_activity(
            "codex",
            "project-1",
            "agent2",
            &runtime,
            "tool",
            "   ",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("2026-05-27T00:00:00Z"),
        )
        .unwrap();

        let payload = load_activity(&runtime).unwrap();
        assert_eq!(payload["source"], "provider_hook");

        let evidence = read_activity_evidence(
            &runtime,
            "project-1",
            "agent2",
            "codex",
            None,
            None,
            None,
            None,
            Some("2026-05-27T00:00:05Z"),
            30.0,
        )
        .unwrap();
        assert_eq!(evidence.source, "provider_hook");
    }

    #[test]
    fn test_read_activity_evidence_missing_diagnostics_is_empty_object() {
        let dir = TempDir::new().unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap();
        let runtime = path.join("runtime");

        write_activity(
            "claude",
            "project-1",
            "agent3",
            &runtime,
            "waiting",
            "claude_hook",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("2026-05-27T00:00:00Z"),
        )
        .unwrap();

        let evidence = read_activity_evidence(
            &runtime,
            "project-1",
            "agent3",
            "claude",
            None,
            None,
            None,
            None,
            Some("2026-05-27T00:00:05Z"),
            30.0,
        )
        .unwrap();

        assert_eq!(evidence.diagnostics, Some(Map::new()));
    }
}
