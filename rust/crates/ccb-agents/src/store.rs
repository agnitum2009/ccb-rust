use ccb_storage::atomic::atomic_write_json;
use ccb_storage::paths::PathLayout;
use serde::{Deserialize, Serialize};

use crate::models::{AgentRestoreState, AgentRuntime, AgentSpec, SCHEMA_VERSION};

pub const RECORD_TYPE_AGENT_SPEC: &str = "agent_spec";
pub const RECORD_TYPE_AGENT_RUNTIME: &str = "agent_runtime";
pub const RECORD_TYPE_AGENT_RESTORE: &str = "agent_restore";

#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct StoreError(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpecRecord {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_agent_spec_record_type")]
    pub record_type: String,
    #[serde(flatten)]
    pub spec: AgentSpec,
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}
fn default_agent_spec_record_type() -> String {
    RECORD_TYPE_AGENT_SPEC.into()
}
fn default_agent_runtime_record_type() -> String {
    RECORD_TYPE_AGENT_RUNTIME.into()
}
fn default_agent_restore_record_type() -> String {
    RECORD_TYPE_AGENT_RESTORE.into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeRecord {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_agent_runtime_record_type")]
    pub record_type: String,
    #[serde(flatten)]
    pub runtime: AgentRuntime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRestoreRecord {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_agent_restore_record_type")]
    pub record_type: String,
    #[serde(flatten)]
    pub restore: AgentRestoreState,
}

#[derive(Debug, Clone)]
pub struct AgentSpecStore {
    paths: PathLayout,
}

impl AgentSpecStore {
    pub fn new(paths: PathLayout) -> Self {
        Self { paths }
    }

    pub fn path(&self, agent_name: &str) -> std::path::PathBuf {
        self.paths.agent_dir(agent_name).join("spec.json").into()
    }

    pub fn load(&self, agent_name: &str) -> crate::Result<Option<AgentSpec>> {
        let path = self.path(agent_name);
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)?;
        let record: AgentSpecRecord = serde_json::from_str(&text)?;
        validate_record_type(&record.record_type, RECORD_TYPE_AGENT_SPEC)?;
        validate_schema_version(record.schema_version)?;
        Ok(Some(record.spec))
    }

    pub fn save(&self, spec: &AgentSpec) -> crate::Result<()> {
        spec.validate()?;
        let path = self.path(&spec.name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let record = AgentSpecRecord {
            schema_version: SCHEMA_VERSION,
            record_type: RECORD_TYPE_AGENT_SPEC.into(),
            spec: spec.clone(),
        };
        let utf8_path = camino::Utf8Path::from_path(&path).ok_or_else(|| {
            crate::AgentError::Store(StoreError("agent spec path is not valid utf-8".into()))
        })?;
        atomic_write_json(utf8_path, &record)?;
        Ok(())
    }

    pub fn remove(&self, agent_name: &str) -> crate::Result<bool> {
        let path = self.path(agent_name);
        if !path.exists() {
            return Ok(false);
        }
        std::fs::remove_file(&path)?;
        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct AgentRuntimeStore {
    paths: PathLayout,
}

impl AgentRuntimeStore {
    pub fn new(paths: PathLayout) -> Self {
        Self { paths }
    }

    pub fn path(&self, agent_name: &str) -> std::path::PathBuf {
        self.paths.agent_dir(agent_name).join("runtime.json").into()
    }

    pub fn load(&self, agent_name: &str) -> crate::Result<Option<AgentRuntime>> {
        let path = self.path(agent_name);
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)?;
        let record: AgentRuntimeRecord = serde_json::from_str(&text)?;
        validate_record_type(&record.record_type, RECORD_TYPE_AGENT_RUNTIME)?;
        validate_schema_version(record.schema_version)?;
        Ok(Some(record.runtime))
    }

    pub fn save(&self, runtime: &AgentRuntime) -> crate::Result<()> {
        let path = self.path(&runtime.agent_name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let record = AgentRuntimeRecord {
            schema_version: SCHEMA_VERSION,
            record_type: RECORD_TYPE_AGENT_RUNTIME.into(),
            runtime: runtime.clone(),
        };
        let utf8_path = camino::Utf8Path::from_path(&path).ok_or_else(|| {
            crate::AgentError::Store(StoreError("agent runtime path is not valid utf-8".into()))
        })?;
        atomic_write_json(utf8_path, &record)?;
        Ok(())
    }

    pub fn remove(&self, agent_name: &str) -> crate::Result<bool> {
        let path = self.path(agent_name);
        if !path.exists() {
            return Ok(false);
        }
        std::fs::remove_file(&path)?;
        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct AgentRestoreStore {
    paths: PathLayout,
}

impl AgentRestoreStore {
    pub fn new(paths: PathLayout) -> Self {
        Self { paths }
    }

    pub fn path(&self, agent_name: &str) -> std::path::PathBuf {
        self.paths.agent_dir(agent_name).join("restore.json").into()
    }

    pub fn load(&self, agent_name: &str) -> crate::Result<Option<AgentRestoreState>> {
        let path = self.path(agent_name);
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)?;
        let record: AgentRestoreRecord = serde_json::from_str(&text)?;
        validate_record_type(&record.record_type, RECORD_TYPE_AGENT_RESTORE)?;
        validate_schema_version(record.schema_version)?;
        Ok(Some(record.restore))
    }

    pub fn save(&self, agent_name: &str, restore: &AgentRestoreState) -> crate::Result<()> {
        let path = self.path(agent_name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let record = AgentRestoreRecord {
            schema_version: SCHEMA_VERSION,
            record_type: RECORD_TYPE_AGENT_RESTORE.into(),
            restore: restore.clone(),
        };
        let utf8_path = camino::Utf8Path::from_path(&path).ok_or_else(|| {
            crate::AgentError::Store(StoreError("agent restore path is not valid utf-8".into()))
        })?;
        atomic_write_json(utf8_path, &record)?;
        Ok(())
    }

    pub fn remove(&self, agent_name: &str) -> crate::Result<bool> {
        let path = self.path(agent_name);
        if !path.exists() {
            return Ok(false);
        }
        std::fs::remove_file(&path)?;
        Ok(true)
    }
}

fn validate_record_type(actual: &str, expected: &str) -> crate::Result<()> {
    if actual != expected {
        return Err(crate::AgentError::Store(StoreError(format!(
            "record_type mismatch: expected {expected}, got {actual}"
        ))));
    }
    Ok(())
}

fn validate_schema_version(version: u32) -> crate::Result<()> {
    if version != SCHEMA_VERSION {
        return Err(crate::AgentError::Store(StoreError(format!(
            "schema_version mismatch: expected {SCHEMA_VERSION}, got {version}"
        ))));
    }
    Ok(())
}

/// Render a default fresh restore record.
pub fn fresh_restore_state() -> AgentRestoreState {
    AgentRestoreState {
        restore_mode: crate::models::RestoreMode::Fresh,
        last_checkpoint: None,
        conversation_summary: "fresh start".into(),
        open_tasks: Vec::new(),
        files_touched: Vec::new(),
        base_commit: None,
        head_commit: None,
        last_restore_status: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths() -> (PathLayout, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        (PathLayout::new(root), dir)
    }

    fn sample_spec(name: &str) -> AgentSpec {
        AgentSpec {
            name: name.into(),
            provider: "codex".into(),
            target: ".".into(),
            ..crate::models::AgentSpec::default_with_name(name)
        }
    }

    #[test]
    fn test_spec_roundtrip() {
        let (paths, _dir) = temp_paths();
        let store = AgentSpecStore::new(paths);
        let spec = sample_spec("agent1");
        store.save(&spec).unwrap();
        let loaded = store.load("agent1").unwrap().unwrap();
        assert_eq!(loaded.name, "agent1");
    }

    #[test]
    fn test_restore_roundtrip() {
        let (paths, _dir) = temp_paths();
        let store = AgentRestoreStore::new(paths);
        let restore = fresh_restore_state();
        store.save("agent1", &restore).unwrap();
        let loaded = store.load("agent1").unwrap().unwrap();
        assert_eq!(loaded.restore_mode, crate::models::RestoreMode::Fresh);
    }
}
