use camino::{Utf8Path, Utf8PathBuf};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Map;
use std::collections::HashMap;
use std::sync::LazyLock;

use ccb_storage::atomic::atomic_write_json;

pub const SCHEMA_VERSION: i32 = 1;

// Match Python provider_core.protocol_runtime.constants exactly.
const ANY_REQ_ID_PATTERN: &str = r"(?:job_[a-z0-9]+|[0-9a-fA-F]{32}|\d{8}-\d{6}-\d{3}-\d+-\d+)";

static REQ_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)CCB_REQ_ID:\s*({ANY_REQ_ID_PATTERN})(?-i:[^A-Za-z0-9_-]|$)"
    ))
    .unwrap()
});

static OUTER_REQ_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)^\s*CCB_REQ_ID:\s*({ANY_REQ_ID_PATTERN})(?-i:[^A-Za-z0-9_-]|$)"
    ))
    .unwrap()
});

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompletionEventPayload {
    schema_version: i32,
    record_type: String,
    provider: String,
    agent_name: String,
    workspace_path: String,
    req_id: String,
    status: String,
    reply: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hook_event_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transcript_path: Option<String>,
    diagnostics: Map<String, serde_json::Value>,
    timestamp: String,
}

pub fn event_path(completion_dir: impl AsRef<Utf8Path>, req_id: &str) -> Utf8PathBuf {
    expand_user_path(completion_dir.as_ref())
        .join("events")
        .join(format!("{req_id}.json"))
}

pub fn completion_dir_from_session_data(
    session_data: &HashMap<String, serde_json::Value>,
) -> Option<Utf8PathBuf> {
    let explicit = session_data
        .get("completion_artifact_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if !explicit.is_empty() {
        return Some(expand_user_path(Utf8Path::new(explicit)));
    }
    let runtime_dir = session_data
        .get("runtime_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if !runtime_dir.is_empty() {
        return Some(expand_user_path(Utf8Path::new(runtime_dir)).join("completion"));
    }
    None
}

pub fn extract_req_id(text: &str) -> Option<String> {
    REQ_ID_RE
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn extract_outer_req_id(text: &str) -> Option<String> {
    OUTER_REQ_ID_RE
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn latest_req_id_from_transcript(transcript_path: Option<&str>) -> Option<String> {
    let path = transcript_path?;
    let path = expand_user_path(Utf8Path::new(path));
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    latest_req_id_from_transcript_text(&content)
}

pub fn latest_req_id_from_transcript_text(content: &str) -> Option<String> {
    if let Some(req_id) = latest_user_req_id_from_transcript_text(content) {
        return Some(req_id);
    }
    if let Some(req_id) = latest_last_prompt_req_id_from_transcript_text(content) {
        return Some(req_id);
    }
    extract_outer_req_id(content)
}

pub fn latest_user_req_id_from_transcript_text(content: &str) -> Option<String> {
    let mut latest: Option<String> = None;
    for line in content.lines() {
        let record: serde_json::Value = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let text = match user_message_text(&record) {
            Some(t) => t,
            None => continue,
        };
        if let Some(req_id) = extract_outer_req_id(&text) {
            latest = Some(req_id);
        }
    }
    latest
}

pub fn latest_last_prompt_req_id_from_transcript_text(content: &str) -> Option<String> {
    let mut latest: Option<String> = None;
    for line in content.lines() {
        let record: serde_json::Value = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let obj = match record.as_object() {
            Some(o) => o,
            None => continue,
        };
        if obj
            .get("type")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_lowercase())
            != Some("last-prompt".to_string())
        {
            continue;
        }
        let prompt = obj.get("lastPrompt").and_then(|v| v.as_str()).unwrap_or("");
        if let Some(req_id) = extract_outer_req_id(prompt) {
            latest = Some(req_id);
        }
    }
    latest
}

pub fn current_turn_req_id_from_transcript(
    transcript_path: Option<&str>,
    assistant_reply: Option<&str>,
) -> Option<String> {
    let path = transcript_path?;
    let path = expand_user_path(Utf8Path::new(path));
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    current_turn_req_id_from_transcript_text(&content, assistant_reply)
}

pub fn current_turn_req_id_from_transcript_text(
    content: &str,
    assistant_reply: Option<&str>,
) -> Option<String> {
    let records = parse_jsonl_records(content);
    if records.is_empty() {
        return None;
    }
    if assistant_reply.unwrap_or("").trim().is_empty() {
        return empty_reply_turn_req_id(&records, content);
    }
    let index = match current_assistant_index(&records, assistant_reply) {
        Some(i) => i,
        None => {
            // When the transcript has no assistant records at all, fall back to
            // the latest user/last-prompt req id. Mirrors Python behavior.
            if records.iter().any(is_assistant_record) {
                return None;
            }
            return latest_req_id_from_transcript_text(content);
        }
    };
    let indexed = uuid_index(&records);
    req_id_for_assistant_turn(&records[index], &indexed)
}

fn parse_jsonl_records(content: &str) -> Vec<serde_json::Value> {
    content
        .lines()
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
            if value.is_object() {
                Some(value)
            } else {
                None
            }
        })
        .collect()
}

