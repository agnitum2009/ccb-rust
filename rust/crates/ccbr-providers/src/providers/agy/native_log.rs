use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use ccbr_provider_core::source_home::current_provider_source_home;
use serde_json::{Map, Value};

use crate::native_cli_support::clean_native_reply;

/// Observation of an AGY native transcript log for a single request.
///
/// Mirrors Python `provider_backends.agy.native_log.AgyTranscriptObservation`.
#[derive(Debug, Clone)]
pub struct AgyTranscriptObservation {
    pub request_seen: bool,
    pub completed: bool,
    pub reply: String,
    pub conversation_id: Option<String>,
    pub transcript_path: Option<PathBuf>,
    pub provider_turn_ref: Option<String>,
    pub line_count: usize,
    pub native_started_at: Option<String>,
    pub native_completed_at: Option<String>,
    pub latest_status: Option<String>,
}

/// Observe the AGY native transcript logs for a request.
///
/// Mirrors Python `provider_backends.agy.native_log.observe_agy_transcript`.
pub fn observe_agy_transcript(
    work_dir: &Path,
    req_id: &str,
    home_candidates: Option<&[PathBuf]>,
) -> Option<AgyTranscriptObservation> {
    let _ = work_dir;
    if req_id.is_empty() {
        return None;
    }
    let mut observations: Vec<AgyTranscriptObservation> = Vec::new();
    for transcript in transcript_paths(home_candidates) {
        if let Some(observed) = observe_transcript(&transcript, req_id) {
            observations.push(observed);
        }
    }
    if observations.is_empty() {
        return None;
    }
    let completed: Vec<_> = observations
        .iter()
        .filter(|o| o.completed)
        .cloned()
        .collect();
    if !completed.is_empty() {
        return Some(
            completed
                .into_iter()
                .max_by(|a, b| observation_sort_key(a).cmp(&observation_sort_key(b)))
                .unwrap(),
        );
    }
    Some(
        observations
            .into_iter()
            .max_by(|a, b| observation_sort_key(a).cmp(&observation_sort_key(b)))
            .unwrap(),
    )
}

/// Extract an explicit AGY home from a start command string.
///
/// Mirrors Python `provider_backends.agy.native_log.agy_home_from_start_cmd`.
pub fn agy_home_from_start_cmd(start_cmd: &str) -> Option<PathBuf> {
    if start_cmd.is_empty() || !start_cmd.contains("HOME=") {
        return None;
    }
    let prefix = start_cmd.split(';').next().unwrap_or("").trim();
    let prefix = prefix.strip_prefix("export ").unwrap_or(prefix);
    for part in prefix.split_whitespace() {
        if let Some(value) = part.strip_prefix("HOME=") {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(PathBuf::from(expand_tilde(value)));
            }
        }
    }
    None
}

fn transcript_paths(home_candidates: Option<&[PathBuf]>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for home in candidate_homes(home_candidates) {
        for root in brain_roots(&home) {
            if !root.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
                let logs = entry.path().join(".system_generated").join("logs");
                if !logs.is_dir() {
                    continue;
                }
                for log_entry in std::fs::read_dir(&logs).into_iter().flatten().flatten() {
                    let path = log_entry.path();
                    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if path.is_file()
                        && !name.starts_with('.')
                        && name.starts_with("transcript")
                        && name.ends_with(".jsonl")
                    {
                        let resolved = path.canonical_or_self();
                        if seen.iter().any(|s| s == &resolved) {
                            continue;
                        }
                        seen.push(resolved);
                        paths.push(path);
                    }
                }
            }
        }
    }
    paths.sort_by_key(|a| path_mtime(a));
    paths
}

fn candidate_homes(home_candidates: Option<&[PathBuf]>) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(explicit) =
        std::env::var("AGY_HOME").or_else(|_| std::env::var("CCB_AGY_SOURCE_HOME"))
    {
        let explicit = explicit.trim();
        if !explicit.is_empty() {
            candidates.push(PathBuf::from(expand_tilde(explicit)));
        }
    }
    if let Some(list) = home_candidates {
        for item in list {
            candidates.push(item.expand_home());
        }
    }
    candidates.push(current_provider_source_home());
    candidates.push(dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")));

    let mut unique: Vec<PathBuf> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for candidate in candidates {
        let key = candidate.to_string_lossy().to_string();
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        unique.push(candidate);
    }
    unique
}

