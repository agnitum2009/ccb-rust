use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Tuple returned by `read_new_subagent_events`: (events, new_state).
type SubagentEventTuple = (Vec<(String, String, Option<String>, Option<String>)>, Value);

/// A structured event record produced by the Claude log reader.
///
/// Fields mirror the Python `structured_event` record used by the
/// `ClaudePollState` machine.
#[derive(Debug, Clone)]
pub struct ClaudeLogEntry {
    pub role: String,
    pub text: String,
    pub entry_type: Option<String>,
    pub subtype: Option<String>,
    pub uuid: Option<String>,
    pub parent_uuid: Option<String>,
    pub stop_reason: Option<String>,
    pub subagent_id: Option<String>,
    pub subagent_name: Option<String>,
}

impl ClaudeLogEntry {
    pub fn to_value(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("role".to_string(), Value::String(self.role.clone()));
        obj.insert("text".to_string(), Value::String(self.text.clone()));
        if let Some(v) = &self.entry_type {
            obj.insert("entry_type".to_string(), Value::String(v.clone()));
        }
        if let Some(v) = &self.subtype {
            obj.insert("subtype".to_string(), Value::String(v.clone()));
        }
        if let Some(v) = &self.uuid {
            obj.insert("uuid".to_string(), Value::String(v.clone()));
        }
        if let Some(v) = &self.parent_uuid {
            obj.insert("parent_uuid".to_string(), Value::String(v.clone()));
        }
        if let Some(v) = &self.stop_reason {
            obj.insert("stop_reason".to_string(), Value::String(v.clone()));
        }
        if let Some(v) = &self.subagent_id {
            obj.insert("subagent_id".to_string(), Value::String(v.clone()));
        }
        if let Some(v) = &self.subagent_name {
            obj.insert("subagent_name".to_string(), Value::String(v.clone()));
        }
        Value::Object(obj)
    }
}

/// Reads Claude session JSONL logs (main session + subagent logs).
///
/// Mirrors the subset of Python `ClaudeLogReader` used by the execution
/// adapter: incremental reads, session discovery, and subagent support.
#[derive(Debug, Clone)]
pub struct ClaudeLogReader {
    projects_root: PathBuf,
    work_dir: PathBuf,
    preferred_session: Option<PathBuf>,
    include_subagents: bool,
    include_subagent_user: bool,
    subagent_tag: Option<String>,
}

impl ClaudeLogReader {
    pub fn new(projects_root: Option<&Path>, work_dir: &Path) -> Self {
        Self {
            projects_root: projects_root
                .map(expand_tilde_path)
                .unwrap_or_else(default_projects_root),
            work_dir: work_dir.to_path_buf(),
            preferred_session: None,
            include_subagents: true,
            include_subagent_user: false,
            subagent_tag: std::env::var("CCBR_CLAUDE_SUBAGENT_TAG")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        }
    }