fn uuid_index(records: &[serde_json::Value]) -> HashMap<String, &serde_json::Value> {
    let mut indexed = HashMap::new();
    for record in records {
        if let Some(uuid) = record
            .get("uuid")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
        {
            if !uuid.is_empty() {
                indexed.insert(uuid, record);
            }
        }
    }
    indexed
}

fn current_assistant_index(
    records: &[serde_json::Value],
    assistant_reply: Option<&str>,
) -> Option<usize> {
    let expected = assistant_reply.unwrap_or("").trim();
    if !expected.is_empty() {
        for (index, record) in records.iter().enumerate().rev() {
            if !is_assistant_record(record) {
                continue;
            }
            if assistant_message_text(record).trim() == expected {
                return Some(index);
            }
        }
    }
    for (index, record) in records.iter().enumerate().rev() {
        if is_assistant_record(record) && !assistant_message_text(record).trim().is_empty() {
            return Some(index);
        }
    }
    None
}

fn empty_reply_turn_req_id(records: &[serde_json::Value], content: &str) -> Option<String> {
    let mut latest_ccb_user: Option<(usize, String)> = None;
    let mut latest_assistant_index: Option<usize> = None;
    for (index, record) in records.iter().enumerate() {
        if is_assistant_record(record) {
            latest_assistant_index = Some(index);
        }
        if let Some(text) = user_message_text(record) {
            if let Some(req_id) = extract_outer_req_id(&text) {
                latest_ccb_user = Some((index, req_id));
            }
        }
    }
    if let Some((user_index, req_id)) = latest_ccb_user {
        if latest_assistant_index.is_none_or(|ai| user_index > ai) {
            return Some(req_id);
        }
    }
    if latest_assistant_index.is_none() {
        return latest_req_id_from_transcript_text(content);
    }
    None
}

