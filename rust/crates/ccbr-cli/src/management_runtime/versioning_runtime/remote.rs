//! Mirrors Python `lib/cli/management_runtime/versioning_runtime/remote.py`.

use serde_json::Value;

use super::constants::REMOTE_MAIN_COMMIT_API;
use super::transport::fetch_json_via_curl;

/// Fetch remote version info (commit + date) from the GitHub commit API.
///
/// Mirrors Python `get_remote_version_info()`. Returns `None` when the API is
/// unreachable or returns a non-dict payload.
pub fn get_remote_version_info() -> Option<Value> {
    let data = fetch_json_via_curl(REMOTE_MAIN_COMMIT_API, 10.0)?;
    let commit = data.get("sha").and_then(|v| v.as_str()).unwrap_or("");
    let short_commit: String = commit.chars().take(7).collect();
    let date_str = data
        .get("commit")
        .and_then(|c| c.get("committer"))
        .and_then(|c| c.get("date"))
        .and_then(|d| d.as_str())
        .unwrap_or("");
    let date: String = date_str.chars().take(10).collect();
    let date = if date.is_empty() { None } else { Some(date) };
    Some(serde_json::json!({
        "commit": short_commit,
        "date": date,
    }))
}