    pub fn from_session(session: &super::session::ClaudeProjectSession) -> Self {
        let root = session.claude_projects_root();
        let work_dir = session
            .data
            .get("work_dir")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                session
                    .session_file
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| {
                        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                    })
            });
        let mut reader = Self::new(root.as_deref(), &work_dir);
        if let Some(path) = session.claude_session_path() {
            reader.set_preferred_session(Some(PathBuf::from(path)));
        }
        reader
    }

    pub fn set_preferred_session(&mut self, session_path: Option<PathBuf>) {
        self.preferred_session = session_path.as_deref().map(expand_tilde_path);
    }

    pub fn current_session_path(&self) -> Option<PathBuf> {
        self.latest_session()
    }

    /// Capture reader state for an initial (tail) read.
    pub fn capture_state(&self) -> HashMap<String, Value> {
        let session = self.latest_session();
        let mut state = HashMap::new();
        state.insert(
            "session_path".to_string(),
            session
                .as_ref()
                .map(|p| Value::String(p.to_string_lossy().to_string()))
                .unwrap_or(Value::Null),
        );
        let offset = session
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .unwrap_or(0);
        state.insert("offset".to_string(), Value::Number(offset.into()));
        state.insert("carry".to_string(), Value::String(String::new()));
        if self.include_subagents {
            if let Some(session) = &session {
                state.insert(
                    "subagents".to_string(),
                    Value::Object(
                        subagent_state_for_session(session, true)
                            .into_iter()
                            .collect(),
                    ),
                );
            }
        }
        state
    }

    /// Non-blocking attempt to read new structured entries since `state`.
    pub fn try_get_entries(
        &self,
        state: &HashMap<String, Value>,
    ) -> (Vec<ClaudeLogEntry>, HashMap<String, Value>) {
        let session = self.latest_session();
        let Some(session) = session else {
            return (Vec::new(), state.clone());
        };

        let mut current_state = reset_state_for_session(state, &session);
        let (entries, new_state) = read_new_entries(&session, &current_state);
        let mut events: Vec<ClaudeLogEntry> = entries
            .iter()
            .filter_map(|e| parse_structured_event(e, None, None))
            .collect();
        current_state = new_state;

        if self.include_subagents {
            let (sub_events, sub_state) =
                read_new_subagent_events(&session, &current_state, self.include_subagent_user);
            for (role, text, sub_id, sub_name) in sub_events {
                events.push(ClaudeLogEntry {
                    role: role.to_string(),
                    text: self.format_subagent_text(&text, sub_id.as_deref(), sub_name.as_deref()),
                    entry_type: None,
                    subtype: None,
                    uuid: None,
                    parent_uuid: None,
                    stop_reason: None,
                    subagent_id: sub_id,
                    subagent_name: sub_name,
                });
            }
            current_state.insert("subagents".to_string(), sub_state);
        }

        (events, current_state)
    }

    fn latest_session(&self) -> Option<PathBuf> {
        if let Some(preferred) = &self.preferred_session {
            if preferred.exists() {
                return Some(preferred.clone());
            }
        }

        let project_dir = self
            .projects_root
            .join(project_key_for_path(&self.work_dir));
        if let Some(session) = scan_project_dir(&project_dir) {
            return Some(session);
        }

        if std::env::var("CLAUDE_ALLOW_ANY_PROJECT_SCAN")
            .ok()
            .map(|s| matches!(s.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
        {
            return scan_any_project(&self.projects_root);
        }
        None
    }

    fn format_subagent_text(
        &self,
        text: &str,
        subagent_id: Option<&str>,
        subagent_name: Option<&str>,
    ) -> String {
        let mut label = self.subagent_tag.clone().unwrap_or_default();
        if label.is_empty() {
            return text.to_string();
        }
        if let Some(id) = subagent_id {
            label = format!("{}:{}", label, id);
        }
        if let Some(name) = subagent_name {
            label = format!("{} {}", label, name);
        }
        format!("{}\n{}", label, text)
    }
}

fn default_projects_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".claude")
        .join("projects")
}

fn project_key_for_path(path: &Path) -> String {
    let normalized = path
        .to_string_lossy()
        .replace(['\\', '/'], "-")
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "-");
    normalized.trim_matches('-').to_string()
}

fn scan_project_dir(project_dir: &Path) -> Option<PathBuf> {
    if !project_dir.exists() {
        return None;
    }
    let Ok(entries) = std::fs::read_dir(project_dir) else {
        return None;
    };
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().and_then(|s| s.to_str()) == Some("jsonl")
                && !p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.starts_with('.'))
                    .unwrap_or(false)
        })
        .collect();
    candidates.sort_by(|a, b| newer_path(b, a));
    candidates.into_iter().next()
}

