//! Remote version discovery for self-update.
//!
//! Mirrors Python `cli.management_runtime.versioning_runtime.tags`. Resolves the
//! set of published versions from the GitHub tags API (via `curl`) with a
//! `git ls-remote` fallback, parsing both the API JSON and the git ref output.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

/// GitHub tags API endpoint.
pub const REMOTE_TAGS_API: &str = "https://api.github.com/repos/bfly123/claude_code_bridge/tags";

/// GitHub main-branch commit API endpoint.
pub const REMOTE_MAIN_COMMIT_API: &str =
    "https://api.github.com/repos/bfly123/claude_code_bridge/commits/main";

/// Upstream repository URL (clone target for `git ls-remote`).
pub const REPO_URL: &str = "https://github.com/bfly123/claude_code_bridge";

const DEFAULT_CURL_TIMEOUT: u64 = 15;
const DEFAULT_GIT_TIMEOUT: u64 = 30;
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Fetch the set of published versions.
///
/// Mirrors `versioning_runtime.tags.get_available_versions`. Tries the GitHub
/// tags API via `curl`, then falls back to `git ls-remote --tags`.
pub fn get_available_versions() -> Vec<String> {
    let mut versions = fetch_versions_via_curl();
    if versions.is_empty() {
        versions = fetch_versions_via_git();
    }
    versions
}

fn fetch_versions_via_curl() -> Vec<String> {
    let Some(data) = fetch_json_via_curl(REMOTE_TAGS_API, DEFAULT_CURL_TIMEOUT) else {
        return Vec::new();
    };
    match data {
        Value::Array(items) => parse_api_response(&items),
        _ => Vec::new(),
    }
}

/// Fetch and parse a JSON document from `url` via `curl`.
///
/// Mirrors `versioning_runtime.transport.fetch_json_via_curl`. Returns `None`
/// when `curl` is absent, exits non-zero, or the body is not valid JSON.
pub fn fetch_json_via_curl(url: &str, timeout_s: u64) -> Option<Value> {
    which("curl")?;
    let stdout = run_with_timeout("curl", &["-fsSL".to_string(), url.to_string()], timeout_s)?;
    serde_json::from_str::<Value>(stdout.trim()).ok()
}

/// Fetch the upstream main-branch commit and date.
///
/// Mirrors `versioning_runtime.remote.get_remote_version_info`. Returns
/// `(commit_prefix, date)` where `commit_prefix` is the first 7 chars of the
/// commit SHA and `date` is the committer date (`YYYY-MM-DD`) or `None`.
pub fn get_remote_version_info() -> Option<(String, Option<String>)> {
    let data = fetch_json_via_curl(REMOTE_MAIN_COMMIT_API, 10)?;
    extract_remote_info(&data)
}

/// Extract `(commit_prefix, date)` from a GitHub commit API payload.
///
/// Split out from `get_remote_version_info` so the parsing is unit-testable.
pub fn extract_remote_info(data: &Value) -> Option<(String, Option<String>)> {
    let sha = data.get("sha").and_then(|v| v.as_str()).unwrap_or("");
    if sha.is_empty() {
        return None;
    }
    let commit = sha.chars().take(7).collect::<String>();
    let date_raw = data
        .get("commit")
        .and_then(|c| c.get("committer"))
        .and_then(|c| c.get("date"))
        .and_then(|d| d.as_str())
        .unwrap_or("");
    let date = if date_raw.is_empty() {
        None
    } else {
        Some(date_raw.chars().take(10).collect::<String>())
    };
    Some((commit, date))
}

fn fetch_versions_via_git() -> Vec<String> {
    if which("git").is_none() {
        return Vec::new();
    }
    let output = run_with_timeout(
        "git",
        &[
            "ls-remote".to_string(),
            "--tags".to_string(),
            REPO_URL.to_string(),
        ],
        DEFAULT_GIT_TIMEOUT,
    );
    let Some(stdout) = output else {
        return Vec::new();
    };
    parse_git_refs(&stdout)
}