fn brain_roots(home: &Path) -> Vec<PathBuf> {
    match home.file_name().and_then(|s| s.to_str()) {
        Some("brain") => vec![home.to_path_buf()],
        Some("antigravity-cli") => vec![home.join("brain")],
        Some(".gemini") => vec![home.join("antigravity-cli").join("brain")],
        _ => vec![
            home.join(".gemini").join("antigravity-cli").join("brain"),
            home.join(".antigravity").join("brain"),
        ],
    }
}

fn observe_transcript(path: &Path, req_id: &str) -> Option<AgyTranscriptObservation> {
    let raw = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = raw.lines().collect();

    let mut active = false;
    let mut request_line = 0usize;
    let mut latest_reply = String::new();
    let mut latest_status: Option<String> = None;
    let mut started_at: Option<String> = None;
    let mut completed_at: Option<String> = None;
    let mut provider_turn_ref: Option<String> = None;
    let mut latest_line = 0usize;

    for (index, line) in lines.iter().enumerate() {
        let index = index + 1;
        let event: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let event = match event.as_object() {
            Some(o) => o,
            None => continue,
        };

        if is_user_input(event) {
            let content = event_content(event);
            if content.contains(req_id) {
                active = true;
                request_line = index;
                latest_reply.clear();
                latest_status = event_status(event);
                started_at = event
                    .get("created_at")
                    .or_else(|| event.get("timestamp"))
                    .and_then(Value::as_str)
                    .map(|s| s.to_string());
                completed_at = None;
                provider_turn_ref = event_ref(event);
                latest_line = index;
            } else if active {
                active = false;
            }
            continue;
        }

        if !active {
            continue;
        }

        if let Some(status) = event_status(event) {
            latest_status = Some(status);
        }

        if !is_model_reply_event(event) {
            continue;
        }
        let reply = clean_native_reply(&event_content(event), req_id);
        if reply.is_empty() {
            continue;
        }
        latest_reply = reply;
        completed_at = event
            .get("created_at")
            .or_else(|| event.get("timestamp"))
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        provider_turn_ref = event_ref(event).or(provider_turn_ref.clone());
        latest_line = index;
    }

    if request_line == 0 {
        return None;
    }

    Some(AgyTranscriptObservation {
        request_seen: true,
        completed: !latest_reply.is_empty(),
        reply: latest_reply,
        conversation_id: conversation_id(path),
        transcript_path: Some(path.to_path_buf()),
        provider_turn_ref: provider_turn_ref.or_else(|| conversation_id(path)),
        line_count: latest_line.max(request_line),
        native_started_at: started_at,
        native_completed_at: completed_at,
        latest_status,
    })
}

fn is_user_input(event: &Map<String, Value>) -> bool {
    let source = event.get("source").and_then(Value::as_str).unwrap_or("");
    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");
    source.to_uppercase().starts_with("USER") && event_type.to_uppercase().contains("USER_INPUT")
}

fn is_model_reply_event(event: &Map<String, Value>) -> bool {
    let source = event.get("source").and_then(Value::as_str).unwrap_or("");
    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");
    if !source.to_uppercase().starts_with("MODEL") {
        return false;
    }
    if let Some(status) = event_status(event) {
        if status != "DONE" {
            return false;
        }
    }
    let et = event_type.to_uppercase();
    matches!(
        et.as_str(),
        "PLANNER_RESPONSE"
            | "MODEL_RESPONSE"
            | "ASSISTANT_RESPONSE"
            | "FINAL_RESPONSE"
            | "RESPONSE"
    ) || et.ends_with("_RESPONSE")
}

