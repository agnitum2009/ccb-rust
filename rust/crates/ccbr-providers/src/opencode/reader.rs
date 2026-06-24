use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::paths::{
    compute_opencode_project_id, normalize_path_for_match, path_matches, req_id_re,
};
use super::replies::find_new_assistant_reply_with_state;
use super::storage::OpenCodeStorageAccessor;

/// Simplified OpenCode log reader that works with the existing ported opencode
/// runtime modules. It reads sessions/messages/parts from the file-backed
/// OpenCode storage layout (no SQLite support yet).
#[derive(Debug, Clone)]
pub struct OpenCodeLogReader {
    storage: OpenCodeStorageAccessor,
    work_dir: PathBuf,
    project_id: String,
    session_id_filter: Option<String>,
    allow_any_session: bool,
    _allow_session_rollover: bool,
    allow_parent_match: bool,
}

impl OpenCodeLogReader {
    pub fn new(
        root: Option<&Path>,
        work_dir: &Path,
        project_id: impl Into<String>,
        session_id_filter: Option<String>,
    ) -> Self {
        let root = root.map(PathBuf::from).unwrap_or_else(default_storage_root);
        let storage = OpenCodeStorageAccessor::new(&root);
        let work_dir = PathBuf::from(work_dir);
        let raw_project_id = project_id.into();
        let explicit_project_id = {
            let trimmed = raw_project_id.trim();
            !trimmed.is_empty() && trimmed != "global"
        };
        let env_project_id = std::env::var("OPENCODE_PROJECT_ID")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let project_id = env_project_id.unwrap_or_else(|| {
            if explicit_project_id {
                raw_project_id
            } else {
                detect_project_id_for_workdir(&storage, &work_dir)
                    .unwrap_or_else(|| compute_opencode_project_id(&work_dir))
            }
        });
        Self {
            storage,
            work_dir,
            project_id,
            session_id_filter,
            allow_any_session: env_truthy("OPENCODE_ALLOW_ANY_SESSION"),
            _allow_session_rollover: env_truthy("OPENCODE_ALLOW_SESSION_ROLLOVER"),
            allow_parent_match: env_truthy("OPENCODE_ALLOW_PARENT_WORKDIR_MATCH"),
        }
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn session_id_filter(&self) -> Option<&str> {
        self.session_id_filter.as_deref()
    }

    pub fn work_dir(&self) -> &Path {
        &self.work_dir
    }

    pub fn storage(&self) -> &OpenCodeStorageAccessor {
        &self.storage
    }

    /// Capture the current reader state.
    pub fn capture_state(&self) -> HashMap<String, Value> {
        capture_state(self)
    }

    /// Read messages for a session, preferring SQLite and falling back to JSON files.
    pub fn read_messages(&self, session_id: &str) -> Vec<Value> {
        let mut messages = read_messages_from_db(self, session_id);
        if messages.is_empty() {
            messages = read_messages_from_files(self, session_id);
        }
        messages.sort_by(|a, b| {
            let a_key = a
                .as_object()
                .map(OpenCodeStorageAccessor::message_sort_key)
                .unwrap_or((-1, 0.0, String::new()));
            let b_key = b
                .as_object()
                .map(OpenCodeStorageAccessor::message_sort_key)
                .unwrap_or((-1, 0.0, String::new()));
            a_key
                .partial_cmp(&b_key)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        messages
    }

    /// Read parts for a message, preferring SQLite and falling back to JSON files.
    pub fn read_parts(&self, message_id: &str) -> Vec<Value> {
        let mut parts = read_parts_from_db(self, message_id);
        if parts.is_empty() {
            parts = read_parts_from_files(self, message_id);
        }
        parts.sort_by(|a, b| {
            let a_key = a
                .as_object()
                .map(OpenCodeStorageAccessor::part_sort_key)
                .unwrap_or((-1, 0.0, String::new()));
            let b_key = b
                .as_object()
                .map(OpenCodeStorageAccessor::part_sort_key)
                .unwrap_or((-1, 0.0, String::new()));
            a_key
                .partial_cmp(&b_key)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        parts
    }

    /// Find the latest session matching this reader's work directory from the SQLite database.
    pub fn get_latest_session_from_db(&self) -> Option<SessionEntry> {
        let candidates = build_work_dir_candidates(&self.work_dir);
        let rows = self.storage.fetch_opencode_db_rows(
            "SELECT id, directory, time_updated FROM session ORDER BY time_updated DESC LIMIT 200",
            [],
        );
        let mut best_match: Option<SessionEntry> = None;
        let mut best_updated: i64 = -1;
        let mut latest_unfiltered: Option<SessionEntry> = None;
        let mut latest_unfiltered_updated: i64 = -1;

        for row in rows {
            let directory = row.get("directory").and_then(|v| v.as_str())?;
            if !db_directories_match(&candidates, directory, self.allow_parent_match) {
                continue;
            }
            let sid = row.get("id").and_then(|v| v.as_str())?.to_string();
            let updated = row
                .get("time_updated")
                .and_then(|v| v.as_i64())
                .unwrap_or(-1);
            let entry = SessionEntry {
                path: None,
                payload: serde_json::Map::from_iter([
                    ("id".to_string(), Value::String(sid.clone())),
                    (
                        "directory".to_string(),
                        Value::String(directory.to_string()),
                    ),
                    (
                        "time".to_string(),
                        Value::Object(serde_json::Map::from_iter([(
                            "updated".to_string(),
                            Value::Number(updated.into()),
                        )])),
                    ),
                ]),
            };
            if updated > latest_unfiltered_updated {
                latest_unfiltered = Some(entry.clone());
                latest_unfiltered_updated = updated;
            }
            if let Some(filter) = self.session_id_filter() {
                if sid != filter {
                    continue;
                }
            }
            if updated > best_updated {
                best_match = Some(entry);
                best_updated = updated;
            }
        }

        if self._allow_session_rollover {
            if let Some(latest) = latest_unfiltered {
                if latest_unfiltered_updated > best_updated {
                    return Some(latest);
                }
            }
        }
        best_match
    }

    /// Non-blocking attempt to read a new assistant message.
    pub fn try_get_message(
        &self,
        state: &HashMap<String, Value>,
    ) -> (Option<String>, HashMap<String, Value>) {
        let session = match get_latest_session(self) {
            Some(s) => s,
            None => return (None, state.clone()),
        };
        let session_id = session
            .payload
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let session_path = session.path.map(|p| p.to_string_lossy().to_string());
        let mut next_state = state.clone();
        if let Some(sid) = session_id.clone() {
            next_state.insert("session_id".to_string(), Value::String(sid));
        }
        if let Some(spath) = session_path.clone() {
            next_state.insert("session_path".to_string(), Value::String(spath));
        }

        let session_id = match session_id {
            Some(s) => s,
            None => return (None, next_state),
        };

        let messages = self.read_messages(&session_id);
        let re = req_id_re();
        let read_parts = |message_id: &str| self.read_parts(message_id);
        let extract_req_id = |text: &str| super::replies::extract_req_id_from_text(text, &re);

        let (reply, reply_state) = find_new_assistant_reply_with_state(
            &messages,
            &next_state,
            &read_parts,
            Some(&extract_req_id),
        );

        if let Some(rs) = reply_state {
            next_state.extend(rs);
        }
        (reply, next_state)
    }
}

fn default_storage_root() -> PathBuf {
    super::paths::default_opencode_storage_root().unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".local")
            .join("share")
            .join("opencode")
            .join("storage")
    })
}

fn env_truthy(name: &str) -> bool {
    let raw = std::env::var(name).unwrap_or_default();
    matches!(
        raw.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub path: Option<PathBuf>,
    pub payload: serde_json::Map<String, Value>,
}

fn get_latest_session(reader: &OpenCodeLogReader) -> Option<SessionEntry> {
    reader
        .get_latest_session_from_db()
        .or_else(|| get_latest_session_from_files(reader))
}

fn get_latest_session_from_files(reader: &OpenCodeLogReader) -> Option<SessionEntry> {
    let sessions_dir = reader.storage.session_dir(&reader.project_id);
    if !sessions_dir.exists() {
        return None;
    }
    let files = session_files(&sessions_dir);
    let (filtered_match, _filtered_updated) = filtered_match(reader, &files);
    let (best_match, _best_updated, best_any) = scan_file_candidates(reader, &files);

    if let Some(entry) = filtered_match {
        return Some(entry);
    }
    if let Some(entry) = best_match {
        return Some(entry);
    }
    if reader.allow_any_session {
        return best_any;
    }
    None
}

fn session_files(sessions_dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(sessions_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.starts_with("ses_"))
                    .unwrap_or(false)
        })
        .collect()
}

fn filtered_match(reader: &OpenCodeLogReader, files: &[PathBuf]) -> (Option<SessionEntry>, i64) {
    let filter = match reader.session_id_filter() {
        Some(f) => f,
        None => return (None, -1),
    };
    for path in files {
        let payload = reader.storage.load_json(path);
        if let Some(sid) = payload.get("id").and_then(|v| v.as_str()) {
            if sid == filter {
                return (
                    Some(SessionEntry {
                        path: Some(path.clone()),
                        payload: payload.clone(),
                    }),
                    coerce_updated(
                        payload
                            .get("time")
                            .and_then(|t| t.as_object())
                            .and_then(|t| t.get("updated")),
                    ),
                );
            }
        }
    }
    (None, -1)
}

fn scan_file_candidates(
    reader: &OpenCodeLogReader,
    files: &[PathBuf],
) -> (Option<SessionEntry>, i64, Option<SessionEntry>) {
    let mut best_match: Option<SessionEntry> = None;
    let mut best_updated: i64 = -1;
    let mut best_mtime = -1.0;
    let mut best_any: Option<SessionEntry> = None;
    let mut best_any_updated: i64 = -1;
    let mut best_any_mtime = -1.0;

    for path in files {
        let payload = reader.storage.load_json(path);
        let Some(entry) = file_entry(path, payload) else {
            continue;
        };
        let updated = entry.updated;
        let mtime = entry.mtime;
        let candidate = entry.entry;

        if updated > best_any_updated || (updated == best_any_updated && mtime >= best_any_mtime) {
            best_any = Some(candidate.clone());
            best_any_updated = updated;
            best_any_mtime = mtime;
        }

        let directory = candidate.payload.get("directory").and_then(|v| v.as_str());
        if !directories_match(reader, directory) {
            continue;
        }
        if updated > best_updated || (updated == best_updated && mtime >= best_mtime) {
            best_match = Some(candidate);
            best_updated = updated;
            best_mtime = mtime;
        }
    }

    (best_match, best_updated, best_any)
}

#[derive(Debug, Clone)]
struct FileEntry {
    entry: SessionEntry,
    updated: i64,
    mtime: f64,
}

fn file_entry(path: &Path, payload: serde_json::Map<String, Value>) -> Option<FileEntry> {
    let sid = payload.get("id").and_then(|v| v.as_str())?;
    if sid.is_empty() {
        return None;
    }
    let updated = coerce_updated(
        payload
            .get("time")
            .and_then(|t| t.as_object())
            .and_then(|t| t.get("updated")),
    );
    let mtime = path.metadata().and_then(|m| m.modified()).ok()?;
    let mtime = mtime
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    Some(FileEntry {
        entry: SessionEntry {
            path: Some(path.to_path_buf()),
            payload,
        },
        updated,
        mtime,
    })
}

fn directories_match(reader: &OpenCodeLogReader, directory: Option<&str>) -> bool {
    let directory = match directory {
        Some(d) => d,
        None => return false,
    };
    let candidates = build_work_dir_candidates(&reader.work_dir);
    db_directories_match(&candidates, directory, reader.allow_parent_match)
}

fn db_directories_match(candidates: &[String], directory: &str, allow_parent_match: bool) -> bool {
    if directory.trim().is_empty() {
        return false;
    }
    let dir_norm = normalize_path_for_match(directory);
    candidates.iter().any(|c| {
        if allow_parent_match {
            path_matches(&dir_norm, c, true)
        } else {
            dir_norm == *c
        }
    })
}

fn build_work_dir_candidates(work_dir: &Path) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Ok(raw_pwd) = std::env::var("PWD") {
        let trimmed = raw_pwd.trim();
        if !trimmed.is_empty() {
            candidates.push(trimmed.to_string());
        }
    }
    candidates.push(work_dir.to_string_lossy().to_string());
    if let Ok(canonical) = work_dir.canonicalize() {
        candidates.push(canonical.to_string_lossy().to_string());
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for candidate in candidates {
        let norm = normalize_path_for_match(&candidate);
        if !norm.is_empty() && seen.insert(norm.clone()) {
            out.push(norm);
        }
    }
    out
}

fn detect_project_id_for_workdir(
    storage: &OpenCodeStorageAccessor,
    work_dir: &Path,
) -> Option<String> {
    let projects_dir = storage.root().join("project");
    if !projects_dir.exists() {
        return None;
    }
    let work_candidates = build_work_dir_candidates(work_dir);
    let mut best_id: Option<String> = None;
    let mut best_score: (usize, i64, f64) = (0, -1, -1.0);

    let paths = project_json_files(&projects_dir);
    for path in paths {
        let payload = storage.load_json(&path);
        let (pid, worktree_norm) = project_identity(&payload, &path)?;
        if !work_candidates.iter().any(|c| {
            path_matches(
                &worktree_norm,
                c,
                env_truthy("OPENCODE_ALLOW_PARENT_WORKDIR_MATCH"),
            )
        }) {
            continue;
        }
        let score = project_match_score(&payload, &path, &worktree_norm);
        if score > best_score {
            best_id = Some(pid);
            best_score = score;
        }
    }
    best_id
}

fn project_json_files(projects_dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(projects_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json") && p.is_file())
        .collect()
}

fn project_identity(
    payload: &serde_json::Map<String, Value>,
    path: &Path,
) -> Option<(String, String)> {
    let pid = payload
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        });
    let worktree = payload.get("worktree").and_then(|v| v.as_str())?;
    if pid.is_empty() || worktree.is_empty() {
        return None;
    }
    Some((pid, normalize_path_for_match(worktree)))
}

