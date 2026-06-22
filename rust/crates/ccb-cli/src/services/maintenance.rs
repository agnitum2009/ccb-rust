//! Mirrors Python `lib/cli/services/maintenance.py`.

use crate::context::CliContext;

/// Stop the maintenance heartbeat runner. Currently a no-op stub.
pub fn stop_maintenance_heartbeat_runner(_context: &CliContext, _reason: &str) {}
