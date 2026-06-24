use super::api_models::common::{LeaseHealth, MountState, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcbdLease {
    pub project_id: String,
    pub ccbd_pid: u32,
    pub socket_path: String,
    pub owner_uid: u32,
    pub boot_id: String,
    pub started_at: String,
    pub last_heartbeat_at: String,
    pub mount_state: MountState,
    #[serde(default = "default_generation")]
    pub generation: u32,
    #[serde(default)]
    pub config_signature: Option<String>,
    #[serde(default)]
    pub keeper_pid: Option<u32>,
    #[serde(default)]
    pub daemon_instance_id: Option<String>,
    pub api_version: u32,
}

fn default_generation() -> u32 {
    1
}

impl CcbdLease {
    pub fn with_heartbeat(&self, timestamp: &str) -> Self {
        let mut clone = self.clone();
        clone.last_heartbeat_at = timestamp.into();
        clone
    }

    pub fn with_mount_state(&self, state: MountState, heartbeat_at: &str) -> Self {
        let mut clone = self.clone();
        clone.mount_state = state;
        clone.last_heartbeat_at = heartbeat_at.into();
        clone
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "ccbd_lease",
            "api_version": self.api_version,
            "project_id": self.project_id,
            "ccbd_pid": self.ccbd_pid,
            "socket_path": self.socket_path,
            "owner_uid": self.owner_uid,
            "boot_id": self.boot_id,
            "started_at": self.started_at,
            "last_heartbeat_at": self.last_heartbeat_at,
            "mount_state": self.mount_state,
            "generation": self.generation,
            "config_signature": self.config_signature,
            "keeper_pid": self.keeper_pid,
            "daemon_instance_id": self.daemon_instance_id,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseInspection {
    pub lease: Option<CcbdLease>,
    pub health: LeaseHealth,
    pub pid_alive: bool,
    pub socket_connectable: bool,
    pub heartbeat_fresh: bool,
    pub takeover_allowed: bool,
    pub reason: String,
}

impl LeaseInspection {
    pub fn generation(&self) -> Option<u32> {
        self.lease.as_ref().map(|l| l.generation)
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "health": self.health,
            "pid_alive": self.pid_alive,
            "socket_connectable": self.socket_connectable,
            "heartbeat_fresh": self.heartbeat_fresh,
            "takeover_allowed": self.takeover_allowed,
            "reason": self.reason,
            "lease": self.lease.as_ref().map(|l| l.to_record()),
        })
    }
}
