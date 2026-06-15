//! Mirrors Python `lib/cli/management_runtime/commands_runtime/matching.py`.

/// Convert a dotted version string into a comparable tuple of integers.
///
/// Mirrors Python `version_key(version)`. Non-numeric segments are dropped.
pub fn version_key(version: &str) -> Vec<i64> {
    version
        .split('.')
        .filter(|part| part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty())
        .map(|part| part.parse::<i64>().unwrap_or(0))
        .collect()
}

/// Find the highest version whose leading segments match `target`.
///
/// Mirrors Python `find_matching_version(target, versions)`.
pub fn find_matching_version(target: &str, versions: &[String]) -> Option<String> {
    let target_parts: Vec<&str> = target.split('.').collect();
    let mut matching: Vec<String> = versions
        .iter()
        .filter(|version| {
            let version_parts: Vec<&str> = version.split('.').collect();
            version_parts.len() >= target_parts.len()
                && version_parts[..target_parts.len()] == target_parts[..]
        })
        .cloned()
        .collect();
    if matching.is_empty() {
        return None;
    }
    matching.sort_by(|a, b| version_key(b).cmp(&version_key(a)));
    matching.into_iter().next()
}

/// Return the highest version from the list.
///
/// Mirrors Python `latest_version(versions)`.
pub fn latest_version(versions: &[String]) -> Option<String> {
    let mut seen: Vec<String> = versions
        .iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();
    seen.sort_by(|a, b| version_key(b).cmp(&version_key(a)));
    seen.into_iter().next()
}

/// Return `true` when `candidate` is strictly newer than `current`.
///
/// Mirrors Python `is_newer_version(candidate, current)`.
pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    version_key(candidate) > version_key(current)
}
