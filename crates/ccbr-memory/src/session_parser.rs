use crate::types::{
    ConversationEntry, MemoryError, Result, SessionInfo, SessionStats, ToolExecution,
};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Default Claude projects root.
pub fn claude_projects_root() -> PathBuf {
    std::env::var("CLAUDE_PROJECTS_ROOT")
        .or_else(|_| std::env::var("CLAUDE_PROJECT_ROOT"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/tmp"))
                .join(".claude")
                .join("projects")
        })
}

/// Parse Claude JSONL session files and extract conversations.
#[derive(Debug, Clone)]
pub struct ClaudeSessionParser {
    pub root: PathBuf,
}

impl Default for ClaudeSessionParser {
    fn default() -> Self {
        Self::new(claude_projects_root())
    }
}

impl ClaudeSessionParser {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolve a session file from work_dir and optional explicit path.
    pub fn resolve_session(&self, work_dir: &Path, session_path: Option<&Path>) -> Result<PathBuf> {
        if let Some(path) = session_path {
            if path.exists() {
                return Ok(path.to_path_buf());
            }
        }

        if let Some(resolved) = self.resolve_known_session(work_dir) {
            return Ok(resolved);
        }

        if allow_any_project_scan() {
            if let Some(any_session) = self.scan_all_projects() {
                return Ok(any_session);
            }
        }

        Err(MemoryError::SessionNotFound(format!(
            "No session found for {}",
            work_dir.display()
        )))
    }

    /// Parse a session file into conversation entries.
    pub fn parse_session(&self, session_path: &Path) -> Result<Vec<ConversationEntry>> {
        if !session_path.exists() {
            return Err(MemoryError::SessionNotFound(format!(
                "Session file not found: {}",
                session_path.display()
            )));
        }

        let content = std::fs::read_to_string(session_path)
            .map_err(|e| MemoryError::SessionParse(format!("Failed to read session file: {e}")))?;

        let mut entries = Vec::new();
        let mut errors = 0usize;
        let mut total = 0usize;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            total += 1;
            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(obj) => {
                    if let Some(entry) = parse_entry(&obj) {
                        entries.push(entry);
                    }
                }
                Err(_) => {
                    errors += 1;
                }
            }
        }

        if total > 0 && errors * 2 > total {
            return Err(MemoryError::SessionParse(format!(
                "Too many parse errors: {errors}/{total} lines failed"
            )));
        }

        Ok(entries)
    }

    /// Get information about a session file.
    pub fn get_session_info(&self, session_path: &Path) -> Result<SessionInfo> {
        let last_modified = session_path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64());
        Ok(SessionInfo {
            session_id: session_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
            session_path: session_path.to_string_lossy().to_string(),
            project_path: None,
            is_sidechain: false,
            last_modified,
            provider: None,
        })
    }

    /// Extract session statistics.
    pub fn extract_session_stats(&self, session_path: &Path) -> Result<SessionStats> {
        if !session_path.exists() {
            return Err(MemoryError::SessionNotFound(format!(
                "Session file not found: {}",
                session_path.display()
            )));
        }

        let mut stats = SessionStats::default();
        let mut seen_files: HashSet<String> = HashSet::new();
        let mut tool_uses: HashMap<String, serde_json::Map<String, serde_json::Value>> =
            HashMap::new();
        let mut tool_results: HashMap<String, serde_json::Map<String, serde_json::Value>> =
            HashMap::new();

        for obj in iter_session_objects(session_path)? {
            collect_stats(
                &obj,
                &mut stats,
                &mut seen_files,
                &mut tool_uses,
                &mut tool_results,
            );
        }

        build_tool_executions(&mut stats, &tool_uses, &tool_results);
        Ok(stats)
    }

    fn resolve_known_session(&self, work_dir: &Path) -> Option<PathBuf> {
        self.resolve_from_index(work_dir)
            .or_else(|| self.scan_project_dir(work_dir))
    }

    fn resolve_from_index(&self, work_dir: &Path) -> Option<PathBuf> {
        let index_path = self.root.join("sessions-index.json");
        if !index_path.exists() {
            return None;
        }

        let sessions = index_sessions(&index_path).ok()?;
        let candidates = index_candidates(&sessions, work_dir);
        let session_id = latest_index_session_id(&candidates)?;
        self.find_session_file(&session_id, work_dir)
    }

    fn find_session_file(&self, session_id: &str, work_dir: &Path) -> Option<PathBuf> {
        for project_dir in self.candidate_project_dirs(work_dir) {
            let candidate = project_dir.join(format!("{session_id}.jsonl"));
            if candidate.exists() {
                return Some(candidate);
            }
        }
        None
    }

    fn get_project_dir(&self, work_dir: &Path) -> Option<PathBuf> {
        let key = regex_replace_invalid(&work_dir.to_string_lossy());
        let project_dir = self.root.join(key);
        if project_dir.exists() {
            Some(project_dir)
        } else {
            None
        }
    }

    fn scan_project_dir(&self, work_dir: &Path) -> Option<PathBuf> {
        let project_dir = self.get_project_dir(work_dir)?;
        if !project_dir.exists() {
            return None;
        }
        latest_project_jsonl(&project_dir)
    }

    fn scan_all_projects(&self) -> Option<PathBuf> {
        if !self.root.exists() {
            return None;
        }

        let mut best: Option<PathBuf> = None;
        let mut best_mtime = 0.0f64;

        for entry in std::fs::read_dir(&self.root).ok()? {
            let entry = entry.ok()?;
            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }
            for jsonl_file in project_dir.read_dir().ok()? {
                let jsonl_file = jsonl_file.ok()?.path();
                if jsonl_file.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                    continue;
                }
                let mtime = jsonl_file
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0);
                if mtime > best_mtime {
                    best_mtime = mtime;
                    best = Some(jsonl_file);
                }
            }
        }

        best
    }

    fn candidate_project_dirs(&self, work_dir: &Path) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(project_dir) = self.get_project_dir(work_dir) {
            dirs.push(project_dir);
        }
        if self.root.exists() {
            for entry in std::fs::read_dir(&self.root)
                .unwrap_or_else(|_| std::fs::read_dir(".").expect("cannot read current directory"))
                .flatten()
            {
                let path = entry.path();
                if path.is_dir() && !dirs.contains(&path) {
                    dirs.push(path);
                }
            }
        }
        dirs
    }
}

