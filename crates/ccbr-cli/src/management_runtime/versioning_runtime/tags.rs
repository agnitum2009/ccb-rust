//! Mirrors Python `lib/cli/management_runtime/versioning_runtime/tags.py`.

use serde_json::Value;
use std::collections::HashSet;
use std::process::Command;

use super::constants::{REMOTE_TAGS_API, REPO_URL};
use super::transport::fetch_json_via_curl;

/// Return `true` when `s` matches `^\d+(\.\d+)*$` (dotted-numeric version).
fn is_dotted_numeric(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut digits_seen = false;
    for segment in s.split('.') {
        if segment.is_empty() || !segment.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        digits_seen = true;
    }
    digits_seen
}

/// Fetch the list of available release versions.
///
/// Mirrors Python `get_available_versions(urllib_timeout, curl_timeout, git_timeout)`.
/// Tries the curl transport first, then falls back to `git ls-remote`.
pub fn get_available_versions(curl_timeout: f64, git_timeout: f64) -> Vec<String> {
    let mut versions: Vec<String> = Vec::new();

    if let Some(Value::Array(data)) = fetch_json_via_curl(REMOTE_TAGS_API, curl_timeout) {
        versions = parse_api_response(&data);
    }
    if versions.is_empty() && which_git() {
        if let Ok(output) = Command::new("git")
            .args(["ls-remote", "--tags", REPO_URL])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let _ = git_timeout;
                versions = parse_git_refs(&text);
            }
        }
    }
    versions
}

/// Parse the GitHub tags API response into a list of version strings.
///
/// Mirrors Python `parse_api_response(data)`.
pub fn parse_api_response(data: &[Value]) -> Vec<String> {
    let mut result = Vec::new();
    for tag in data {
        let name = tag.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let name = name.strip_prefix('v').unwrap_or(name);
        if is_dotted_numeric(name) {
            result.push(name.to_string());
        }
    }
    result
}

/// Parse `git ls-remote --tags` output into a deduplicated list of version strings.
///
/// Mirrors Python `parse_git_refs(output)`.
pub fn parse_git_refs(output: &str) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    for line in output.trim().split('\n') {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let ref_ = parts[1];
            if let Some(rest) = ref_.strip_prefix("refs/tags/v") {
                let name = rest.trim_end_matches("^{}");
                if is_dotted_numeric(name) {
                    seen.insert(name.to_string());
                }
            }
        }
    }
    seen.into_iter().collect()
}

fn which_git() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
