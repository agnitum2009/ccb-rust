//! Codex launcher session path helpers.
//!
//! Mirrors Python `lib/provider_backends/codex/launcher_runtime/session_paths.py`.

use camino::{Utf8Path, Utf8PathBuf};
use ccb_provider_core::pathing::session_filename_for_agent;
use ccb_provider_profiles::models::{ProviderProfileSpec, ResolvedProviderProfile};

/// Locate the project-level Codex session file for an agent runtime directory.
///
/// Mirrors Python `session_file_for_runtime_dir`.
pub fn session_file_for_runtime_dir(runtime_dir: &Utf8Path) -> Option<Utf8PathBuf> {
    let ccb_dir = find_project_ccb_dir(runtime_dir)?;
    let agent_name = runtime_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())?;
    if agent_name.is_empty() {
        return None;
    }
    let filename = session_filename_for_agent("codex", agent_name).ok()?;
    Some(ccb_dir.join(filename))
}

/// Find the nearest `.ccb` directory ancestor of `runtime_dir`.
///
/// Falls back to the relocated runtime anchor marker if no `.ccb` directory
/// is found in the ancestor chain.
pub fn find_project_ccb_dir(runtime_dir: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut current = Some(runtime_dir);
    while let Some(p) = current {
        if p.file_name() == Some(".ccb") {
            return Some(p.to_path_buf());
        }
        current = p.parent();
    }
    ccb_storage::path_helpers::runtime_project_anchor_from_path(runtime_dir)
}

/// Decide whether to resume a previous Codex session and return its id.
///
/// Mirrors Python `load_resume_session_id`.
pub fn load_resume_session_id(
    _spec: &ccb_agents::models::AgentSpec,
    runtime_dir: &Utf8Path,
    profile: Option<&ResolvedProviderProfile>,
    current_fingerprint: Option<&str>,
    current_memory_fingerprint: Option<&str>,
) -> Option<String> {
    let session_path = preferred_session_path(_spec, runtime_dir)?;
    let data = read_session_payload(&session_path)?;
    let profile_spec = profile.map(resolved_profile_to_spec);
    if !crate::session_authority::resume_authority_matches(
        &data,
        profile_spec.as_ref(),
        current_fingerprint,
        current_memory_fingerprint,
    ) {
        return None;
    }
    if !resume_session_binding_is_usable(&data) {
        return None;
    }
    payload_resume_session_id(&data)
}

fn preferred_session_path(
    spec: &ccb_agents::models::AgentSpec,
    runtime_dir: &Utf8Path,
) -> Option<Utf8PathBuf> {
    agent_session_path(spec, runtime_dir)
}

fn agent_session_path(
    spec: &ccb_agents::models::AgentSpec,
    runtime_dir: &Utf8Path,
) -> Option<Utf8PathBuf> {
    let ccb_dir = find_project_ccb_dir(runtime_dir)?;
    let filename = session_filename_for_agent("codex", &spec.name).ok()?;
    Some(ccb_dir.join(filename))
}