fn parse_entry(obj: &serde_json::Value) -> Option<ConversationEntry> {
    let obj = obj.as_object()?;
    let msg_type = obj.get("type").and_then(|v| v.as_str())?;
    let message = obj.get("message").and_then(|v| v.as_object()).cloned();
    let message = message.as_ref()?;

    if msg_type == "user" {
        let content = extract_content(message);
        if content.is_empty() {
            return None;
        }
        return Some(ConversationEntry {
            role: "user".to_string(),
            content,
            uuid: obj.get("uuid").and_then(|v| v.as_str()).map(String::from),
            parent_uuid: obj
                .get("parentUuid")
                .and_then(|v| v.as_str())
                .map(String::from),
            timestamp: obj
                .get("timestamp")
                .and_then(|v| v.as_str())
                .map(String::from),
            tool_calls: Vec::new(),
        });
    }

    if msg_type == "assistant" {
        let content = extract_content(message);
        let tool_calls = extract_tool_calls(message);
        if content.is_empty() && tool_calls.is_empty() {
            return None;
        }
        return Some(ConversationEntry {
            role: "assistant".to_string(),
            content,
            uuid: obj.get("uuid").and_then(|v| v.as_str()).map(String::from),
            parent_uuid: obj
                .get("parentUuid")
                .and_then(|v| v.as_str())
                .map(String::from),
            timestamp: obj
                .get("timestamp")
                .and_then(|v| v.as_str())
                .map(String::from),
            tool_calls,
        });
    }

    None
}

fn extract_content(message: &serde_json::Map<String, serde_json::Value>) -> String {
    match message.get("content") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(blocks)) => {
            let texts: Vec<String> = blocks
                .iter()
                .filter_map(|block| {
                    if let Some(obj) = block.as_object() {
                        if obj.get("type").and_then(|v| v.as_str()) == Some("text") {
                            obj.get("text").and_then(|v| v.as_str()).map(String::from)
                        } else {
                            None
                        }
                    } else {
                        block.as_str().map(String::from)
                    }
                })
                .collect();
            texts.join("\n")
        }
        _ => String::new(),
    }
}