/// Parse a GitHub tags API response into a list of bare version strings.
///
/// Mirrors `versioning_runtime.tags.parse_api_response`.
pub fn parse_api_response(data: &[Value]) -> Vec<String> {
    let mut result = Vec::new();
    for tag in data {
        let name = tag.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let name = name.strip_prefix('v').unwrap_or(name);
        if is_numeric_version(name) {
            result.push(name.to_string());
        }
    }
    result
}

/// Parse `git ls-remote --tags` output into a deduplicated list of versions.
///
/// Mirrors `versioning_runtime.tags.parse_git_refs`.
pub fn parse_git_refs(output: &str) -> Vec<String> {
    let mut result = Vec::new();
    for line in output.trim().split('\n') {
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let _hash = parts.next();
        let Some(reference) = parts.next() else {
            continue;
        };
        if let Some(rest) = reference.strip_prefix("refs/tags/v") {
            let name = rest.trim_end_matches("^{}");
            if is_numeric_version(name) {
                result.push(name.to_string());
            }
        }
    }
    result.sort();
    result.dedup();
    result
}

/// Return true for bare version strings like `1`, `1.2`, `1.2.3`.
///
/// Mirrors the regex `^\d+(\.\d+)*$` from `tags.py` without a regex dependency.
fn is_numeric_version(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut last_was_dot = true; // leading '.' is invalid
    for ch in name.chars() {
        if ch.is_ascii_digit() {
            last_was_dot = false;
        } else if ch == '.' {
            if last_was_dot {
                return false; // leading or consecutive '.'
            }
            last_was_dot = true;
        } else {
            return false;
        }
    }
    !last_was_dot
}

/// Run a command, capturing stdout, with a wall-clock timeout.
fn run_with_timeout(program: &str, args: &[String], timeout_s: u64) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(timeout_s);
    let status = loop {
        if let Ok(Some(status)) = child.try_wait() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    if !status.success() {
        return None;
    }
    use std::io::Read;
    let mut buf = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_string(&mut buf);
    }
    Some(buf)
}

fn which(program: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if let Ok(meta) = std::fs::metadata(&candidate) {
            if meta.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Local version info — mirrors `versioning_runtime.local`.
// ---------------------------------------------------------------------------

/// Resolved installation/version metadata for a CCB install directory.
///
/// Mirrors the dict returned by `local.get_version_info`.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VersionInfo {
    pub commit: Option<String>,
    pub date: Option<String>,
    pub version: Option<String>,
    pub build_time: Option<String>,
    pub platform: Option<String>,
    pub arch: Option<String>,
    pub channel: Option<String>,
    pub source_kind: Option<String>,
    pub install_mode: Option<String>,
    pub installed_at: Option<String>,
    pub install_user_id: Option<String>,
    pub install_user_name: Option<String>,
    pub root_install: Option<bool>,
    pub sudo_user: Option<String>,
}

impl VersionInfo {
    /// Merge another info record in place, letting `other` override set fields.
    fn merge(&mut self, other: VersionInfo) {
        macro_rules! take {
            ($f:ident) => {
                if other.$f.is_some() {
                    self.$f = other.$f;
                }
            };
        }
        take!(commit);
        take!(date);
        take!(version);
        take!(build_time);
        take!(platform);
        take!(arch);
        take!(channel);
        take!(source_kind);
        take!(install_mode);
        take!(installed_at);
        take!(install_user_id);
        take!(install_user_name);
        take!(root_install);
        take!(sudo_user);
    }

    /// Fill `install_mode`/`source_kind`/`channel` defaults from the install dir.
    ///
    /// Mirrors `local.normalize_installation_info`.
    fn normalize(&mut self, dir_path: &Path) {
        let is_source = dir_path.join(".git").exists();
        if self.install_mode.is_none() {
            self.install_mode = Some(if is_source { "source" } else { "release" }.into());
        }
        if self.source_kind.is_none() {
            self.source_kind = Some(if is_source { "source" } else { "release" }.into());
        }
        if self.channel.is_none() && is_source {
            self.channel = Some("dev".into());
        }
    }
}

/// Build the version info for an install directory.
///
/// Mirrors `local.get_version_info`.
pub fn get_version_info(dir_path: &Path) -> VersionInfo {
    let mut info = VersionInfo::default();
    info.merge(read_build_info(&dir_path.join("BUILD_INFO.json")));
    if let Some(version) = read_version_file(&dir_path.join("VERSION")) {
        info.version = Some(version);
    }
    info.merge(read_embedded_version_info(&dir_path.join("ccb")));
    if let Some(git) = git_version_info(dir_path) {
        info.commit = Some(git.0);
        info.date = Some(git.1);
    }
    info.normalize(dir_path);
    info
}

/// Format a version info record for display, e.g. `v1.2.3 abc1234 2026-06-14`.
///
/// Mirrors `local.format_version_info`.
pub fn format_version_info(info: &VersionInfo) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(version) = &info.version {
        parts.push(format!("v{version}"));
    }
    if let Some(commit) = &info.commit {
        parts.push(commit.clone());
    }
    if let Some(date) = &info.date {
        parts.push(date.clone());
    }
    if parts.is_empty() {
        "unknown".to_string()
    } else {
        parts.join(" ")
    }
}

