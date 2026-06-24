use std::path::{Path, PathBuf};

use camino::Utf8Path;
use ccbr_provider_core::pathing::session_filename_for_agent;
use ccbr_storage::path_helpers::runtime_project_anchor_from_path;
use serde_json::{Map, Value};

/// Find the project `.ccbr` directory for a runtime directory.
///
/// First walks up the filesystem looking for a directory named `.ccbr`. If none
/// is found, falls back to the relocated runtime anchor marker via
/// `runtime_project_anchor_from_path`.
pub fn find_project_ccbr_dir(runtime_dir: impl AsRef<Path>) -> Option<PathBuf> {
    let runtime_dir = runtime_dir.as_ref();
    for parent in runtime_dir.ancestors() {
        match parent.file_name() {
            Some(name) if name == ".ccbr" => return Some(parent.to_path_buf()),
            Some(_) => continue,
            None => break,
        }
    }
    let utf8_path = Utf8Path::from_path(runtime_dir)?;
    runtime_project_anchor_from_path(utf8_path).map(|p| p.as_std_path().to_path_buf())
}

/// Build the provider session file path for a runtime directory.
///
/// Expects `runtime_dir` to look like `.../agents/<agent>/provider-runtime/<provider>`.
/// The agent name is taken from the grandparent directory of `runtime_dir`.
pub fn session_file_for_runtime_dir(
    provider: &str,
    runtime_dir: impl AsRef<Path>,
) -> Option<PathBuf> {
    let ccbr_dir = find_project_ccbr_dir(&runtime_dir)?;
    let runtime_dir = runtime_dir.as_ref();
    let agent_name = runtime_dir.parent()?.parent()?.file_name()?.to_str()?;
    let filename = session_filename_for_agent(provider, agent_name).ok()?;
    Some(ccbr_dir.join(filename))
}

/// Build the provider state directory path for a runtime directory.
///
/// Expects `runtime_dir` to look like `.../agents/<agent>/provider-runtime/<provider>`.
pub fn state_dir_for_runtime_dir(runtime_dir: impl AsRef<Path>) -> Option<PathBuf> {
    let runtime_dir = runtime_dir.as_ref();
    let provider = runtime_dir.file_name()?.to_str()?.trim().to_lowercase();
    if provider.is_empty() {
        return None;
    }
    let parent = runtime_dir.parent()?;
    if parent.file_name()? != "provider-runtime" {
        return None;
    }
    let agent_dir = parent.parent()?;
    Some(agent_dir.join("provider-state").join(provider))
}

/// Read a session JSON payload from disk, returning `None` on any error.
pub fn read_session_payload(session_path: impl AsRef<Path>) -> Option<Map<String, Value>> {
    let data = std::fs::read_to_string(session_path.as_ref()).ok()?;
    let value: Value = serde_json::from_str(&data).ok()?;
    value.as_object().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_dir_for_runtime_dir_basic() {
        let runtime_dir = PathBuf::from("/repo/.ccbr/agents/reviewer/provider-runtime/claude");
        assert_eq!(
            state_dir_for_runtime_dir(&runtime_dir).unwrap(),
            PathBuf::from("/repo/.ccbr/agents/reviewer/provider-state/claude")
        );
    }

    #[test]
    fn test_state_dir_for_runtime_dir_rejects_non_provider_runtime() {
        let runtime_dir = PathBuf::from("/repo/.ccbr/agents/reviewer/runtime/claude");
        assert!(state_dir_for_runtime_dir(&runtime_dir).is_none());
    }
}