fn read_session_payload(
    session_path: &Utf8Path,
) -> Option<std::collections::HashMap<String, serde_json::Value>> {
    let text = std::fs::read_to_string(session_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .as_object()
        .cloned()
        .map(|obj| obj.into_iter().collect())
}

fn payload_resume_session_id(
    data: &std::collections::HashMap<String, serde_json::Value>,
) -> Option<String> {
    let session_id = data
        .get("codex_session_id")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if session_id.is_some() {
        return session_id;
    }
    let start_cmd = data
        .get("codex_start_cmd")
        .or_else(|| data.get("start_cmd"))
        .and_then(|v| v.as_str())?;
    crate::session_authority::extract_resume_session_id(start_cmd)
}

fn resume_session_binding_is_usable(
    data: &std::collections::HashMap<String, serde_json::Value>,
) -> bool {
    let session_path = path_or_none(data.get("codex_session_path").and_then(|v| v.as_str()));
    if session_path.is_none() {
        return true;
    }
    let session_path = session_path.unwrap();
    if !session_path.is_file() {
        return false;
    }
    let session_root = path_or_none(data.get("codex_session_root").and_then(|v| v.as_str()));
    if let Some(session_root) = session_root {
        if !is_within(&session_path, &session_root) {
            return false;
        }
    }
    true
}

fn path_or_none(value: Option<&str>) -> Option<std::path::PathBuf> {
    let raw = value.unwrap_or("").trim();
    if raw.is_empty() {
        return None;
    }
    Some(std::path::Path::new(raw).to_path_buf())
}

fn is_within(path: &std::path::Path, root: &std::path::Path) -> bool {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    path.strip_prefix(&root).is_ok()
}

fn resolved_profile_to_spec(profile: &ResolvedProviderProfile) -> ProviderProfileSpec {
    ProviderProfileSpec {
        mode: profile.mode.trim().to_lowercase(),
        home: profile.runtime_home.clone(),
        env: profile.env.clone(),
        inherit_api: profile.inherit_api,
        inherit_auth: profile.inherit_auth,
        inherit_config: profile.inherit_config,
        inherit_skills: profile.inherit_skills,
        inherit_commands: profile.inherit_commands,
        inherit_memory: profile.inherit_memory,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_session_file_for_runtime_dir() {
        let runtime = Utf8PathBuf::from("/repo/.ccb/agents/agent1/provider-runtime/codex");
        assert_eq!(
            session_file_for_runtime_dir(&runtime),
            Some(Utf8PathBuf::from("/repo/.ccb/.codex-agent1-session"))
        );
    }

    fn write_runtime_marker(anchor: &Utf8Path, runtime_root: &Utf8Path, project_root: &Utf8Path) {
        let marker = runtime_root.join("runtime-root.json");
        std::fs::create_dir_all(runtime_root).unwrap();
        std::fs::write(
            &marker,
            json!({
                "schema_version": 1,
                "record_type": "ccb_runtime_root",
                "project_id": "proj-1",
                "project_root": project_root.as_str(),
                "anchor_path": anchor.as_str(),
                "runtime_root_path": runtime_root.as_str(),
                "created_at": "2026-05-07T00:00:00Z",
            })
            .to_string(),
        )
        .unwrap();
    }

    #[test]
    fn session_file_follows_relocated_runtime_anchor() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root_buf = tmp.path().join("repo");
        let project_root = Utf8Path::from_path(&project_root_buf).unwrap();
        let anchor = project_root.join(".ccb");
        std::fs::create_dir_all(&anchor).unwrap();
        let runtime_root_buf = tmp.path().join("state-root");
        let runtime_root = Utf8Path::from_path(&runtime_root_buf).unwrap();
        write_runtime_marker(&anchor, &runtime_root, &project_root);

        let runtime_dir = runtime_root.join("agents/reviewer/provider-runtime/codex");
        std::fs::create_dir_all(&runtime_dir).unwrap();

        let expected = anchor.join(".codex-reviewer-session");
        assert_eq!(
            find_project_ccb_dir(&runtime_dir),
            Some(anchor.to_path_buf())
        );
        assert_eq!(session_file_for_runtime_dir(&runtime_dir), Some(expected));
    }

    #[test]
    fn session_file_rejects_invalid_relocated_runtime_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root_buf = tmp.path().join("repo");
        let project_root = Utf8Path::from_path(&project_root_buf).unwrap();
        let anchor = project_root.join(".ccb");
        std::fs::create_dir_all(&anchor).unwrap();
        let runtime_root_buf = tmp.path().join("state-root");
        let runtime_root = Utf8Path::from_path(&runtime_root_buf).unwrap();
        let different_root_buf = tmp.path().join("different-root");
        let different_root = Utf8Path::from_path(&different_root_buf).unwrap();
        write_runtime_marker(&anchor, &runtime_root, &project_root);
        // Corrupt the marker to point at a different runtime root.
        let marker = runtime_root.join("runtime-root.json");
        std::fs::write(
            &marker,
            json!({
                "schema_version": 1,
                "record_type": "ccb_runtime_root",
                "project_id": "proj-1",
                "project_root": project_root.as_str(),
                "anchor_path": anchor.as_str(),
                "runtime_root_path": different_root.as_str(),
                "created_at": "2026-05-07T00:00:00Z",
            })
            .to_string(),
        )
        .unwrap();

        let runtime_dir = runtime_root.join("agents/reviewer/provider-runtime/codex");
        std::fs::create_dir_all(&runtime_dir).unwrap();

        assert!(find_project_ccb_dir(&runtime_dir).is_none());
        assert!(session_file_for_runtime_dir(&runtime_dir).is_none());
    }
}