/// Extract a single `KEY=value` assignment from an embedded-binary line.
///
/// Mirrors `local.version_assignment`. Returns `(normalized_key, value)`.
fn version_assignment(line: &str) -> (Option<&'static str>, Option<String>) {
    let text = line.trim();
    if !text.contains('=') {
        return (None, None);
    }
    let mut split = text.splitn(2, '=');
    let name = split.next().unwrap_or("").trim();
    let raw_value = split.next().unwrap_or("");
    let value = raw_value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();
    let key = match name {
        "VERSION" => Some("version"),
        "GIT_COMMIT" => Some("commit"),
        "GIT_DATE" => Some("date"),
        _ => None,
    };
    (key, if value.is_empty() { None } else { Some(value) })
}

/// Read embedded version assignments from the first 60 lines of `ccb`.
///
/// Mirrors `local.read_embedded_version_info`.
fn read_embedded_version_info(ccb_file: &Path) -> VersionInfo {
    let mut info = VersionInfo::default();
    let content = match std::fs::read_to_string(ccb_file) {
        Ok(c) => c,
        Err(_) => return info,
    };
    for line in content.split('\n').take(60) {
        let (key, value) = version_assignment(line);
        let (Some(key), Some(value)) = (key, value) else {
            continue;
        };
        match key {
            "version" => info.version = Some(value),
            "commit" => info.commit = Some(value),
            "date" => info.date = Some(value),
            _ => {}
        }
    }
    info
}

