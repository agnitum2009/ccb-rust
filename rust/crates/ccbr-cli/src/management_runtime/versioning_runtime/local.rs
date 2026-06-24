//! Mirrors Python `lib/cli/management_runtime/versioning_runtime/local.py`.

use serde_json::{Map, Value};
use std::fs;
use std::path::Path;
use std::process::Command;

const BUILD_INFO_KEYS: &[&str] = &[
    "version",
    "commit",
    "date",
    "build_time",
    "platform",
    "arch",
    "channel",
    "source_kind",
    "install_mode",
    "installed_at",
    "install_user_id",
    "install_user_name",
    "root_install",
    "sudo_user",
];

fn empty_version_info() -> Map<String, Value> {
    let mut map = Map::new();
    for key in [
        "commit",
        "date",
        "version",
        "build_time",
        "platform",
        "arch",
        "channel",
        "source_kind",
        "install_mode",
        "installed_at",
        "install_user_id",
        "install_user_name",
        "root_install",
        "sudo_user",
    ] {
        map.insert(key.to_string(), Value::Null);
    }
    map
}

fn merge(into: &mut Map<String, Value>, from: Map<String, Value>) {
    for (k, v) in from {
        into.insert(k, v);
    }
}

/// Gather local version info from build artifacts, version file, and git.
///
/// Mirrors Python `get_version_info(dir_path)`.
pub fn get_version_info(dir_path: &Path) -> Map<String, Value> {
    let mut info = empty_version_info();
    merge(
        &mut info,
        read_build_info(&dir_path.join("BUILD_INFO.json")),
    );
    merge(&mut info, read_version_file(&dir_path.join("VERSION")));
    merge(&mut info, read_embedded_version_info(&dir_path.join("ccb")));
    if let Some(git) = git_version_info(dir_path) {
        merge(&mut info, git);
    }
    normalize_installation_info(&mut info, dir_path);
    info
}

/// Parse embedded version assignments from the `ccb` launcher script.
///
/// Mirrors Python `read_embedded_version_info(ccbr_file)`.
pub fn read_embedded_version_info(ccbr_file: &Path) -> Map<String, Value> {
    let content = match fs::read_to_string(ccbr_file) {
        Ok(c) => c,
        Err(_) => return Map::new(),
    };
    let mut info: Map<String, Value> = Map::new();
    for line in content.split('\n').take(60) {
        let (key, value) = version_assignment(line);
        if let (Some(k), Some(v)) = (key, value) {
            info.insert(k.to_string(), Value::String(v));
        }
    }
    info
}

/// Extract a `(normalized_key, value)` pair from a `KEY="value"` assignment line.
///
/// Mirrors Python `version_assignment(line)`.
pub fn version_assignment(line: &str) -> (Option<&'static str>, Option<String>) {
    let text = line.trim();
    let eq = match text.find('=') {
        Some(i) => i,
        None => return (None, None),
    };
    let name = text[..eq].trim();
    let raw_value = &text[eq + 1..];
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
    let value_opt = if value.is_empty() { None } else { Some(value) };
    (key, value_opt)
}

/// Read a bare version string from a `VERSION` file.
///
/// Mirrors Python `read_version_file(version_file)`.
pub fn read_version_file(version_file: &Path) -> Map<String, Value> {
    let mut map = Map::new();
    let value = match fs::read_to_string(version_file) {
        Ok(v) => v.trim().to_string(),
        Err(_) => return map,
    };
    if !value.is_empty() {
        map.insert("version".to_string(), Value::String(value));
    }
    map
}

/// Read normalized fields from a `BUILD_INFO.json` file.
///
/// Mirrors Python `read_build_info(build_info_file)`.
pub fn read_build_info(build_info_file: &Path) -> Map<String, Value> {
    let mut normalized: Map<String, Value> = Map::new();
    let content = match fs::read_to_string(build_info_file) {
        Ok(c) => c,
        Err(_) => return normalized,
    };
    let payload: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return normalized,
    };
    let obj = match payload.as_object() {
        Some(o) => o,
        None => return normalized,
    };
    for &key in BUILD_INFO_KEYS {
        let value = obj.get(key);
        let entry = if key == "root_install" {
            value.and_then(|v| v.as_bool()).map(Value::Bool)
        } else {
            value.and_then(|v| match v {
                Value::Null => None,
                _ => {
                    let s = match v {
                        Value::String(s) => s.trim().to_string(),
                        other => serde_json::to_string(other)
                            .unwrap_or_default()
                            .trim()
                            .to_string(),
                    };
                    if s.is_empty() {
                        None
                    } else {
                        Some(Value::String(s))
                    }
                }
            })
        };
        normalized.insert(key.to_string(), entry.unwrap_or(Value::Null));
    }
    normalized
}

/// Run `git log` to extract commit + date for a source checkout.
///
/// Mirrors Python `git_version_info(dir_path)`.
pub fn git_version_info(dir_path: &Path) -> Option<Map<String, Value>> {
    if !dir_path.join(".git").exists() {
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
    let stdout = stdout.trim();
    if stdout.is_empty() {
        return None;
    }
    let parts: Vec<&str> = stdout.split('|').collect();
    if parts.len() < 2 {
        return None;
    }
    let mut map = Map::new();
    map.insert("commit".to_string(), Value::String(parts[0].to_string()));
    let date = parts[1].split_whitespace().next().unwrap_or("");
    map.insert("date".to_string(), Value::String(date.to_string()));
    Some(map)
}

/// Fill in `install_mode` / `source_kind` / `channel` defaults.
///
/// Mirrors Python `normalize_installation_info(info, dir_path)`.
pub fn normalize_installation_info(info: &mut Map<String, Value>, dir_path: &Path) {
    let is_source = dir_path.join(".git").exists();
    let default_kind = if is_source { "source" } else { "release" };
    if info
        .get("install_mode")
        .map(|v| v.is_null())
        .unwrap_or(true)
    {
        info.insert(
            "install_mode".to_string(),
            Value::String(default_kind.to_string()),
        );
    }
    if info.get("source_kind").map(|v| v.is_null()).unwrap_or(true) {
        info.insert(
            "source_kind".to_string(),
            Value::String(default_kind.to_string()),
        );
    }
    if info.get("channel").map(|v| v.is_null()).unwrap_or(true) {
        if is_source {
            info.insert("channel".to_string(), Value::String("dev".to_string()));
        } else {
            info.insert("channel".to_string(), Value::Null);
        }
    }
}

/// Render a human-readable version string from version info.
///
/// Mirrors Python `format_version_info(info)`.
pub fn format_version_info(info: &Map<String, Value>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(v) = info.get("version").and_then(|v| v.as_str()) {
        if !v.is_empty() {
            parts.push(format!("v{}", v));
        }
    }
    if let Some(c) = info.get("commit").and_then(|v| v.as_str()) {
        if !c.is_empty() {
            parts.push(c.to_string());
        }
    }
    if let Some(d) = info.get("date").and_then(|v| v.as_str()) {
        if !d.is_empty() {
            parts.push(d.to_string());
        }
    }
    if parts.is_empty() {
        "unknown".to_string()
    } else {
        parts.join(" ")
    }
}
