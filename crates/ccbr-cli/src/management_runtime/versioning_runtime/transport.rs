//! Mirrors Python `lib/cli/management_runtime/versioning_runtime/transport.py`.

use serde_json::Value;
use std::process::Command;

/// Fetch JSON from a URL by shelling out to `curl`.
///
/// Mirrors Python `fetch_json_via_curl(url, timeout)`.
/// Returns `None` when curl is missing or the request fails.
pub fn fetch_json_via_curl(url: &str, timeout: f64) -> Option<Value> {
    let max_secs = if timeout.is_finite() && timeout > 0.0 {
        timeout as u64
    } else {
        10
    };
    let output = Command::new("curl")
        .args(["-fsSL", "--max-time", &max_secs.to_string(), url])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&text).ok()
}

// TODO: align `fetch_json_via_urllib` with Python once an HTTP client dependency
// (e.g. ureq/reqwest) is added to ccbr-cli. Rust stdlib has no HTTP client, so the
// urllib path cannot be implemented without an external crate.