/// Read the bare version string from a `VERSION` file.
///
/// Mirrors `local.read_version_file`.
fn read_version_file(version_file: &Path) -> Option<String> {
    let value = std::fs::read_to_string(version_file).ok()?;
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Read normalized fields from `BUILD_INFO.json`.
///
/// Mirrors `local.read_build_info`. `root_install` stays a bool; other keys are
/// stringified and empty values become `None`.
fn read_build_info(build_info_file: &Path) -> VersionInfo {
    let mut info = VersionInfo::default();
    let text = match std::fs::read_to_string(build_info_file) {
        Ok(t) => t,
        Err(_) => return info,
    };
    let payload: Value = match serde_json::from_str(&text) {
        Ok(Value::Object(map)) => Value::Object(map),
        _ => return info,
    };
    for (key, field) in [
        ("version", "version"),
        ("commit", "commit"),
        ("date", "date"),
        ("build_time", "build_time"),
        ("platform", "platform"),
        ("arch", "arch"),
        ("channel", "channel"),
        ("source_kind", "source_kind"),
        ("install_mode", "install_mode"),
        ("installed_at", "installed_at"),
        ("install_user_id", "install_user_id"),
        ("install_user_name", "install_user_name"),
        ("sudo_user", "sudo_user"),
    ] {
        let value = payload.get(key);
        let Some(value) = value else { continue };
        let normalized = match value {
            Value::Null => None,
            other => {
                let s = match other {
                    Value::String(s) => s.clone(),
                    _ => other.to_string(),
                };
                let s = s.trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            }
        };
        match field {
            "version" => info.version = normalized,
            "commit" => info.commit = normalized,
            "date" => info.date = normalized,
            "build_time" => info.build_time = normalized,
            "platform" => info.platform = normalized,
            "arch" => info.arch = normalized,
            "channel" => info.channel = normalized,
            "source_kind" => info.source_kind = normalized,
            "install_mode" => info.install_mode = normalized,
            "installed_at" => info.installed_at = normalized,
            "install_user_id" => info.install_user_id = normalized,
            "install_user_name" => info.install_user_name = normalized,
            "sudo_user" => info.sudo_user = normalized,
            _ => {}
        }
    }
    if let Some(Value::Bool(b)) = payload.get("root_install") {
        info.root_install = Some(*b);
    }
    info
}

/// Read commit/date from the local git checkout, if present.
///
/// Mirrors `local.git_version_info`. Returns `(commit, date)`.
fn git_version_info(dir_path: &Path) -> Option<(String, String)> {
    if which("git").is_none() || !dir_path.join(".git").exists() {
        return None;
    }
    let output = Command::new("git")
        .args([
            "-C",
            &dir_path.to_string_lossy(),
            "log",
            "-1",
            "--format=%h|%ci",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parts = trimmed.splitn(2, '|');
    let commit = parts.next()?.to_string();
    let date_raw = parts.next()?;
    if commit.is_empty() || date_raw.is_empty() {
        return None;
    }
    let date = date_raw.split_whitespace().next().unwrap_or(date_raw);
    Some((commit, date.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_api_response_strips_v_prefix() {
        let data = vec![
            serde_json::json!({"name": "v1.2.3"}),
            serde_json::json!({"name": "1.0.0"}),
            serde_json::json!({"name": "v2.0"}),
        ];
        assert_eq!(parse_api_response(&data), vec!["1.2.3", "1.0.0", "2.0"]);
    }

    #[test]
    fn test_parse_api_response_filters_non_numeric() {
        let data = vec![
            serde_json::json!({"name": "v1.2.3"}),
            serde_json::json!({"name": "nightly"}),
            serde_json::json!({"name": "v1.2.3-rc1"}),
            serde_json::json!({}),
        ];
        assert_eq!(parse_api_response(&data), vec!["1.2.3"]);
    }

    #[test]
    fn test_parse_git_refs_basic() {
        let output = "\
abc123\trefs/tags/v1.0.0
def456\trefs/tags/v1.1.0
ghi789\trefs/tags/v1.0.0^{}
jkl012\trefs/heads/main
";
        let mut versions = parse_git_refs(output);
        versions.sort();
        assert_eq!(versions, vec!["1.0.0", "1.1.0"]);
    }

    #[test]
    fn test_parse_git_refs_filters_non_numeric() {
        let output = "abc\trefs/tags/v1.2.3\ndef\trefs/tags/nightly\n";
        assert_eq!(parse_git_refs(output), vec!["1.2.3"]);
    }

    #[test]
    fn test_is_numeric_version() {
        assert!(is_numeric_version("1"));
        assert!(is_numeric_version("1.2"));
        assert!(is_numeric_version("1.2.3"));
        assert!(is_numeric_version("12.0.1"));
        assert!(!is_numeric_version(""));
        assert!(!is_numeric_version(".1"));
        assert!(!is_numeric_version("1."));
        assert!(!is_numeric_version("1..2"));
        assert!(!is_numeric_version("1.2a"));
        assert!(!is_numeric_version("v1.2.3"));
        assert!(!is_numeric_version("1.2.3-rc1"));
    }

    #[test]
    fn test_version_assignment() {
        assert_eq!(
            version_assignment(r#"VERSION="1.2.3""#),
            (Some("version"), Some("1.2.3".into()))
        );
        assert_eq!(
            version_assignment("GIT_COMMIT=abc123"),
            (Some("commit"), Some("abc123".into()))
        );
        assert_eq!(
            version_assignment("GIT_DATE='2026-06-14'"),
            (Some("date"), Some("2026-06-14".into()))
        );
        assert_eq!(version_assignment("UNKNOWN=1"), (None, Some("1".into())));
        assert_eq!(version_assignment("no equals here"), (None, None));
        assert_eq!(version_assignment("VERSION="), (Some("version"), None));
    }

    #[test]
    fn test_format_version_info() {
        let mut info = VersionInfo::default();
        assert_eq!(format_version_info(&info), "unknown");
        info.version = Some("1.2.3".into());
        assert_eq!(format_version_info(&info), "v1.2.3");
        info.commit = Some("abc1234".into());
        info.date = Some("2026-06-14".into());
        assert_eq!(format_version_info(&info), "v1.2.3 abc1234 2026-06-14");
    }

    #[test]
    fn test_read_version_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("VERSION");
        std::fs::write(&path, "7.5.2\n").unwrap();
        assert_eq!(read_version_file(&path), Some("7.5.2".into()));
        std::fs::write(&path, "   \n").unwrap();
        assert_eq!(read_version_file(&path), None);
        assert_eq!(read_version_file(Path::new("/nonexistent/VERSION")), None);
    }

    #[test]
    fn test_read_build_info() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("BUILD_INFO.json");
        std::fs::write(
            &path,
            r#"{"version":"1.0.0","platform":"linux","arch":"x86_64","root_install":true,"empty":""}"#,
        )
        .unwrap();
        let info = read_build_info(&path);
        assert_eq!(info.version.as_deref(), Some("1.0.0"));
        assert_eq!(info.platform.as_deref(), Some("linux"));
        assert_eq!(info.arch.as_deref(), Some("x86_64"));
        assert_eq!(info.root_install, Some(true));
        assert!(info.commit.is_none());
        assert_eq!(
            read_build_info(Path::new("/nonexistent/build.json")),
            VersionInfo::default()
        );
    }

    #[test]
    fn test_read_build_info_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("BUILD_INFO.json");
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(read_build_info(&path), VersionInfo::default());
    }

    #[test]
    fn test_read_embedded_version_info() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ccb");
        std::fs::write(
            &path,
            "shebang line\nVERSION=\"2.0.0\"\nGIT_COMMIT=deadbee\nGIT_DATE=2026-01-01\nother\n",
        )
        .unwrap();
        let info = read_embedded_version_info(&path);
        assert_eq!(info.version.as_deref(), Some("2.0.0"));
        assert_eq!(info.commit.as_deref(), Some("deadbee"));
        assert_eq!(info.date.as_deref(), Some("2026-01-01"));
    }

    #[test]
    fn test_get_version_info_normalizes_release_install() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("VERSION"), "3.1.0\n").unwrap();
        std::fs::write(
            dir.path().join("BUILD_INFO.json"),
            r#"{"platform":"linux","arch":"aarch64","root_install":false}"#,
        )
        .unwrap();
        let info = get_version_info(dir.path());
        assert_eq!(info.version.as_deref(), Some("3.1.0"));
        assert_eq!(info.platform.as_deref(), Some("linux"));
        assert_eq!(info.arch.as_deref(), Some("aarch64"));
        assert_eq!(info.root_install, Some(false));
        // No .git dir => release install defaults.
        assert_eq!(info.install_mode.as_deref(), Some("release"));
        assert_eq!(info.source_kind.as_deref(), Some("release"));
        assert!(info.channel.is_none());
    }

    #[test]
    fn test_get_version_info_source_install_channel() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        let info = get_version_info(dir.path());
        assert_eq!(info.install_mode.as_deref(), Some("source"));
        assert_eq!(info.channel.as_deref(), Some("dev"));
    }

    #[test]
    fn test_extract_remote_info_parses_commit_and_date() {
        let payload = serde_json::json!({
            "sha": "abcdef1234567890",
            "commit": {"committer": {"date": "2026-06-14T10:30:00Z"}}
        });
        let (commit, date) = extract_remote_info(&payload).unwrap();
        assert_eq!(commit, "abcdef1");
        assert_eq!(date.as_deref(), Some("2026-06-14"));
    }

    #[test]
    fn test_extract_remote_info_missing_sha_returns_none() {
        let payload = serde_json::json!({"commit": {"committer": {"date": "2026-06-14"}}});
        assert!(extract_remote_info(&payload).is_none());
    }

    #[test]
    fn test_extract_remote_info_missing_date() {
        let payload = serde_json::json!({"sha": "deadbee"});
        let (commit, date) = extract_remote_info(&payload).unwrap();
        assert_eq!(commit, "deadbee");
        assert!(date.is_none());
    }
}