fn project_match_score(
    payload: &serde_json::Map<String, Value>,
    path: &Path,
    worktree_norm: &str,
) -> (usize, i64, f64) {
    let updated = payload
        .get("time")
        .and_then(|t| t.as_object())
        .and_then(|t| t.get("updated"))
        .and_then(|v| v.as_i64())
        .unwrap_or(-1);
    let mtime = path
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    (worktree_norm.len(), updated, mtime)
}

fn coerce_updated(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(-1),
        Some(Value::String(s)) => s.parse::<i64>().unwrap_or(-1),
        _ => -1,
    }
}

fn read_messages_from_db(reader: &OpenCodeLogReader, session_id: &str) -> Vec<Value> {
    let rows = reader.storage.fetch_opencode_db_rows(
        "SELECT id, session_id, time_created, time_updated, data FROM message WHERE session_id = ?1 ORDER BY time_created ASC, time_updated ASC, id ASC",
        [session_id],
    );
    rows.into_iter()
        .map(|row| {
            let mut payload = reader
                .storage
                .load_json_blob(row.get("data").unwrap_or(&Value::Null));
            payload
                .entry("id".to_string())
                .or_insert_with(|| row.get("id").cloned().unwrap_or(Value::Null));
            payload
                .entry("sessionID".to_string())
                .or_insert_with(|| row.get("session_id").cloned().unwrap_or(Value::Null));
            let mut time = payload
                .get("time")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();
            time.entry("created".to_string())
                .or_insert_with(|| row.get("time_created").cloned().unwrap_or(Value::Null));
            time.entry("updated".to_string())
                .or_insert_with(|| row.get("time_updated").cloned().unwrap_or(Value::Null));
            payload.insert("time".to_string(), Value::Object(time));
            Value::Object(payload)
        })
        .collect()
}

