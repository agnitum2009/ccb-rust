use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Accessor for OpenCode file storage paths.
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
