use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 2;
pub const API_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Accepted,
    Queued,
    Running,
    Completed,
    Cancelled,
    Failed,
    Incomplete,
}

impl JobStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Failed | Self::Incomplete
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryScope {
    Single,
    Broadcast,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    #[default]
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MountState {
    Mounted,
    Unmounted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeaseHealth {
    Healthy,
    Degraded,
    Stale,
    Unmounted,
    Missing,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct CcbdModelError(pub String);