fn read_messages_from_files(reader: &OpenCodeLogReader, session_id: &str) -> Vec<Value> {
    let message_dir = reader.storage.message_dir(session_id);
    if !message_dir.exists() {
        return Vec::new();
    }
    std::fs::read_dir(&message_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.starts_with("msg_"))
                    .unwrap_or(false)
        })
        .filter_map(|path| {
            let payload = reader.storage.load_json(&path);
            if payload.get("sessionID").and_then(|v| v.as_str()) != Some(session_id) {
                return None;
            }
            let mut value = Value::Object(payload);
            if let Value::Object(ref mut obj) = value {
                obj.insert(
                    "_path".to_string(),
                    Value::String(path.to_string_lossy().to_string()),
                );
            }
            Some(value)
        })
        .collect()
}

fn read_parts_from_db(reader: &OpenCodeLogReader, message_id: &str) -> Vec<Value> {
    let rows = reader.storage.fetch_opencode_db_rows(
        "SELECT id, message_id, session_id, time_created, time_updated, data FROM part WHERE message_id = ?1 ORDER BY time_created ASC, time_updated ASC, id ASC",
        [message_id],
    );
    rows.into_iter()
        .map(|row| {
            let mut payload = reader
                .storage
                .load_json_blob(row.get("data").unwrap_or(&Value::Null));
            payload
                .entry("id".to_string())
                .or_insert_with(|| row.get("id").cloned().unwrap_or(Value::Null));
            payload
                .entry("messageID".to_string())
                .or_insert_with(|| row.get("message_id").cloned().unwrap_or(Value::Null));
            payload
                .entry("sessionID".to_string())
                .or_insert_with(|| row.get("session_id").cloned().unwrap_or(Value::Null));
            let mut time = payload
                .get("time")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();
            time.entry("start".to_string())
                .or_insert_with(|| row.get("time_created").cloned().unwrap_or(Value::Null));
            time.entry("updated".to_string())
                .or_insert_with(|| row.get("time_updated").cloned().unwrap_or(Value::Null));
            payload.insert("time".to_string(), Value::Object(time));
            Value::Object(payload)
        })
        .collect()
}

