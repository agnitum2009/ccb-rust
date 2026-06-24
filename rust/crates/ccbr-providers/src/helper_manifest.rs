//! Mirrors Python `lib/provider_runtime/helper_manifest.py`.

use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use ccbr_storage::json::JsonStore;
use ccbr_storage::path_helpers::normalize_agent_name;
use ccbr_storage::paths::PathLayout;

const SCHEMA_VERSION: i64 = 1;
const RECORD_TYPE: &str = "provider_helper_manifest";

/// On-disk record for a provider helper process (e.g., Codex bridge).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderHelperManifest {
    #[serde(rename = "schema_version")]
    pub schema_version: i64,
    #[serde(rename = "record_type")]
    pub record_type: String,
    pub agent_name: String,
    pub runtime_generation: i64,
    pub helper_kind: String,
    pub leader_pid: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pgid: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_daemon_generation: Option<i64>,
    #[serde(default = "default_state")]
    pub state: String,
}

fn default_state() -> String {
    "running".to_string()
}

impl ProviderHelperManifest {
    /// Build a new manifest, normalizing and validating inputs.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        agent_name: impl AsRef<str>,
        runtime_generation: i64,
        helper_kind: impl AsRef<str>,
        leader_pid: i64,
        pgid: Option<i64>,
        started_at: Option<String>,
        owner_daemon_generation: Option<i64>,
        state: Option<String>,
    ) -> Result<Self> {
        let agent_name = normalize_agent_name(agent_name.as_ref())?;
        let runtime_generation = runtime_generation.max(1);
        let helper_kind = helper_kind.as_ref().trim().to_string();
        let state = state
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "running".to_string());
        if leader_pid <= 0 {
            return Err(anyhow!("leader_pid must be positive"));
        }
        if helper_kind.is_empty() {
            return Err(anyhow!("helper_kind cannot be empty"));
        }
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            record_type: RECORD_TYPE.to_string(),
            agent_name,
            runtime_generation,
            helper_kind,
            leader_pid,
            pgid,
            started_at,
            owner_daemon_generation,
            state,
        })
    }

    /// Convert to the on-disk record shape.
    pub fn to_record(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut map = serde_json::Map::new();
        map.insert(
            "schema_version".to_string(),
            serde_json::Value::Number(SCHEMA_VERSION.into()),
        );
        map.insert(
            "record_type".to_string(),
            serde_json::Value::String(RECORD_TYPE.to_string()),
        );
        map.insert(
            "agent_name".to_string(),
            serde_json::Value::String(self.agent_name.clone()),
        );
        map.insert(
            "runtime_generation".to_string(),
            serde_json::Value::Number(self.runtime_generation.into()),
        );
        map.insert(
            "helper_kind".to_string(),
            serde_json::Value::String(self.helper_kind.clone()),
        );
        map.insert(
            "leader_pid".to_string(),
            serde_json::Value::Number(self.leader_pid.into()),
        );
        if let Some(pgid) = self.pgid {
            map.insert("pgid".to_string(), serde_json::Value::Number(pgid.into()));
        }
        if let Some(started_at) = &self.started_at {
            map.insert(
                "started_at".to_string(),
                serde_json::Value::String(started_at.clone()),
            );
        }
        if let Some(owner) = self.owner_daemon_generation {
            map.insert(
                "owner_daemon_generation".to_string(),
                serde_json::Value::Number(owner.into()),
            );
        }
        map.insert(
            "state".to_string(),
            serde_json::Value::String(self.state.clone()),
        );
        map
    }
}

