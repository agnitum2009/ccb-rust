use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::paths::{default_sessions_root, expand_tilde_path};
use super::session::DroidProjectSession;

/// A parsed log event: role and text content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEvent {
    User(String),
    Assistant(String),
}

impl LogEvent {
    pub fn role(&self) -> &str {
        match self {
            LogEvent::User(_) => "user",
            LogEvent::Assistant(_) => "assistant",
        }
    }

    pub fn text(&self) -> &str {
        match self {
            LogEvent::User(t) | LogEvent::Assistant(t) => t,
        }
    }
}

/// Reads Droid session logs from a sessions root.
///
/// Mirrors Python `provider_backends.droid.comm_runtime.log_reader.DroidLogReader`.
#[derive(Debug, Clone)]
pub struct DroidLogReader {
    root: PathBuf,
    work_dir: PathBuf,
    preferred_session: Option<PathBuf>,
    session_id_hint: Option<String>,
    scan_limit: usize,
}

impl DroidLogReader {
    pub fn new(root: Option<&Path>, work_dir: Option<&Path>) -> Self {
        Self {
            root: root
                .map(expand_tilde_path)
                .unwrap_or_else(default_sessions_root),
            work_dir: work_dir
                .map(expand_tilde_path)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            preferred_session: None,
            session_id_hint: None,
            scan_limit: std::env::var("DROID_SESSION_SCAN_LIMIT")
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(200)
                .max(1),
        }
    }

    pub fn from_session(session: &DroidProjectSession) -> Self {
        let root = session_root_from_session(session);
        let work_dir = session
            .data
            .get("work_dir")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                session
                    .session_file
                    .parent()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| {
                        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                    })
            });
        let mut reader = Self::new(root.as_deref(), Some(&work_dir));
        if let Some(path) = session.droid_session_path() {
            reader.set_preferred_session(Some(PathBuf::from(path)));
        }
        if let Some(id) = session.droid_session_id() {
            reader.set_session_id_hint(Some(id.to_string()));
        }
        reader
    }

    pub fn set_preferred_session(&mut self, session_path: Option<PathBuf>) {
        self.preferred_session = session_path.as_deref().map(expand_tilde_path);
    }

    pub fn set_session_id_hint(&mut self, session_id: Option<String>) {
        self.session_id_hint = session_id;
    }

    pub fn current_session_path(&self) -> Option<PathBuf> {
        self.latest_session()
    }

    /// Capture the current reader state.
    pub fn capture_state(&self) -> HashMap<String, Value> {
        let mut state = HashMap::new();
        state.insert(
            "session_path".to_string(),
            self.current_session_path()
                .map(|p| Value::String(p.to_string_lossy().to_string()))
                .unwrap_or(Value::Null),
        );
        state.insert("offset".to_string(), Value::Number(0.into()));
        state.insert("carry".to_string(), Value::String(String::new()));
        state
    }

    /// Try to read new events since the last captured state (non-blocking).
    pub fn try_get_events(
        &self,
        state: &HashMap<String, Value>,
    ) -> (Vec<LogEvent>, HashMap<String, Value>) {
        self.read_since_events(state, 0.0, false)
    }

    /// Read events since `state`, optionally blocking up to `timeout` seconds.
    pub fn read_since_events(
        &self,
        state: &HashMap<String, Value>,
        timeout: f64,
        block: bool,
    ) -> (Vec<LogEvent>, HashMap<String, Value>) {
        let deadline = if block {
            std::time::Instant::now() + std::time::Duration::from_secs_f64(timeout.max(0.0))
        } else {
            std::time::Instant::now()
        };
        let mut current_state = state.clone();
        loop {
            let session = self.latest_session();
            let Some(session) = session else {
                if !block || std::time::Instant::now() >= deadline {
                    return (Vec::new(), current_state);
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
                continue;
            };
            current_state = reset_state_for_session(&current_state, &session);
            let (events, new_state) = read_new_events(&session, &current_state);
            if !events.is_empty() {
                return (events, new_state);
            }
            if !block || std::time::Instant::now() >= deadline {
                return (Vec::new(), new_state);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    fn latest_session(&self) -> Option<PathBuf> {
        let preferred = self
            .preferred_session
            .as_ref()
            .and_then(|p| p.exists().then_some(p.clone()));
        let scanned = self.scan_latest_session();

        if let Some(preferred) = &preferred {
            if let Some(scanned) = scanned {
                if newer_path(&scanned, preferred) {
                    return Some(scanned);
                }
            }
            return Some(preferred.clone());
        }

        if let Some(hint) = &self.session_id_hint {
            if let Some(by_id) = self.find_session_by_id(hint) {
                return Some(by_id);
            }
        }

        if let Some(scanned) = scanned {
            return Some(scanned);
        }

        if std::env::var("DROID_ALLOW_ANY_PROJECT_SCAN")
            .ok()
            .map(|s| matches!(s.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
        {
            return self.scan_latest_session_any_project();
        }
        None
    }

    fn scan_latest_session(&self) -> Option<PathBuf> {
        self.scan_latest_matching(Some(&self.work_dir))
    }

    fn scan_latest_session_any_project(&self) -> Option<PathBuf> {
        self.scan_latest_matching(None)
    }

    fn scan_latest_matching(&self, work_dir: Option<&Path>) -> Option<PathBuf> {
        if !self.root.exists() {
            return None;
        }
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return None;
        };
        let mut candidates: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("jsonl"))
            .filter(|p| {
                work_dir.is_none_or(|wd| {
                    let content = std::fs::read_to_string(p).unwrap_or_default();
                    content.contains(&format!("\"work_dir\":\"{}\"", wd.to_string_lossy()))
                        || content.contains(&format!("\"work_dir\": \"{}\"", wd.to_string_lossy()))
                })
            })
            .collect();
        candidates.sort_by(|a, b| {
            let a_mtime = a.metadata().and_then(|m| m.modified()).ok();
            let b_mtime = b.metadata().and_then(|m| m.modified()).ok();
            b_mtime.cmp(&a_mtime)
        });
        candidates.truncate(self.scan_limit);
        candidates.into_iter().next()
    }

    fn find_session_by_id(&self, session_id: &str) -> Option<PathBuf> {
        if !self.root.exists() {
            return None;
        }
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return None;
        };
        entries.flatten().map(|e| e.path()).find(|p| {
            p.is_file()
                && p.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.starts_with(session_id))
                    .unwrap_or(false)
        })
    }
}

fn session_root_from_session(session: &DroidProjectSession) -> Option<PathBuf> {
    let data = &session.data;
    if let Some(raw) = data
        .get("droid_sessions_root")
        .or_else(|| data.get("factory_sessions_root"))
        .and_then(Value::as_str)
    {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Some(expand_tilde(trimmed).into());
        }
    }
    if let Some(raw) = data
        .get("droid_home")
        .or_else(|| data.get("factory_home"))
        .and_then(Value::as_str)
    {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(expand_tilde(trimmed)).join("sessions"));
        }
    }
    None
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
    new_state
}