fn read_parts_from_files(reader: &OpenCodeLogReader, message_id: &str) -> Vec<Value> {
    let part_dir = reader.storage.part_dir(message_id);
    if !part_dir.exists() {
        return Vec::new();
    }
    std::fs::read_dir(&part_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.starts_with("prt_"))
                    .unwrap_or(false)
        })
        .filter_map(|path| {
            let payload = reader.storage.load_json(&path);
            if payload.get("messageID").and_then(|v| v.as_str()) != Some(message_id) {
                return None;
            }
            let mut value = Value::Object(payload);
            if let Value::Object(ref mut obj) = value {
                obj.insert(
                    "_path".to_string(),
                    Value::String(path.to_string_lossy().to_string()),
                );
            }
            Some(value)
        })
        .collect()
}

fn capture_state(reader: &OpenCodeLogReader) -> HashMap<String, Value> {
    let session = match get_latest_session(reader) {
        Some(s) => s,
        None => return empty_capture_state(),
    };
    let session_id = session
        .payload
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let updated_i = coerce_updated(
        session
            .payload
            .get("time")
            .and_then(|t| t.as_object())
            .and_then(|t| t.get("updated")),
    );
    let assistant_count = session_id.as_ref().map_or(0, |sid| {
        let messages = reader.read_messages(sid);
        messages
            .iter()
            .filter(|m| {
                m.as_object()
                    .and_then(|o| o.get("role").and_then(|r| r.as_str()))
                    == Some("assistant")
            })
            .count()
    });

    let mut state = HashMap::new();
    state.insert(
        "session_path".to_string(),
        session
            .path
            .map(|p| Value::String(p.to_string_lossy().to_string()))
            .unwrap_or(Value::Null),
    );
    state.insert(
        "session_id".to_string(),
        session_id.map(Value::String).unwrap_or(Value::Null),
    );
    state.insert(
        "session_updated".to_string(),
        Value::Number(updated_i.into()),
    );
    state.insert(
        "assistant_count".to_string(),
        Value::Number(assistant_count.into()),
    );
    // last_assistant_* fields are intentionally left empty on first capture so
    // that a subsequent try_get_message can detect the current reply as new.
    state.insert("last_assistant_id".to_string(), Value::Null);
    state.insert("last_assistant_parent_id".to_string(), Value::Null);
    state.insert("last_assistant_completed".to_string(), Value::Null);
    state.insert("last_assistant_req_id".to_string(), Value::Null);
    state.insert("last_assistant_text_hash".to_string(), Value::Null);
    state.insert("last_assistant_aborted".to_string(), Value::Bool(false));
    state
}

