//! Mirrors Python `lib/ccbrd/services/health_runtime.py`.
//! Re-export shim: forwards to the actual health assessment implementation.

pub use crate::services::health::{HealthMonitor, HealthState};