fn read_new_events(
    session: &Path,
    state: &HashMap<String, Value>,
) -> (Vec<LogEvent>, HashMap<String, Value>) {
    let (entries, new_state) = read_new_entries(session, state);
    let events: Vec<LogEvent> = entries.iter().filter_map(entry_event).collect();
    (events, new_state)
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
    let size = session_size(session);
    let size = match size {
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

fn entry_event(entry: &HashMap<String, Value>) -> Option<LogEvent> {
    if let Some(user_msg) = extract_message(entry, "user") {
        return Some(LogEvent::User(user_msg));
    }
    if let Some(assistant_msg) = extract_message(entry, "assistant") {
        return Some(LogEvent::Assistant(assistant_msg));
    }
    None
}

fn extract_message(entry: &HashMap<String, Value>, role: &str) -> Option<String> {
    let entry_type = entry
        .get("type")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    if entry_type == "message" {
        if let Some(message) = entry.get("message").and_then(Value::as_object) {
            let msg_role = message
                .get("role")
                .and_then(Value::as_str)
                .map(|s| s.trim().to_lowercase())
                .unwrap_or_default();
            if msg_role == role {
                return extract_content_text(message.get("content"));
            }
        }
    }
    let msg_role = entry
        .get("role")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_else(|| entry_type);
    if msg_role == role {
        return extract_content_text(entry.get("content").or_else(|| entry.get("message")));
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

fn newer_path(a: &Path, b: &Path) -> bool {
    let a_mtime = a.metadata().and_then(|m| m.modified()).ok();
    let b_mtime = b.metadata().and_then(|m| m.modified()).ok();
    match (a_mtime, b_mtime) {
        (Some(a), Some(b)) => a > b,
        (Some(_), None) => true,
        _ => false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_extract_message_text() {
        let entry = serde_json::json!({
            "type": "message",
            "message": {"role": "assistant", "content": "hello"}
        });
        let map: HashMap<String, Value> = entry.as_object().unwrap().clone().into_iter().collect();
        assert_eq!(
            extract_message(&map, "assistant"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn test_extract_message_parts() {
        let entry = serde_json::json!({
            "role": "assistant",
            "content": [
                {"type": "text", "text": "hello "},
                {"type": "text", "text": "world"}
            ]
        });
        let map: HashMap<String, Value> = entry.as_object().unwrap().clone().into_iter().collect();
        assert_eq!(
            extract_message(&map, "assistant"),
            Some("hello\nworld".to_string())
        );
    }

    #[test]
    fn test_read_new_entries() {
        let dir = TempDir::new().unwrap();
        let work_dir = dir.path().to_string_lossy();
        let session = dir.path().join("session.jsonl");
        let mut file = std::fs::File::create(&session).unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"user","content":"hi"}},"work_dir":"{}"}}"#,
            work_dir
        )
        .unwrap();
        writeln!(file, r#"{{"type":"message","message":{{"role":"assistant","content":"hello"}},"work_dir":"{}"}}"#, work_dir).unwrap();

        let reader = DroidLogReader::new(Some(dir.path()), Some(dir.path()));
        let state = reader.capture_state();
        let (events, _new_state) = reader.try_get_events(&state);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].role(), "user");
        assert_eq!(events[1].role(), "assistant");
    }

    #[test]
    fn test_read_new_entries_respects_offset() {
        let dir = TempDir::new().unwrap();
        let session = dir.path().join("session.jsonl");
        let mut file = std::fs::File::create(&session).unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"user","content":"first"}}}}"#
        )
        .unwrap();

        let mut state = HashMap::new();
        state.insert(
            "session_path".to_string(),
            Value::String(session.to_string_lossy().to_string()),
        );
        state.insert("offset".to_string(), Value::Number(0.into()));
        state.insert("carry".to_string(), Value::String(String::new()));

        let (events, new_state) = read_new_events(&session, &state);
        assert_eq!(events.len(), 1);
        assert!(new_state["offset"].as_u64().unwrap() > 0);

        let (events2, _) = read_new_events(&session, &new_state);
        assert!(events2.is_empty());
    }
}
