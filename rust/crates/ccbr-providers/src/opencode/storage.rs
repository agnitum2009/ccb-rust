use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

/// Accessor for OpenCode file storage paths and SQLite rows.
/// SQLite path resolution is exposed for callers that have their own database layer.
#[derive(Debug, Clone)]
pub struct OpenCodeStorageAccessor {
    root: PathBuf,
    db_path_hint: Option<PathBuf>,
}

impl OpenCodeStorageAccessor {
    pub fn new(root: &Path) -> Self {
        Self {
            root: PathBuf::from(shellexpand::tilde(&root.to_string_lossy())),
            db_path_hint: None,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn session_dir(&self, project_id: &str) -> PathBuf {
        self.root.join("session").join(project_id)
    }

    pub fn message_dir(&self, message_id: &str) -> PathBuf {
        let nested = self.root.join("message").join(message_id);
        if nested.exists() {
            return nested;
        }
        self.root.join("message")
    }

    pub fn part_dir(&self, message_id: &str) -> PathBuf {
        let nested = self.root.join("part").join(message_id);
        if nested.exists() {
            return nested;
        }
        self.root.join("part")
    }

    pub fn load_json(&self, path: &Path) -> serde_json::Map<String, serde_json::Value> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    pub fn load_json_blob(
        &self,
        raw: &serde_json::Value,
    ) -> serde_json::Map<String, serde_json::Value> {
        if let Some(obj) = raw.as_object() {
            return obj.clone();
        }
        raw.as_str()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    pub fn opencode_db_candidates(&self) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        if let Ok(env) = std::env::var("OPENCODE_DB_PATH") {
            let trimmed = env.trim();
            if !trimmed.is_empty() {
                candidates.push(PathBuf::from(shellexpand::tilde(trimmed)));
            }
        }
        let parent_db = self
            .root
            .parent()
            .map(|p| p.join("opencode.db"))
            .unwrap_or_else(|| self.root.join("opencode.db"));
        candidates.push(parent_db);
        candidates.push(self.root.join("opencode.db"));
        let mut seen = HashMap::new();
        candidates
            .into_iter()
            .filter(|p| {
                let key = p.to_string_lossy().to_string();
                seen.insert(key, ()).is_none()
            })
            .collect()
    }

    pub fn resolve_opencode_db_path(&mut self) -> Option<PathBuf> {
        if let Some(cached) = self.cached_db_path() {
            return Some(cached);
        }
        let resolved = self.existing_db_candidate()?;
        self.db_path_hint = Some(resolved.clone());
        Some(resolved)
    }

    fn cached_db_path(&self) -> Option<PathBuf> {
        let candidate = self.db_path_hint.as_ref()?;
        if candidate.exists() {
            Some(candidate.clone())
        } else {
            None
        }
    }

    fn existing_db_candidate(&self) -> Option<PathBuf> {
        self.opencode_db_candidates()
            .into_iter()
            .find(|p| p.exists() && p.is_file())
    }

    /// Fetch rows from the resolved OpenCode SQLite database.
    /// Mirrors Python `OpenCodeStorageAccessor.fetch_opencode_db_rows`.
    pub fn fetch_opencode_db_rows<P: rusqlite::Params>(
        &self,
        query: &str,
        params: P,
    ) -> Vec<HashMap<String, serde_json::Value>> {
        let db_path = match self.existing_db_candidate() {
            Some(p) => p,
            None => return Vec::new(),
        };
        let conn = match open_readonly_connection(&db_path) {
            Some(c) => c,
            None => return Vec::new(),
        };
        let mut stmt = match conn.prepare(query) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let columns: Vec<String> = stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let rows = match stmt.query_map(params, |row| {
            let mut map = HashMap::new();
            for (idx, name) in columns.iter().enumerate() {
                let value = row.get_ref_unwrap(idx);
                map.insert(name.clone(), sqlite_value_to_json(value));
            }
            Ok(map)
        }) {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        };
        rows
    }

    /// Sort key for messages, matching Python `message_sort_key`.
    pub fn message_sort_key(
        message: &serde_json::Map<String, serde_json::Value>,
    ) -> (i64, f64, String) {
        let created = message
            .get("time")
            .and_then(|t| t.as_object())
            .and_then(|t| t.get("created"))
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);
        let mtime = message
            .get("_path")
            .and_then(|v| v.as_str())
            .and_then(|p| PathBuf::from(p).metadata().ok())
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let message_id = message
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        (created, mtime, message_id)
    }

    /// Sort key for parts, matching Python `part_sort_key`.
    pub fn part_sort_key(part: &serde_json::Map<String, serde_json::Value>) -> (i64, f64, String) {
        let started = part
            .get("time")
            .and_then(|t| t.as_object())
            .and_then(|t| t.get("start"))
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);
        let mtime = part
            .get("_path")
            .and_then(|v| v.as_str())
            .and_then(|p| PathBuf::from(p).metadata().ok())
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let part_id = part
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        (started, mtime, part_id)
    }
}

fn open_readonly_connection(db_path: &Path) -> Option<Connection> {
    connect_readonly(db_path).or_else(|| connect_direct(db_path))
}

fn connect_readonly(db_path: &Path) -> Option<Connection> {
    let uri = format!("file:{}?mode=ro", db_path.to_string_lossy());
    Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .ok()
}

fn connect_direct(db_path: &Path) -> Option<Connection> {
    Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()
}

fn sqlite_value_to_json(value: rusqlite::types::ValueRef) -> serde_json::Value {
    match value {
        rusqlite::types::ValueRef::Null => serde_json::Value::Null,
        rusqlite::types::ValueRef::Integer(i) => serde_json::Value::Number(i.into()),
        rusqlite::types::ValueRef::Real(f) => {
            serde_json::Value::Number(serde_json::Number::from_f64(f).unwrap_or_else(|| 0.into()))
        }
        rusqlite::types::ValueRef::Text(s) => {
            serde_json::Value::String(String::from_utf8_lossy(s).to_string())
        }
        rusqlite::types::ValueRef::Blob(b) => {
            serde_json::Value::String(String::from_utf8_lossy(b).to_string())
        }
    }
}

// Minimal shell expansion helper.
mod shellexpand {
    use std::env;

    pub fn tilde(input: &str) -> String {
        if let Some(rest) = input.strip_prefix('~') {
            if let Ok(home) = env::var("HOME") {
                return home + rest;
            }
        }
        input.to_string()
    }
}