fn scan_any_project(root: &Path) -> Option<PathBuf> {
    if !root.exists() {
        return None;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return None;
    };
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .flat_map(|e| {
            std::fs::read_dir(e.path())
                .ok()
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.extension().and_then(|s| s.to_str()) == Some("jsonl")
                        && !p
                            .file_name()
                            .and_then(|s| s.to_str())
                            .map(|s| s.starts_with('.'))
                            .unwrap_or(false)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    candidates.sort_by(|a, b| newer_path(b, a));
    candidates.into_iter().next()
}

fn newer_path(a: &Path, b: &Path) -> std::cmp::Ordering {
    let a_mtime = a.metadata().and_then(|m| m.modified()).ok();
    let b_mtime = b.metadata().and_then(|m| m.modified()).ok();
    a_mtime.cmp(&b_mtime)
}

fn reset_state_for_session(
    current_state: &HashMap<String, Value>,
    session: &Path,
) -> HashMap<String, Value> {
    if current_state
        .get("session_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .as_deref()
        == Some(session)
    {
        return current_state.clone();
    }
    let mut new_state = HashMap::new();
    new_state.insert(
        "session_path".to_string(),
        Value::String(session.to_string_lossy().to_string()),
    );
    new_state.insert("offset".to_string(), Value::Number(0.into()));
    new_state.insert("carry".to_string(), Value::String(String::new()));
    if let Some(sub) = current_state.get("subagents") {
        new_state.insert("subagents".to_string(), sub.clone());
    }
    new_state
}

fn read_new_entries(
    session: &Path,
    state: &HashMap<String, Value>,
) -> (Vec<HashMap<String, Value>>, HashMap<String, Value>) {
    let offset = state.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let carry = state
        .get("carry")
        .and_then(Value::as_str)
        .map(|s| s.as_bytes().to_vec())
        .unwrap_or_default();

    let size = match session_size(session) {
        Some(s) => s,
        None => return (Vec::new(), state.clone()),
    };
    let (offset, carry) = normalized_reader_state(size, offset, carry);
    let data = match read_bytes(session, offset) {
        Some(d) => d,
        None => return (Vec::new(), state.clone()),
    };
    let new_offset = offset + data.len();
    let (lines, carry) = split_buffer_lines(&carry, &data);
    let entries = parse_jsonl_entries(&lines);
    let mut new_state = HashMap::new();
    new_state.insert(
        "session_path".to_string(),
        Value::String(session.to_string_lossy().to_string()),
    );
    new_state.insert(
        "offset".to_string(),
        Value::Number((new_offset as u64).into()),
    );
    new_state.insert(
        "carry".to_string(),
        Value::String(String::from_utf8_lossy(&carry).to_string()),
    );
    if let Some(subagents) = state.get("subagents") {
        new_state.insert("subagents".to_string(), subagents.clone());
    }
    (entries, new_state)
}

fn session_size(session: &Path) -> Option<usize> {
    std::fs::metadata(session).ok().map(|m| m.len() as usize)
}

fn normalized_reader_state(size: usize, offset: usize, carry: Vec<u8>) -> (usize, Vec<u8>) {
    if size < offset {
        (0, Vec::new())
    } else {
        (offset, carry)
    }
}

fn read_bytes(session: &Path, offset: usize) -> Option<Vec<u8>> {
    use std::io::{Read, Seek};
    let mut file = std::fs::File::open(session).ok()?;
    file.seek(std::io::SeekFrom::Start(offset as u64)).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn split_buffer_lines(carry: &[u8], data: &[u8]) -> (Vec<Vec<u8>>, Vec<u8>) {
    let mut buffer = carry.to_vec();
    buffer.extend_from_slice(data);
    if buffer.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut lines: Vec<_> = buffer.split(|&b| b == b'\n').map(|s| s.to_vec()).collect();
    if buffer.last() != Some(&b'\n') {
        let carry = lines.pop().unwrap_or_default();
        (lines, carry)
    } else {
        (lines, Vec::new())
    }
}

fn parse_jsonl_entries(lines: &[Vec<u8>]) -> Vec<HashMap<String, Value>> {
    lines
        .iter()
        .filter_map(|raw| {
            let text = String::from_utf8_lossy(raw);
            if text.trim().is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.as_object().cloned().map(|m| m.into_iter().collect()))
        })
        .collect()
}

fn parse_structured_event(
    entry: &HashMap<String, Value>,
    subagent_id: Option<String>,
    subagent_name: Option<String>,
) -> Option<ClaudeLogEntry> {
    let entry_type = optional_text(entry.get("type"), true);
    let subtype = optional_text(entry.get("subtype"), true);
    let uuid = optional_text(entry.get("uuid"), false);
    let parent_uuid = optional_text(entry.get("parentUuid"), false);

    if let Some(text) = extract_message(entry, "user") {
        return Some(ClaudeLogEntry {
            role: "user".to_string(),
            text,
            entry_type,
            subtype,
            uuid,
            parent_uuid,
            stop_reason: None,
            subagent_id,
            subagent_name,
        });
    }

    if let Some(text) = extract_message(entry, "assistant") {
        let stop_reason = assistant_stop_reason(entry);
        return Some(ClaudeLogEntry {
            role: "assistant".to_string(),
            text,
            entry_type,
            subtype,
            uuid,
            parent_uuid,
            stop_reason,
            subagent_id,
            subagent_name,
        });
    }

    if entry_type.as_deref() == Some("system") {
        return Some(ClaudeLogEntry {
            role: "system".to_string(),
            text: String::new(),
            entry_type,
            subtype,
            uuid,
            parent_uuid,
            stop_reason: None,
            subagent_id,
            subagent_name,
        });
    }
    None
}

fn assistant_stop_reason(entry: &HashMap<String, Value>) -> Option<String> {
    entry
        .get("message")
        .and_then(Value::as_object)
        .and_then(|m| m.get("stop_reason"))
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn extract_message(entry: &HashMap<String, Value>, role: &str) -> Option<String> {
    let entry_type = entry
        .get("type")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();

    if entry_type == "response_item" {
        if let Some(payload) = entry.get("payload").and_then(Value::as_object) {
            if payload.get("type").and_then(Value::as_str) == Some("message") {
                let msg_role = payload
                    .get("role")
                    .and_then(Value::as_str)
                    .map(|s| s.trim().to_lowercase())
                    .unwrap_or_default();
                if msg_role == role {
                    return extract_content_text(payload.get("content"));
                }
            }
        }
    }

    if entry_type == "event_msg" {
        if let Some(payload) = entry.get("payload").and_then(Value::as_object) {
            let payload_type = payload
                .get("type")
                .and_then(Value::as_str)
                .map(|s| s.trim().to_lowercase())
                .unwrap_or_default();
            if ["agent_message", "assistant_message", "assistant"].contains(&payload_type.as_str())
            {
                let msg_role = payload
                    .get("role")
                    .and_then(Value::as_str)
                    .map(|s| s.trim().to_lowercase())
                    .unwrap_or_default();
                if msg_role == role {
                    for key in ["message", "content", "text"] {
                        if let Some(text) = payload.get(key).and_then(Value::as_str) {
                            let trimmed = text.trim();
                            if !trimmed.is_empty() {
                                return Some(trimmed.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(top_role) = entry.get("role").and_then(Value::as_str) {
        if top_role.trim().to_lowercase() == role {
            return extract_content_text(entry.get("content"));
        }
    }

    if let Some(message) = entry.get("message").and_then(Value::as_object) {
        let msg_role = message
            .get("role")
            .and_then(Value::as_str)
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_else(|| entry_type.clone());
        if msg_role == role {
            return extract_content_text(message.get("content"));
        }
    }

    if entry_type == role {
        return extract_content_text(entry.get("content"));
    }

    None
}

fn extract_content_text(content: Option<&Value>) -> Option<String> {
    let content = content?;
    if let Some(text) = content.as_str() {
        let trimmed = text.trim();
        return if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    let array = content.as_array()?;
    let mut texts = Vec::new();
    for item in array {
        if let Some(text) = extract_text_fragment(item) {
            texts.push(text);
        }
    }
    if texts.is_empty() {
        return None;
    }
    Some(texts.join("\n").trim().to_string())
}

fn extract_text_fragment(item: &Value) -> Option<String> {
    let obj = item.as_object()?;
    let item_type = obj
        .get("type")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    if item_type == "thinking" || item_type == "thinking_delta" {
        return None;
    }
    let text = obj
        .get("text")
        .or_else(|| {
            if item_type == "text" {
                obj.get("content")
            } else {
                None
            }
        })
        .and_then(Value::as_str)?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn optional_text(value: Option<&Value>, lowercase: bool) -> Option<String> {
    let text = value.and_then(Value::as_str).map(|s| s.trim())?;
    if text.is_empty() {
        return None;
    }
    Some(if lowercase {
        text.to_lowercase()
    } else {
        text.to_string()
    })
}

fn subagent_state_for_session(session: &Path, start_from_end: bool) -> HashMap<String, Value> {
    let mut state = HashMap::new();
    for log_path in list_subagent_logs(session) {
        let key = log_path.to_string_lossy().to_string();
        let offset = if start_from_end {
            std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        let mut entry = serde_json::Map::new();
        entry.insert("offset".to_string(), Value::Number(offset.into()));
        entry.insert("carry".to_string(), Value::String(String::new()));
        state.insert(key, Value::Object(entry));
    }
    state
}

fn list_subagent_logs(session: &Path) -> Vec<PathBuf> {
    let session_dir = session.with_extension("");
    let sub_dir = session_dir.join("subagents");
    if !sub_dir.exists() {
        return Vec::new();
    }
    let mut logs: Vec<PathBuf> = std::fs::read_dir(&sub_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("jsonl"))
        .collect();
    logs.sort();
    logs
}

fn read_new_subagent_events(
    session: &Path,
    state: &HashMap<String, Value>,
    include_user: bool,
) -> SubagentEventTuple {
    let sub_state = state
        .get("subagents")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let logs = list_subagent_logs(session);
    let mut new_state = serde_json::Map::new();
    let mut events = Vec::new();

    for log_path in logs {
        let key = log_path.to_string_lossy().to_string();
        let (offset, carry) =
            sub_state
                .get(&key)
                .and_then(Value::as_object)
                .map_or((0, Vec::new()), |obj| {
                    let offset = obj.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
                    let carry = obj
                        .get("carry")
                        .and_then(Value::as_str)
                        .map(|s| s.as_bytes().to_vec())
                        .unwrap_or_default();
                    (offset, carry)
                });

        let (entries, updated) = read_new_entries(&log_path, &sub_state_entry(offset, carry));
        for entry in entries {
            if let Some(user_msg) = extract_message(&entry, "user") {
                if include_user {
                    events.push((
                        "user".to_string(),
                        user_msg,
                        subagent_id(&entry),
                        subagent_name(&entry),
                    ));
                }
                continue;
            }
            if let Some(assistant_msg) = extract_message(&entry, "assistant") {
                events.push((
                    "assistant".to_string(),
                    assistant_msg,
                    subagent_id(&entry),
                    subagent_name(&entry),
                ));
            }
        }
        new_state.insert(key, updated_to_value(updated));
    }

    (events, Value::Object(new_state))
}

fn subagent_id(entry: &HashMap<String, Value>) -> Option<String> {
    entry
        .get("agentId")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn subagent_name(entry: &HashMap<String, Value>) -> Option<String> {
    entry
        .get("slug")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn sub_state_entry(offset: usize, carry: Vec<u8>) -> HashMap<String, Value> {
    let mut state = HashMap::new();
    state.insert("offset".to_string(), Value::Number((offset as u64).into()));
    state.insert(
        "carry".to_string(),
        Value::String(String::from_utf8_lossy(&carry).to_string()),
    );
    state
}

fn updated_to_value(updated: HashMap<String, Value>) -> Value {
    let mut obj = serde_json::Map::new();
    if let Some(offset) = updated.get("offset") {
        obj.insert("offset".to_string(), offset.clone());
    }
    if let Some(carry) = updated.get("carry") {
        obj.insert("carry".to_string(), carry.clone());
    }
    Value::Object(obj)
}

fn expand_tilde_path(path: &Path) -> PathBuf {
    if let Some(s) = path.to_str() {
        PathBuf::from(expand_tilde(s))
    } else {
        path.to_path_buf()
    }
}

fn expand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    input.to_string()
}

mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use tempfile::TempDir;

    fn projects_root(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join(".claude").join("projects")
    }

    fn make_project_dir(root: &Path, work_dir: &Path) -> PathBuf {
        root.join(project_key_for_path(work_dir))
    }

    #[test]
    fn test_capture_state_starts_at_end() {
        let dir = TempDir::new().unwrap();
        let work_dir = dir.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        let root = projects_root(&dir);
        let project_dir = make_project_dir(&root, &work_dir);
        std::fs::create_dir_all(&project_dir).unwrap();
        let session = project_dir.join("session.jsonl");
        let mut file = std::fs::File::create(&session).unwrap();
        writeln!(file, r#"{{"type":"message","role":"user","content":"hi"}}"#).unwrap();

        let reader = ClaudeLogReader::new(Some(&root), &work_dir);
        let state = reader.capture_state();
        assert_eq!(
            state.get("session_path").unwrap().as_str().unwrap(),
            session.to_string_lossy()
        );
        assert!(state.get("offset").unwrap().as_u64().unwrap() > 0);
    }

    #[test]
    fn test_try_get_entries_reads_new_events() {
        let dir = TempDir::new().unwrap();
        let work_dir = dir.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        let root = projects_root(&dir);
        let project_dir = make_project_dir(&root, &work_dir);
        std::fs::create_dir_all(&project_dir).unwrap();
        let session = project_dir.join("session.jsonl");

        let reader = ClaudeLogReader::new(Some(&root), &work_dir);
        let state = reader.capture_state();

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&session)
            .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"user","content":"hello"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"assistant","content":"world"}}}}"#
        )
        .unwrap();
        drop(file);

        let (entries, new_state) = reader.try_get_entries(&state);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].role, "user");
        assert_eq!(entries[0].text, "hello");
        assert_eq!(entries[1].role, "assistant");
        assert_eq!(entries[1].text, "world");
        assert!(
            new_state.get("offset").unwrap().as_u64().unwrap()
                > state.get("offset").unwrap().as_u64().unwrap()
        );
    }

    #[test]
    fn test_subagent_logs_are_read() {
        let dir = TempDir::new().unwrap();
        let work_dir = dir.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        let root = projects_root(&dir);
        let project_dir = make_project_dir(&root, &work_dir);
        std::fs::create_dir_all(&project_dir).unwrap();
        let session = project_dir.join("session.jsonl");
        std::fs::File::create(&session).unwrap();
        let sub_dir = session.with_extension("").join("subagents");
        std::fs::create_dir_all(&sub_dir).unwrap();
        let sub_log = sub_dir.join("sub1.jsonl");
        {
            let mut file = std::fs::File::create(&sub_log).unwrap();
            writeln!(file, r#"{{"type":"message","role":"assistant","content":"old","agentId":"sub-1","slug":"helper"}}"#).unwrap();
        }

        let reader = ClaudeLogReader::new(Some(&root), &work_dir);
        let state = reader.capture_state();

        {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&sub_log)
                .unwrap();
            writeln!(file, r#"{{"type":"message","role":"assistant","content":"sub reply","agentId":"sub-1","slug":"helper"}}"#).unwrap();
        }

        let (entries, _new_state) = reader.try_get_entries(&state);
        let assistant_entries: Vec<_> = entries
            .into_iter()
            .filter(|e| e.role == "assistant")
            .collect();
        assert_eq!(assistant_entries.len(), 1);
        assert_eq!(assistant_entries[0].subagent_id.as_deref(), Some("sub-1"));
        assert_eq!(
            assistant_entries[0].subagent_name.as_deref(),
            Some("helper")
        );
    }
}