/// Load a manifest from disk, returning `None` if missing or invalid.
pub fn load_helper_manifest(path: &Utf8Path) -> Option<ProviderHelperManifest> {
    if !path.exists() {
        return None;
    }
    let store = JsonStore::new();
    store
        .load::<serde_json::Map<String, serde_json::Value>>(path)
        .ok()
        .and_then(|record| {
            validate_record(&record, RECORD_TYPE).ok()?;
            Some(ProviderHelperManifest {
                schema_version: record.get("schema_version")?.as_i64()?,
                record_type: record.get("record_type")?.as_str()?.to_string(),
                agent_name: record.get("agent_name")?.as_str()?.to_string(),
                runtime_generation: record.get("runtime_generation")?.as_i64()?,
                helper_kind: record.get("helper_kind")?.as_str()?.to_string(),
                leader_pid: record.get("leader_pid")?.as_i64()?,
                pgid: record.get("pgid").and_then(|v| v.as_i64()),
                started_at: record
                    .get("started_at")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                owner_daemon_generation: record
                    .get("owner_daemon_generation")
                    .and_then(|v| v.as_i64()),
                state: record
                    .get("state")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "running".to_string()),
            })
        })
}

/// Save a manifest to disk atomically.
pub fn save_helper_manifest(
    path: &Utf8Path,
    manifest: &ProviderHelperManifest,
) -> Result<Utf8PathBuf> {
    let store = JsonStore::new();
    store.save(path, &manifest.to_record())?;
    Ok(path.into())
}

/// Remove the manifest file if it exists.
pub fn clear_helper_manifest(path: &Utf8Path) {
    let _ = std::fs::remove_file(path);
}

/// Write the current runtime helper manifest for an agent, or clear it if none.
pub fn sync_runtime_helper_manifest(
    layout: &PathLayout,
    runtime: &ProviderRuntimeView,
) -> Option<ProviderHelperManifest> {
    let helper_path = layout.agent_helper_path(&runtime.agent_name);
    let manifest = build_runtime_helper_manifest(runtime);
    match manifest {
        Some(ref m) => {
            let _ = save_helper_manifest(&helper_path, m);
            Some(m.clone())
        }
        None => {
            clear_helper_manifest(&helper_path);
            None
        }
    }
}

/// Build a manifest from a runtime view, returning `None` when not applicable.
pub fn build_runtime_helper_manifest(
    runtime: &ProviderRuntimeView,
) -> Option<ProviderHelperManifest> {
    let provider = runtime.provider.trim().to_lowercase();
    if provider != "codex" {
        return None;
    }
    let runtime_root = runtime.runtime_root.trim();
    if runtime_root.is_empty() {
        return None;
    }
    let leader_pid = read_pid(Utf8Path::new(runtime_root).join("bridge.pid").as_ref())?;
    let runtime_generation = canonical_runtime_generation(runtime.runtime_generation)?;
    let started_at = runtime
        .started_at
        .as_deref()
        .or(runtime.last_seen_at.as_deref())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    ProviderHelperManifest::new(
        &runtime.agent_name,
        runtime_generation,
        "codex_bridge",
        leader_pid,
        Some(leader_pid),
        started_at,
        runtime.daemon_generation,
        Some("running".to_string()),
    )
    .ok()
}

/// Lightweight runtime snapshot used by helper manifest logic.
#[derive(Debug, Clone, Default)]
pub struct ProviderRuntimeView {
    pub agent_name: String,
    pub provider: String,
    pub state: Option<ccbr_agents::models::AgentState>,
    pub runtime_root: String,
    pub runtime_generation: Option<i64>,
    pub started_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub daemon_generation: Option<i64>,
}

pub(crate) fn canonical_runtime_generation(generation: Option<i64>) -> Option<i64> {
    generation.filter(|&g| g > 0)
}

fn read_pid(path: &Utf8Path) -> Option<i64> {
    let raw = std::fs::read_to_string(path).ok()?;
    let raw = raw.trim();
    raw.parse::<u64>().ok()?;
    let pid: i64 = raw.parse().ok()?;
    if pid > 0 {
        Some(pid)
    } else {
        None
    }
}

fn validate_record(
    record: &serde_json::Map<String, serde_json::Value>,
    expected_type: &str,
) -> Result<()> {
    if record.get("schema_version").and_then(|v| v.as_i64()) != Some(SCHEMA_VERSION) {
        return Err(anyhow!("schema_version must be {SCHEMA_VERSION}"));
    }
    if record
        .get("record_type")
        .and_then(|v| v.as_str())
        .map(|s| s != expected_type)
        .unwrap_or(true)
    {
        return Err(anyhow!("record_type must be {expected_type:?}"));
    }
    Ok(())
}