fn req_id_for_assistant_turn(
    record: &serde_json::Value,
    indexed: &HashMap<String, &serde_json::Value>,
) -> Option<String> {
    let mut parent_uuid = record
        .get("parentUuid")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let mut seen = std::collections::HashSet::new();
    while !parent_uuid.is_empty() && !seen.contains(&parent_uuid) {
        seen.insert(parent_uuid.clone());
        let parent = match indexed.get(&parent_uuid) {
            Some(p) => *p,
            None => return None,
        };
        if is_tool_result_user_record(parent) {
            parent_uuid = parent
                .get("parentUuid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            continue;
        }
        if is_user_record(parent) {
            return extract_outer_req_id(&user_message_text(parent).unwrap_or_default());
        }
        parent_uuid = parent
            .get("parentUuid")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
    }
    None
}

fn is_user_record(record: &serde_json::Value) -> bool {
    let message = match record.get("message").and_then(|v| v.as_object()) {
        Some(m) => m,
        None => return false,
    };
    if record
        .get("type")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        != Some("user".to_string())
    {
        return false;
    }
    let role = message
        .get("role")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    role.is_empty() || role == "user"
}

fn is_tool_result_user_record(record: &serde_json::Value) -> bool {
    if !is_user_record(record) {
        return false;
    }
    if record
        .get("toolUseResult")
        .and_then(|v| v.as_object())
        .is_some()
    {
        return true;
    }
    let content = record
        .get("message")
        .and_then(|v| v.as_object())
        .and_then(|m| m.get("content"))
        .and_then(|v| v.as_array());
    let content = match content {
        Some(c) if !c.is_empty() => c,
        _ => return false,
    };
    content.iter().all(|item| {
        item.as_object()
            .and_then(|o| o.get("type"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_lowercase())
            == Some("tool_result".to_string())
    })
}

fn is_assistant_record(record: &serde_json::Value) -> bool {
    if record
        .get("type")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        != Some("assistant".to_string())
    {
        return false;
    }
    let message = match record.get("message").and_then(|v| v.as_object()) {
        Some(m) => m,
        None => return false,
    };
    let role = message
        .get("role")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    role.is_empty() || role == "assistant"
}

fn assistant_message_text(record: &serde_json::Value) -> String {
    let message = match record.get("message").and_then(|v| v.as_object()) {
        Some(m) => m,
        None => return String::new(),
    };
    content_text(message.get("content"))
}

fn user_message_text(record: &serde_json::Value) -> Option<String> {
    let message = record.get("message").and_then(|v| v.as_object())?;
    if record
        .get("type")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        != Some("user".to_string())
    {
        return None;
    }
    let role = message
        .get("role")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    if !role.is_empty() && role != "user" {
        return None;
    }
    Some(content_text(message.get("content")))
}

fn content_text(content: Option<&serde_json::Value>) -> String {
    match content {
        None => String::new(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(parts)) => {
            let mut texts = Vec::new();
            for item in parts {
                match item {
                    serde_json::Value::String(s) => texts.push(s.clone()),
                    serde_json::Value::Object(o) => {
                        if let Some(text) = o.get("text").and_then(|v| v.as_str()) {
                            texts.push(text.to_string());
                        }
                    }
                    _ => {}
                }
            }
            texts.join("\n")
        }
        Some(other) => other.to_string(),
    }
}

pub fn load_event(
    completion_dir: impl AsRef<Utf8Path>,
    req_id: &str,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let path = event_path(completion_dir, req_id);
    if !path.exists() {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let obj = value.as_object().cloned()?;
    if normalized_req_id(obj.get("req_id").and_then(|v| v.as_str()).unwrap_or(""))
        != normalized_req_id(req_id)
    {
        return None;
    }
    Some(obj)
}

#[allow(clippy::too_many_arguments)]
pub fn write_event(
    provider: &str,
    completion_dir: impl AsRef<Utf8Path>,
    agent_name: &str,
    workspace_path: &str,
    req_id: &str,
    status: &str,
    reply: &str,
    session_id: Option<&str>,
    hook_event_name: Option<&str>,
    transcript_path: Option<&str>,
    diagnostics: Option<&HashMap<String, serde_json::Value>>,
) -> Result<Utf8PathBuf, crate::HookError> {
    let payload = event_payload(
        provider,
        agent_name,
        workspace_path,
        req_id,
        status,
        reply,
        session_id,
        hook_event_name,
        transcript_path,
        diagnostics,
    );
    let path = event_path(completion_dir, req_id);
    atomic_write_json(&path, &payload)?;
    Ok(path)
}

#[allow(clippy::too_many_arguments)]
fn event_payload(
    provider: &str,
    agent_name: &str,
    workspace_path: &str,
    req_id: &str,
    status: &str,
    reply: &str,
    session_id: Option<&str>,
    hook_event_name: Option<&str>,
    transcript_path: Option<&str>,
    diagnostics: Option<&HashMap<String, serde_json::Value>>,
) -> CompletionEventPayload {
    CompletionEventPayload {
        schema_version: SCHEMA_VERSION,
        record_type: "provider_completion_hook".into(),
        provider: provider.trim().to_lowercase(),
        agent_name: agent_name.trim().into(),
        workspace_path: workspace_path.trim().into(),
        req_id: normalized_req_id(req_id),
        status: status.trim().to_lowercase(),
        reply: reply.into(),
        session_id: optional_text(session_id),
        hook_event_name: optional_text(hook_event_name),
        transcript_path: transcript_path.map(expand_user_path_str),
        diagnostics: diagnostics.map_or_else(Map::new, |d| {
            let mut map = Map::new();
            for (k, v) in d {
                map.insert(k.clone(), v.clone());
            }
            map
        }),
        timestamp: Utc::now().to_rfc3339(),
    }
}

fn normalized_req_id(req_id: &str) -> String {
    req_id.trim().into()
}

fn optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn expand_user_path(path: &Utf8Path) -> Utf8PathBuf {
    if let Some(rest) = path.as_str().strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return Utf8PathBuf::from(format!("{home}{rest}"));
        }
    }
    path.to_path_buf()
}

fn expand_user_path_str(value: &str) -> String {
    expand_user_path(Utf8Path::new(value)).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn jsonl(records: &[serde_json::Value]) -> String {
        records
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }

    #[test]
    fn test_extract_req_id_patterns() {
        assert_eq!(
            extract_req_id("CCB_REQ_ID: job_current123"),
            Some("job_current123".into())
        );
        assert_eq!(
            extract_req_id("CCB_REQ_ID: 1234567890abcdef1234567890abcdef"),
            Some("1234567890abcdef1234567890abcdef".into())
        );
        assert_eq!(
            extract_req_id("CCB_REQ_ID: 20240101-120000-000-1-1"),
            Some("20240101-120000-000-1-1".into())
        );
        assert_eq!(extract_req_id("no marker"), None);
    }

    #[test]
    fn test_extract_outer_req_id_requires_start() {
        assert_eq!(
            extract_outer_req_id("CCB_REQ_ID: job_current123\nbody"),
            Some("job_current123".into())
        );
        assert_eq!(
            extract_outer_req_id("body CCB_REQ_ID: job_current123"),
            None
        );
    }

    #[test]
    fn test_latest_user_req_id_uses_outer_marker() {
        let content = jsonl(&[json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": "CCB_REQ_ID: job_current123\n\nReview this transcript:\nCCB_REQ_ID: job_old456\n```text\nCCB_REQ_ID: job_code789\n```"
            }
        })]);
        assert_eq!(
            latest_user_req_id_from_transcript_text(&content),
            Some("job_current123".into())
        );
    }

    #[test]
    fn test_latest_user_req_id_ignores_body_only() {
        let content = jsonl(&[json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": "Please inspect why CCB_REQ_ID: job_old456 did not return."
            }
        })]);
        assert_eq!(latest_user_req_id_from_transcript_text(&content), None);
    }

    #[test]
    fn test_current_turn_req_id_follows_parent_chain() {
        let content = jsonl(&[
            json!({"uuid": "u1", "type": "user", "message": {"role": "user", "content": "CCB_REQ_ID: job_current123\n\nRun a tool."}}),
            json!({"uuid": "a1", "parentUuid": "u1", "type": "assistant", "message": {"role": "assistant", "content": [{"type": "tool_use", "name": "Read"}]}}),
            json!({"uuid": "u2", "parentUuid": "a1", "type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "content": "ok"}]}, "toolUseResult": {"type": "text"}}),
            json!({"uuid": "a2", "parentUuid": "u2", "type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "done"}]}}),
        ]);
        assert_eq!(
            current_turn_req_id_from_transcript_text(&content, Some("done")),
            Some("job_current123".into())
        );
    }

    #[test]
    fn test_current_turn_req_id_empty_reply_after_prior_assistant() {
        let content = jsonl(&[
            json!({"uuid": "u1", "type": "user", "message": {"role": "user", "content": "CCB_REQ_ID: job_previous111\n\nPrevious task."}}),
            json!({"uuid": "a1", "parentUuid": "u1", "type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "previous done"}]}}),
            json!({"uuid": "u2", "type": "user", "message": {"role": "user", "content": "CCB_REQ_ID: job_emptyclaude123\n\nRun the task."}}),
        ]);
        assert_eq!(
            current_turn_req_id_from_transcript_text(&content, None),
            Some("job_emptyclaude123".into())
        );
    }

    #[test]
    fn test_write_and_load_event() {
        let dir = TempDir::new().unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap();
        let completion_dir = path.join("completion");

        let mut diagnostics = HashMap::new();
        diagnostics.insert("hook_event_name".into(), json!("Stop"));

        let result = write_event(
            "claude",
            &completion_dir,
            "agent1",
            "/tmp/workspace",
            "job-123",
            "completed",
            "hello",
            Some("session-1"),
            Some("Stop"),
            None,
            Some(&diagnostics),
        )
        .unwrap();

        assert_eq!(result, completion_dir.join("events").join("job-123.json"));
        let loaded = load_event(&completion_dir, "job-123").unwrap();
        assert_eq!(loaded["provider"], "claude");
        assert_eq!(loaded["status"], "completed");
        assert_eq!(loaded["reply"], "hello");
        assert_eq!(loaded["req_id"], "job-123");
    }
}