fn empty_capture_state() -> HashMap<String, Value> {
    let mut state = HashMap::new();
    state.insert("session_path".to_string(), Value::Null);
    state.insert("session_id".to_string(), Value::Null);
    state.insert("session_updated".to_string(), Value::Number((-1).into()));
    state.insert("assistant_count".to_string(), Value::Number(0.into()));
    state.insert("last_assistant_id".to_string(), Value::Null);
    state.insert("last_assistant_parent_id".to_string(), Value::Null);
    state.insert("last_assistant_completed".to_string(), Value::Null);
    state.insert("last_assistant_req_id".to_string(), Value::Null);
    state.insert("last_assistant_text_hash".to_string(), Value::Null);
    state.insert("last_assistant_aborted".to_string(), Value::Bool(false));
    state
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
    use tempfile::TempDir;

    fn write_json(dir: &std::path::Path, name: &str, content: serde_json::Value) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, serde_json::to_string(&content).unwrap()).unwrap();
        path
    }

    #[test]
    fn test_reader_capture_state_empty() {
        let tmp = TempDir::new().unwrap();
        let reader = OpenCodeLogReader::new(Some(tmp.path()), tmp.path(), "global", None);
        let state = reader.capture_state();
        assert_eq!(state.get("session_id").unwrap(), &Value::Null);
    }

    #[test]
    fn test_reader_detects_project_id() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir(&project_dir).unwrap();
        write_json(
            &project_dir,
            "proj1.json",
            serde_json::json!({
                "id": "proj1",
                "worktree": work_dir.to_string_lossy().to_string(),
            }),
        );
        let reader = OpenCodeLogReader::new(Some(tmp.path()), &work_dir, "global", None);
        assert_eq!(reader.project_id(), "proj1");
    }

    #[test]
    fn test_reader_reads_reply() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        let storage_root = tmp.path().join("storage");
        std::fs::create_dir_all(storage_root.join("session").join("proj1")).unwrap();
        std::fs::create_dir_all(storage_root.join("message")).unwrap();
        std::fs::create_dir_all(storage_root.join("part").join("m2")).unwrap();

        write_json(
            &storage_root.join("session").join("proj1"),
            "ses_1.json",
            serde_json::json!({
                "id": "session-1",
                "directory": work_dir.to_string_lossy().to_string(),
                "time": {"updated": 1},
            }),
        );
        write_json(
            &storage_root.join("message"),
            "msg_m1.json",
            serde_json::json!({
                "id": "m1",
                "sessionID": "session-1",
                "role": "user",
                "parentID": "m0",
                "time": {"created": 1},
            }),
        );
        write_json(
            &storage_root.join("message"),
            "msg_m2.json",
            serde_json::json!({
                "id": "m2",
                "sessionID": "session-1",
                "role": "assistant",
                "parentID": "m1",
                "time": {"created": 2, "completed": 12345},
            }),
        );
        write_json(
            &storage_root.join("part").join("m2"),
            "prt_p1.json",
            serde_json::json!({
                "id": "p1",
                "messageID": "m2",
                "type": "text",
                "text": "hello world",
                "time": {"start": 2},
            }),
        );

        let reader = OpenCodeLogReader::new(Some(&storage_root), &work_dir, "proj1", None);
        let state = reader.capture_state();
        assert_eq!(state.get("session_id").unwrap(), "session-1");
        assert_eq!(state.get("assistant_count").unwrap(), 1);

        let (reply, next_state) = reader.try_get_message(&state);
        assert_eq!(reply.as_deref(), Some("hello world"));
        assert_eq!(next_state.get("last_assistant_id").unwrap(), "m2");
    }

    #[test]
    fn test_opencode_log_reader_reads_messages_and_parts_from_sqlite() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("storage");
        std::fs::create_dir(&root).unwrap();
        let db_path = tmp.path().join("opencode.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE message (
                 id TEXT PRIMARY KEY,
                 session_id TEXT NOT NULL,
                 time_created INTEGER NOT NULL,
                 time_updated INTEGER NOT NULL,
                 data TEXT NOT NULL
             );
             CREATE TABLE part (
                 id TEXT PRIMARY KEY,
                 message_id TEXT NOT NULL,
                 session_id TEXT NOT NULL,
                 time_created INTEGER NOT NULL,
                 time_updated INTEGER NOT NULL,
                 data TEXT NOT NULL
             );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?)",
            rusqlite::params![
                "msg_sqlite",
                "ses_sqlite",
                1700000000123i64,
                1700000000999i64,
                serde_json::json!({"role": "assistant"}).to_string()
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                "prt_sqlite",
                "msg_sqlite",
                "ses_sqlite",
                1700000000222i64,
                1700000000888i64,
                serde_json::json!({"type": "text", "text": "hello from sqlite"}).to_string()
            ],
        )
        .unwrap();
        drop(conn);

        let reader = OpenCodeLogReader::new(Some(&root), tmp.path(), "proj-test", None);
        let messages = reader.read_messages("ses_sqlite");
        assert_eq!(messages.len(), 1);
        let m = messages[0].as_object().unwrap();
        assert_eq!(m.get("id").unwrap(), "msg_sqlite");
        assert_eq!(m.get("sessionID").unwrap(), "ses_sqlite");
        assert_eq!(m.get("role").unwrap(), "assistant");
        assert_eq!(
            m.get("time")
                .unwrap()
                .as_object()
                .unwrap()
                .get("created")
                .unwrap(),
            1700000000123i64
        );

        let parts = reader.read_parts("msg_sqlite");
        assert_eq!(parts.len(), 1);
        let p = parts[0].as_object().unwrap();
        assert_eq!(p.get("id").unwrap(), "prt_sqlite");
        assert_eq!(p.get("messageID").unwrap(), "msg_sqlite");
        assert_eq!(p.get("sessionID").unwrap(), "ses_sqlite");
        assert_eq!(p.get("text").unwrap(), "hello from sqlite");
        assert_eq!(
            p.get("time")
                .unwrap()
                .as_object()
                .unwrap()
                .get("start")
                .unwrap(),
            1700000000222i64
        );
    }

    #[test]
    fn test_opencode_log_reader_falls_back_to_json_when_sqlite_has_no_matching_rows() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("storage");
        let message_dir = root.join("message").join("ses_file");
        let part_dir = root.join("part").join("msg_file");
        std::fs::create_dir_all(&message_dir).unwrap();
        std::fs::create_dir_all(&part_dir).unwrap();
        write_json(
            &message_dir,
            "msg_file.json",
            serde_json::json!({
                "id": "msg_file",
                "sessionID": "ses_file",
                "role": "assistant",
                "time": {"created": 1700000100000i64, "completed": 1700000100010i64},
            }),
        );
        write_json(
            &part_dir,
            "prt_file.json",
            serde_json::json!({
                "id": "prt_file",
                "messageID": "msg_file",
                "type": "text",
                "text": "hello from json",
                "time": {"start": 1700000100001i64},
            }),
        );

        let db_path = tmp.path().join("opencode.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL, data TEXT NOT NULL);
             CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT NOT NULL, session_id TEXT NOT NULL, time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL, data TEXT NOT NULL);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?)",
            rusqlite::params![
                "msg_other",
                "ses_other",
                1i64,
                2i64,
                serde_json::json!({"role": "assistant"}).to_string()
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                "prt_other",
                "msg_other",
                "ses_other",
                1i64,
                2i64,
                serde_json::json!({"type": "text", "text": "other"}).to_string()
            ],
        )
        .unwrap();
        drop(conn);

        let reader = OpenCodeLogReader::new(Some(&root), tmp.path(), "proj-test", None);
        let messages = reader.read_messages("ses_file");
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].as_object().unwrap().get("id").unwrap(),
            "msg_file"
        );

        let parts = reader.read_parts("msg_file");
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].as_object().unwrap().get("id").unwrap(), "prt_file");
    }

    #[test]
    fn test_opencode_log_reader_stays_pinned_to_filtered_session_by_default() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("storage");
        std::fs::create_dir(&root).unwrap();
        let db_path = tmp.path().join("opencode.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT NOT NULL, time_updated INTEGER NOT NULL)",
        )
        .unwrap();
        let project_dir = tmp.path().join("repo");
        std::fs::create_dir(&project_dir).unwrap();
        conn.execute(
            "INSERT INTO session (id, directory, time_updated) VALUES (?, ?, ?)",
            rusqlite::params!["ses_old", project_dir.to_string_lossy().to_string(), 100i64],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session (id, directory, time_updated) VALUES (?, ?, ?)",
            rusqlite::params!["ses_new", project_dir.to_string_lossy().to_string(), 200i64],
        )
        .unwrap();
        drop(conn);

        let reader = OpenCodeLogReader::new(
            Some(&root),
            &project_dir,
            "proj-test",
            Some("ses_old".to_string()),
        );
        let latest = reader.get_latest_session_from_db();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().payload.get("id").unwrap(), "ses_old");
    }
}
