//! Mirrors Python `lib/ccbrd/reload_handoff.py`.

use crate::app::CcbdApp;
use ccbr_storage::json::JsonStore;
use ccbr_storage::paths::PathLayout;
use serde::{Deserialize, Serialize};

pub const RELOAD_HANDOFF_TTL_S: f64 = 60.0;
const RECORD_TYPE: &str = "ccbrd_reload_handoff";

/// An in-progress reload handoff record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadHandoff {
    pub project_id: String,
    pub started_at: String,
    pub old_config_signature: String,
    pub target_config_signature: String,
    pub daemon_pid: u32,
    pub daemon_instance_id: String,
    pub generation: u32,
    pub status: String,
    #[serde(default = "default_ttl")]
    pub ttl_s: f64,
}

fn default_ttl() -> f64 {
    RELOAD_HANDOFF_TTL_S
}

impl ReloadHandoff {
    pub fn new(
        project_id: impl Into<String>,
        started_at: impl Into<String>,
        old_config_signature: impl Into<String>,
        target_config_signature: impl Into<String>,
        daemon_pid: u32,
        daemon_instance_id: impl Into<String>,
        generation: u32,
    ) -> Self {
        let project_id = project_id.into();
        let started_at = started_at.into();
        let old_config_signature = old_config_signature.into();
        let target_config_signature = target_config_signature.into();
        let daemon_instance_id = daemon_instance_id.into();
        if project_id.trim().is_empty() {
            panic!("project_id cannot be empty");
        }
        if started_at.trim().is_empty() {
            panic!("started_at cannot be empty");
        }
        if old_config_signature.trim().is_empty() {
            panic!("old_config_signature cannot be empty");
        }
        if target_config_signature.trim().is_empty() {
            panic!("target_config_signature cannot be empty");
        }
        if daemon_pid == 0 {
            panic!("daemon_pid must be positive");
        }
        if daemon_instance_id.trim().is_empty() {
            panic!("daemon_instance_id cannot be empty");
        }
        if generation == 0 {
            panic!("generation must be positive");
        }
        Self {
            project_id,
            started_at,
            old_config_signature,
            target_config_signature,
            daemon_pid,
            daemon_instance_id,
            generation,
            status: "applying".to_string(),
            ttl_s: RELOAD_HANDOFF_TTL_S,
        }
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": crate::models::api_models::common::SCHEMA_VERSION,
            "record_type": RECORD_TYPE,
            "project_id": self.project_id,
            "started_at": self.started_at,
            "old_config_signature": self.old_config_signature,
            "target_config_signature": self.target_config_signature,
            "daemon_pid": self.daemon_pid,
            "daemon_instance_id": self.daemon_instance_id,
            "generation": self.generation,
            "status": self.status,
            "ttl_s": self.ttl_s,
        })
    }

    pub fn from_record(record: &serde_json::Map<String, serde_json::Value>) -> Self {
        if record.get("schema_version")
            != Some(&serde_json::json!(
                crate::models::api_models::common::SCHEMA_VERSION
            ))
        {
            panic!(
                "schema_version must be {}",
                crate::models::api_models::common::SCHEMA_VERSION
            );
        }
        if record.get("record_type") != Some(&serde_json::json!(RECORD_TYPE)) {
            panic!("record_type must be '{RECORD_TYPE}'");
        }
        Self::new(
            record
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("started_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("old_config_signature")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("target_config_signature")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("daemon_pid")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32)
                .unwrap_or(0),
            record
                .get("daemon_instance_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("generation")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32)
                .unwrap_or(0),
        )
    }
}

/// Persistent store for reload handoff records.
pub struct ReloadHandoffStore {
    layout: PathLayout,
    store: JsonStore,
}

impl ReloadHandoffStore {
    pub fn new(layout: &PathLayout) -> Self {
        Self {
            layout: layout.clone(),
            store: JsonStore::new(),
        }
    }

    pub fn load(&self) -> Result<Option<ReloadHandoff>, crate::DaemonError> {
        let path = self.layout.ccbrd_reload_handoff_path();
        if !path.exists() {
            return Ok(None);
        }
        let value: serde_json::Value = self.store.load(&path)?;
        if let serde_json::Value::Object(obj) = value {
            Ok(Some(ReloadHandoff::from_record(&obj)))
        } else {
            Ok(None)
        }
    }

    pub fn save(&self, handoff: &ReloadHandoff) -> Result<(), crate::DaemonError> {
        let path = self.layout.ccbrd_reload_handoff_path();
        self.store.save(&path, &handoff.to_record())?;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), crate::DaemonError> {
        let path = self.layout.ccbrd_reload_handoff_path();
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

/// Begin a reload handoff and return the record.
pub fn begin_reload_handoff(
    app: &mut CcbdApp,
    target_config_identity: &serde_json::Value,
) -> Option<ReloadHandoff> {
    let current = app.current_service_graph();
    let old_signature = clean_text(
        current
            .config_identity
            .get("config_signature")
            .and_then(|v| v.as_str()),
    );
    let target_signature = clean_text(
        target_config_identity
            .get("config_signature")
            .and_then(|v| v.as_str()),
    );
    let old_signature = old_signature?;
    let target_signature = target_signature?;
    if old_signature == target_signature {
        return None;
    }
    let lease = app.ownership.current();
    let daemon_pid = lease.map(|l| l.owner_pid).unwrap_or(0);
    let daemon_instance_id = lease.map(|l| l.instance_id.clone()).unwrap_or_default();
    let generation = lease.map(|l| l.generation).unwrap_or(0);
    if daemon_pid == 0 || daemon_instance_id.trim().is_empty() || generation == 0 {
        return None;
    }
    let handoff = ReloadHandoff::new(
        app.project_id().to_string(),
        chrono::Utc::now().to_rfc3339(),
        old_signature,
        target_signature,
        daemon_pid,
        daemon_instance_id,
        generation,
    );
    if let Err(e) = ReloadHandoffStore::new(&app.layout).save(&handoff) {
        tracing::warn!("failed to save reload handoff: {e}");
        return None;
    }
    Some(handoff)
}

/// Clear any in-progress reload handoff.
pub fn clear_reload_handoff(app: &mut CcbdApp) {
    let _ = ReloadHandoffStore::new(&app.layout).clear();
}

/// Check whether an active handoff allows a config signature mismatch.
pub fn reload_handoff_allows_signature_mismatch(
    app: &CcbdApp,
    expected_config_signature: &str,
    actual_config_signature: &str,
    now: Option<&str>,
) -> bool {
    let expected = clean_text(Some(expected_config_signature));
    let actual = clean_text(Some(actual_config_signature));
    let (expected, actual) = match (expected, actual) {
        (Some(e), Some(a)) => (e, a),
        _ => return false,
    };
    let handoff = match ReloadHandoffStore::new(&app.layout).load() {
        Ok(Some(h)) => h,
        _ => return false,
    };
    let now_owned = chrono::Utc::now().to_rfc3339();
    let now = now.unwrap_or(&now_owned);
    if !handoff_age_valid(&handoff, now) {
        return false;
    }
    if handoff.project_id != expected_project_id(app) {
        return false;
    }
    if handoff.old_config_signature != actual {
        return false;
    }
    if handoff.target_config_signature != expected {
        return false;
    }
    matches_current_holder(app, &handoff)
}

fn handoff_age_valid(handoff: &ReloadHandoff, now: &str) -> bool {
    let now_ts = match parse_utc_timestamp(now) {
        Some(ts) => ts,
        None => return false,
    };
    let started_ts = match parse_utc_timestamp(&handoff.started_at) {
        Some(ts) => ts,
        None => return false,
    };
    let age_s = (now_ts - started_ts).num_seconds() as f64;
    0.0 <= age_s && age_s <= handoff.ttl_s
}

fn matches_current_holder(app: &CcbdApp, handoff: &ReloadHandoff) -> bool {
    let Some(lease) = app.ownership.current() else {
        return false;
    };
    if lease.owner_pid != handoff.daemon_pid {
        return false;
    }
    if lease.instance_id != handoff.daemon_instance_id {
        return false;
    }
    if lease.generation != handoff.generation {
        return false;
    }
    true
}

fn expected_project_id(app: &CcbdApp) -> String {
    app.project_id().to_string()
}

fn clean_text(value: Option<&str>) -> Option<String> {
    let text = value.unwrap_or("").trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn parse_utc_timestamp(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::start_flow::service::StartFlowService;
    use crate::stop_flow::service::StopFlowService;
    use serde_json::json;
    use tempfile::TempDir;

    fn stub_app(dir: &TempDir) -> CcbdApp {
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        let socket_path = app.socket_path();
        let instance_id = app.daemon_instance_id().to_string();
        let _ = app
            .ownership
            .acquire(std::process::id(), &socket_path, &instance_id);
        app
    }

    #[test]
    fn test_reload_handoff_to_record_roundtrip() {
        let handoff = ReloadHandoff::new(
            "p1",
            "2024-01-01T00:00:00Z",
            "old-sig",
            "new-sig",
            42,
            "instance",
            7,
        );
        let record = handoff.to_record();
        let obj = record.as_object().unwrap();
        let loaded = ReloadHandoff::from_record(obj);
        assert_eq!(loaded.project_id, "p1");
        assert_eq!(loaded.daemon_pid, 42);
        assert_eq!(loaded.generation, 7);
        assert_eq!(loaded.status, "applying");
    }

    #[test]
    fn test_begin_reload_handoff_same_signature_returns_none() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        let graph = app.current_service_graph();
        let identity = graph.config_identity.clone();
        assert!(begin_reload_handoff(&mut app, &identity).is_none());
    }

    #[test]
    fn test_begin_reload_handoff_creates_handoff() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        let identity = json!({"config_signature": "target-sig"});
        let handoff = begin_reload_handoff(&mut app, &identity);
        assert!(handoff.is_some());
        let handoff = handoff.unwrap();
        assert_eq!(handoff.target_config_signature, "target-sig");
        assert!(handoff.daemon_pid > 0);
    }

    #[test]
    fn test_reload_handoff_store_save_load_clear() {
        let dir = TempDir::new().unwrap();
        let app = stub_app(&dir);
        let store = ReloadHandoffStore::new(&app.layout);
        assert!(store.load().unwrap().is_none());
        let handoff = ReloadHandoff::new("p1", "2024-01-01T00:00:00Z", "old", "new", 1, "inst", 1);
        store.save(&handoff).unwrap();
        let loaded = store.load().unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().target_config_signature, "new");
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn test_reload_handoff_allows_signature_mismatch() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        let identity = json!({"config_signature": "target-sig"});
        let handoff = begin_reload_handoff(&mut app, &identity).unwrap();
        let now = handoff.started_at.clone();
        assert!(reload_handoff_allows_signature_mismatch(
            &app,
            "target-sig",
            &handoff.old_config_signature,
            Some(&now)
        ));
        assert!(!reload_handoff_allows_signature_mismatch(
            &app,
            "wrong-target",
            &handoff.old_config_signature,
            Some(&now)
        ));
    }

    #[test]
    fn test_handoff_age_valid() {
        let handoff = ReloadHandoff::new("p1", "2024-01-01T00:00:00Z", "old", "new", 1, "inst", 1);
        assert!(handoff_age_valid(&handoff, "2024-01-01T00:00:30Z"));
        assert!(!handoff_age_valid(&handoff, "2024-01-01T00:02:00Z"));
        assert!(!handoff_age_valid(&handoff, "not-a-timestamp"));
    }
}
