use std::path::PathBuf;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROVIDER_HELPER_SCHEMA_VERSION: u32 = 1;

/// Manifest describing a provider runtime helper process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHelperManifest {
    pub schema_version: u32,
    pub record_type: String,
    pub agent_name: String,
    pub runtime_generation: u64,
    pub helper_kind: String,
    pub leader_pid: u64,
    pub pgid: Option<u64>,
    pub started_at: Option<String>,
    pub owner_daemon_generation: Option<u64>,
    pub state: String,
}

impl ProviderHelperManifest {
    pub fn new(
        agent_name: impl Into<String>,
        runtime_generation: u64,
        helper_kind: impl Into<String>,
        leader_pid: u64,
    ) -> Self {
        assert!(leader_pid > 0, "leader_pid must be positive");
        let helper_kind = helper_kind.into();
        assert!(
            !helper_kind.trim().is_empty(),
            "helper_kind cannot be empty"
        );
        Self {
            schema_version: PROVIDER_HELPER_SCHEMA_VERSION,
            record_type: "provider_helper_manifest".to_string(),
            agent_name: agent_name.into(),
            runtime_generation: runtime_generation.max(1),
            helper_kind,
            leader_pid,
            pgid: Some(leader_pid),
            started_at: None,
            owner_daemon_generation: None,
            state: "running".to_string(),
        }
    }

    pub fn with_started_at(mut self, at: impl Into<String>) -> Self {
        let value = at.into();
        self.started_at = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
        self
    }

    pub fn with_owner_daemon_generation(mut self, generation: Option<u64>) -> Self {
        self.owner_daemon_generation = generation;
        self
    }

    pub fn to_record(&self) -> Value {
        serde_json::json!({
            "schema_version": self.schema_version,
            "record_type": self.record_type,
            "agent_name": self.agent_name,
            "runtime_generation": self.runtime_generation,
            "helper_kind": self.helper_kind,
            "leader_pid": self.leader_pid,
            "pgid": self.pgid,
            "started_at": self.started_at,
            "owner_daemon_generation": self.owner_daemon_generation,
            "state": self.state,
        })
    }
}

/// Load a helper manifest from disk.
pub fn load_helper_manifest(path: &Utf8Path) -> Option<ProviderHelperManifest> {
    if !path.exists() {
        return None;
    }
    let store = ccbr_storage::json::JsonStore::new();
    store.load(path).ok()
}

/// Save a helper manifest to disk.
pub fn save_helper_manifest(
    path: &Utf8Path,
    manifest: &ProviderHelperManifest,
) -> ccbr_storage::Result<()> {
    let store = ccbr_storage::json::JsonStore::new();
    store.save(path, manifest)
}

/// Clear a helper manifest from disk.
pub fn clear_helper_manifest(path: &Utf8Path) {
    let _ = std::fs::remove_file(path);
}

/// Build a runtime helper manifest for a runtime object.
pub fn build_runtime_helper_manifest(runtime: &RuntimeInfo) -> Option<ProviderHelperManifest> {
    if runtime.provider != "codex" {
        return None;
    }
    let runtime_root = runtime.runtime_root.as_deref()?.trim();
    if runtime_root.is_empty() {
        return None;
    }
    let leader_pid = read_pid(PathBuf::from(runtime_root).join("bridge.pid"))?;
    let runtime_generation = runtime.runtime_generation?;
    if runtime_generation == 0 {
        return None;
    }
    let started_at = runtime
        .started_at
        .clone()
        .or_else(|| runtime.last_seen_at.clone())
        .filter(|s| !s.trim().is_empty());
    Some(
        ProviderHelperManifest::new(
            runtime.agent_name.clone(),
            runtime_generation,
            "codex_bridge",
            leader_pid,
        )
        .with_started_at(started_at.unwrap_or_default())
        .with_owner_daemon_generation(runtime.daemon_generation),
    )
}

fn read_pid(path: PathBuf) -> Option<u64> {
    let raw = std::fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let pid = trimmed.parse::<u64>().ok()?;
    if pid == 0 {
        return None;
    }
    Some(pid)
}

/// Minimal runtime info used to build helper manifests.
#[derive(Debug, Clone, Default)]
pub struct RuntimeInfo {
    pub agent_name: String,
    pub provider: String,
    pub runtime_root: Option<String>,
    pub runtime_generation: Option<u64>,
    pub started_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub daemon_generation: Option<u64>,
    pub state: String,
}