fn event_content(event: &Map<String, Value>) -> String {
    let content = event.get("content");
    if let Some(s) = content.and_then(Value::as_str) {
        return s.to_string();
    }
    if let Some(arr) = content.and_then(Value::as_array) {
        let mut parts = Vec::new();
        for item in arr {
            if let Some(s) = item.as_str() {
                parts.push(s.to_string());
            } else if let Some(obj) = item.as_object() {
                let value = obj
                    .get("text")
                    .or_else(|| obj.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                parts.push(value.to_string());
            }
        }
        return parts.join("\n");
    }
    if let Some(obj) = content.and_then(Value::as_object) {
        let value = obj
            .get("text")
            .or_else(|| obj.get("content"))
            .and_then(Value::as_str);
        if let Some(s) = value {
            return s.to_string();
        }
        return serde_json::to_string(obj).unwrap_or_default();
    }
    if let Some(s) = event.get("text").and_then(Value::as_str) {
        return s.to_string();
    }
    String::new()
}

fn event_status(event: &Map<String, Value>) -> Option<String> {
    let text = event
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_uppercase();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn event_ref(event: &Map<String, Value>) -> Option<String> {
    for key in ["id", "message_id", "step_id", "step_index"] {
        if let Some(value) = event.get(key) {
            let text = value.as_str().unwrap_or("").trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn conversation_id(path: &Path) -> Option<String> {
    let parts: Vec<&str> = path.iter().map(|c| c.to_str().unwrap_or("")).collect();
    if let Some(index) = parts.iter().position(|p| *p == "brain") {
        if index + 1 < parts.len() {
            return Some(parts[index + 1].to_string());
        }
        return None;
    }
    if path.parent().and_then(|p| p.parent()).is_some() {
        return path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
    }
    None
}

fn observation_sort_key(observation: &AgyTranscriptObservation) -> (u64, usize) {
    let mtime = observation
        .transcript_path
        .as_ref()
        .map(|p| path_mtime(p))
        .unwrap_or(0);
    (mtime, observation.line_count)
}

fn path_mtime(path: &Path) -> u64 {
    path.metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

trait PathExt {
    fn expand_home(&self) -> PathBuf;
    fn canonical_or_self(&self) -> PathBuf;
}

impl PathExt for Path {
    fn expand_home(&self) -> PathBuf {
        let s = self.to_string_lossy();
        PathBuf::from(expand_tilde(&s))
    }

    fn canonical_or_self(&self) -> PathBuf {
        std::fs::canonicalize(self).unwrap_or_else(|_| self.to_path_buf())
    }
}

fn expand_tilde(input: impl AsRef<str>) -> String {
    let input = input.as_ref();
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
    use std::io::Write;
    use tempfile::TempDir;

    fn write_lines(path: &Path, lines: &[&str]) {
        let mut file = std::fs::File::create(path).unwrap();
        for line in lines {
            writeln!(file, "{}", line).unwrap();
        }
    }

    #[test]
    fn test_agy_home_from_start_cmd() {
        let cmd = "export HOME=/tmp/agy_home; agy";
        assert_eq!(
            agy_home_from_start_cmd(cmd),
            Some(PathBuf::from("/tmp/agy_home"))
        );
        assert!(agy_home_from_start_cmd("agy").is_none());
    }

    #[test]
    fn test_observe_transcript_detects_model_reply() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("transcript.jsonl");
        write_lines(
            &path,
            &[
                r#"{"source":"USER","type":"USER_INPUT","content":"CCB_REQ_ID: req-123"}"#,
                r#"{"source":"MODEL","type":"MODEL_RESPONSE","content":"hello agy"}"#,
            ],
        );

        let obs = observe_transcript(&path, "req-123").unwrap();
        assert!(obs.request_seen);
        assert!(obs.completed);
        assert_eq!(obs.reply, "hello agy");
    }

    #[test]
    fn test_observe_transcript_no_match_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("transcript.jsonl");
        write_lines(
            &path,
            &[r#"{"source":"USER","type":"USER_INPUT","content":"hello"}"#],
        );
        assert!(observe_transcript(&path, "req-123").is_none());
    }

    #[test]
    fn test_observe_agy_transcript_searches_brain() {
        let tmp = TempDir::new().unwrap();
        let brain_root = tmp.path().join("brain");
        let conv = brain_root.join("conv1");
        let logs = conv.join(".system_generated").join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        let path = logs.join("transcript.jsonl");
        write_lines(
            &path,
            &[
                r#"{"source":"USER","type":"USER_INPUT","content":"CCB_REQ_ID: req-abc"}"#,
                r#"{"source":"MODEL","type":"MODEL_RESPONSE","content":"reply"}"#,
            ],
        );

        let obs = observe_agy_transcript(tmp.path(), "req-abc", Some(&[brain_root])).unwrap();
        assert!(obs.completed);
        assert_eq!(obs.reply, "reply");
        assert_eq!(obs.conversation_id, Some("conv1".to_string()));
    }
}
