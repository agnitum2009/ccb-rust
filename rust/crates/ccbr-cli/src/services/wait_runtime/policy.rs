//! Mirrors Python `lib/cli/services/wait_runtime/policy.py`.

use crate::models::ParsedWaitCommand;

const DEFAULT_TIMEOUT_S: f64 = 30.0;
const DEFAULT_POLL_INTERVAL_S: f64 = 0.1;
const MIN_TIMEOUT_S: f64 = 0.1;
const MIN_POLL_INTERVAL_S: f64 = 0.01;

/// Resolve the effective wait timeout.
pub fn resolve_timeout(explicit: Option<f64>) -> f64 {
    if let Some(value) = explicit {
        return value.max(MIN_TIMEOUT_S);
    }
    if let Ok(raw) = std::env::var("CCBR_WAIT_TIMEOUT_S") {
        if let Ok(value) = raw.parse::<f64>() {
            return value.max(MIN_TIMEOUT_S);
        }
    }
    DEFAULT_TIMEOUT_S
}

/// Resolve the polling interval between trace calls.
pub fn resolve_poll_interval() -> f64 {
    if let Ok(raw) = std::env::var("CCBR_WAIT_POLL_INTERVAL_S") {
        if let Ok(value) = raw.parse::<f64>() {
            return value.max(MIN_POLL_INTERVAL_S);
        }
    }
    DEFAULT_POLL_INTERVAL_S
}

/// Resolve the required number of replies for the wait mode.
pub fn resolve_quorum(command: &ParsedWaitCommand, expected_count: usize) -> usize {
    if command.mode == "quorum" {
        let quorum = command.quorum.unwrap_or(0).max(0) as usize;
        if quorum > expected_count {
            panic!(
                "wait quorum {quorum} exceeds available reply routes {expected_count} for target {}",
                command.target
            );
        }
        return quorum;
    }
    if command.mode == "any" {
        1
    } else {
        expected_count
    }
}