fn extract_tool_calls(
    message: &serde_json::Map<String, serde_json::Value>,
) -> Vec<serde_json::Value> {
    let content = match message.get("content") {
        Some(serde_json::Value::Array(arr)) => arr,
        _ => return Vec::new(),
    };

    content
        .iter()
        .filter_map(|block| {
            let obj = block.as_object()?;
            if obj.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                Some(serde_json::json!({
                    "name": obj.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "input": obj.get("input").cloned().unwrap_or(serde_json::json!({})),
                }))
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Session stats collection
// ---------------------------------------------------------------------------

fn collect_stats(
    obj: &serde_json::Value,
    stats: &mut SessionStats,
    seen_files: &mut HashSet<String>,
    tool_uses: &mut HashMap<String, serde_json::Map<String, serde_json::Value>>,
    tool_results: &mut HashMap<String, serde_json::Map<String, serde_json::Value>>,
) {
    let Some(obj) = obj.as_object() else { return };
    let msg_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let message = obj.get("message").and_then(|v| v.as_object());
    let content = message
        .and_then(|m| m.get("content"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    match msg_type {
        "assistant" => collect_assistant_blocks(&content, stats, seen_files, tool_uses),
        "user" => collect_tool_results(&content, tool_results),
        "file-history-snapshot" => {
            if let Some(snapshot) = obj.get("snapshot").and_then(|v| v.as_object()) {
                collect_file_snapshot(snapshot, stats, seen_files);
            }
        }
        _ => {}
    }
}

fn collect_assistant_blocks(
    content: &[serde_json::Value],
    stats: &mut SessionStats,
    seen_files: &mut HashSet<String>,
    tool_uses: &mut HashMap<String, serde_json::Map<String, serde_json::Value>>,
) {
    for block in content {
        let Some(obj) = block.as_object() else {
            continue;
        };
        if obj.get("type").and_then(|v| v.as_str()) != Some("tool_use") {
            continue;
        }
        let tool_id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let input = obj.get("input").cloned().unwrap_or(serde_json::json!({}));
        *stats.tool_calls.entry(name.clone()).or_insert(0) += 1;
        let mut tool_use = serde_json::Map::new();
        tool_use.insert("name".to_string(), serde_json::Value::String(name.clone()));
        tool_use.insert("input".to_string(), input.clone());
        tool_uses.insert(tool_id, tool_use);
        extract_file_info(&name, &input, stats, seen_files);
    }
}

fn collect_tool_results(
    content: &[serde_json::Value],
    tool_results: &mut HashMap<String, serde_json::Map<String, serde_json::Value>>,
) {
    for block in content {
        let Some(obj) = block.as_object() else {
            continue;
        };
        if obj.get("type").and_then(|v| v.as_str()) != Some("tool_result") {
            continue;
        }
        let tool_id = obj
            .get("tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let result_content = obj
            .get("content")
            .cloned()
            .unwrap_or(serde_json::Value::String(String::new()));
        let result_text = match &result_content {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let truncated = if result_text.len() > 2000 {
            format!("{}...[truncated]", &result_text[..2000])
        } else {
            result_text
        };
        let is_error = obj
            .get("is_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let mut result = serde_json::Map::new();
        result.insert("content".to_string(), serde_json::Value::String(truncated));
        result.insert("is_error".to_string(), serde_json::Value::Bool(is_error));
        tool_results.insert(tool_id, result);
    }
}

fn collect_file_snapshot(
    snapshot: &serde_json::Map<String, serde_json::Value>,
    stats: &mut SessionStats,
    seen_files: &mut HashSet<String>,
) {
    if let Some(backups) = snapshot
        .get("trackedFileBackups")
        .and_then(|v| v.as_object())
    {
        for path in backups.keys() {
            record_unique_path(path, &mut stats.files_written, seen_files);
        }
    }
}

pub(crate) fn extract_file_info(
    tool_name: &str,
    tool_input: &serde_json::Value,
    stats: &mut SessionStats,
    seen_files: &mut HashSet<String>,
) {
    let input = match tool_input.as_object() {
        Some(obj) => obj,
        None => return,
    };
    let file_path = input
        .get("file_path")
        .and_then(|v| v.as_str())
        .or_else(|| input.get("path").and_then(|v| v.as_str()));

    match tool_name {
        "Write" => {
            if let Some(path) = file_path {
                record_unique_path(path, &mut stats.files_written, seen_files);
            }
        }
        "Read" => {
            if let Some(path) = file_path {
                record_unique_path(path, &mut stats.files_read, seen_files);
            }
        }
        "Edit" => {
            if let Some(path) = file_path {
                record_unique_path(path, &mut stats.files_edited, seen_files);
            }
        }
        "Bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                record_bash_command(cmd, stats);
            }
        }
        "TaskCreate" => stats.tasks_created += 1,
        "TaskUpdate" if input.get("status").and_then(|v| v.as_str()) == Some("completed") => {
            stats.tasks_completed += 1;
        }
        _ => {}
    }
}

fn build_tool_executions(
    stats: &mut SessionStats,
    tool_uses: &HashMap<String, serde_json::Map<String, serde_json::Value>>,
    tool_results: &HashMap<String, serde_json::Map<String, serde_json::Value>>,
) {
    for (tool_id, tool_use) in tool_uses {
        let result = tool_results.get(tool_id);
        stats.tool_executions.push(ToolExecution {
            tool_id: tool_id.clone(),
            name: tool_use
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            input: tool_use
                .get("input")
                .cloned()
                .unwrap_or(serde_json::json!({})),
            result: result
                .and_then(|r| r.get("content").and_then(|v| v.as_str()).map(String::from)),
            is_error: result
                .and_then(|r| r.get("is_error").and_then(|v| v.as_bool()))
                .unwrap_or(false),
        });
    }
}

fn record_unique_path(path: &str, target: &mut Vec<String>, seen_files: &mut HashSet<String>) {
    let text = path.trim();
    if text.is_empty() || seen_files.contains(text) {
        return;
    }
    target.push(text.to_string());
    seen_files.insert(text.to_string());
}

fn record_bash_command(command: &str, stats: &mut SessionStats) {
    let cmd = command.trim();
    if cmd.is_empty() || stats.bash_commands.len() >= 20 {
        return;
    }
    let cmd = if cmd.len() > 100 {
        format!("{}...", &cmd[..100])
    } else {
        cmd.to_string()
    };
    stats.bash_commands.push(cmd);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn iter_session_objects(session_path: &Path) -> Result<impl Iterator<Item = serde_json::Value>> {
    let content = std::fs::read_to_string(session_path)
        .map_err(|e| MemoryError::SessionParse(format!("Failed to read session file: {e}")))?;

    let objects: Vec<serde_json::Value> = content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            serde_json::from_str(line).ok()
        })
        .collect();

    Ok(objects.into_iter())
}

fn allow_any_project_scan() -> bool {
    matches!(
        std::env::var("CLAUDE_ALLOW_ANY_PROJECT_SCAN").as_deref(),
        Ok("1")
    )
}

fn regex_replace_invalid(value: &str) -> String {
    let re = Regex::new(r"[^A-Za-z0-9]").expect("regex");
    re.replace_all(value, "-").to_string()
}

fn index_sessions(index_path: &Path) -> Result<Vec<serde_json::Value>> {
    let text = std::fs::read_to_string(index_path)
        .map_err(|e| MemoryError::SessionParse(format!("Failed to read index: {e}")))?;
    let data: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| MemoryError::SessionParse(format!("Failed to parse index: {e}")))?;
    let sessions = data.get("sessions").and_then(|v| v.as_array()).cloned();
    Ok(sessions.unwrap_or_default())
}

fn index_candidates(sessions: &[serde_json::Value], work_dir: &Path) -> Vec<serde_json::Value> {
    let sessions: Vec<_> = sessions
        .iter()
        .filter(|s| {
            !s.get("isSidechain")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    if sessions.is_empty() {
        return Vec::new();
    }

    let work_dir_str = match std::fs::canonicalize(work_dir) {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => work_dir.to_string_lossy().to_string(),
    };

    let matched: Vec<_> = sessions
        .iter()
        .filter(|s| project_path_matches(s, &work_dir_str))
        .cloned()
        .collect();

    if matched.is_empty() {
        sessions
    } else {
        matched
    }
}

fn project_path_matches(session: &serde_json::Value, work_dir_str: &str) -> bool {
    let project_path = session
        .get("projectPath")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    !project_path.is_empty() && work_dir_str.starts_with(project_path)
}

fn latest_index_session_id(candidates: &[serde_json::Value]) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let mut candidates: Vec<_> = candidates.to_vec();
    candidates.sort_by(|a, b| {
        let a_mtime = a
            .get("lastModified")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let b_mtime = b
            .get("lastModified")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        b_mtime
            .partial_cmp(&a_mtime)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates
        .first()
        .and_then(|s| {
            s.get("sessionId")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .filter(|s| !s.is_empty())
}

fn latest_project_jsonl(project_dir: &Path) -> Option<PathBuf> {
    let mut jsonl_files: Vec<_> = std::fs::read_dir(project_dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("jsonl"))
        .collect();
    jsonl_files.sort_by(|a, b| {
        let a_mtime = a
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let b_mtime = b
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        b_mtime
            .partial_cmp(&a_mtime)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    jsonl_files.into_iter().next()
}
