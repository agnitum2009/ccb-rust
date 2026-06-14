//! Workspace actor resolution.
//!
//! Mirrors `workspace.actors` from Python v7.5.2.

use std::path::{Path, PathBuf};

use ccb_project::discovery::find_workspace_binding;

use crate::binding::WorkspaceBindingStore;
use crate::models::normalize_path;

/// Resolve the workspace actor for `cwd`, optionally restricted to `project_id`.
pub fn resolve_workspace_actor(cwd: &Path, project_id: Option<&str>) -> Option<String> {
    let current = normalize_path(cwd);
    let binding_path = find_workspace_binding(
        camino::Utf8PathBuf::from_path_buf(current.clone())
            .unwrap_or_else(|_| camino::Utf8PathBuf::from("/")),
    )?;
    let binding = WorkspaceBindingStore::new()
        .load(binding_path.as_std_path())
        .ok()?;
    if let Some(expected) = project_id {
        if binding.project_id != expected {
            return None;
        }
    }
    let workspace_root = normalize_path(PathBuf::from(&binding.workspace_path).as_path());
    if current != workspace_root {
        let mut is_descendant = false;
        for ancestor in current.ancestors() {
            if ancestor == workspace_root {
                is_descendant = true;
                break;
            }
        }
        if !is_descendant {
            return None;
        }
    }
    Some(binding.agent_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;
    use ccb_agents::models::WorkspaceMode;
    use ccb_storage::atomic::atomic_write_json;

    #[test]
    fn resolve_actor_finds_binding() {
        let tmp = tempfile::tempdir().unwrap();
        let binding_path = tmp.path().join(".ccb-workspace.json");
        let binding = crate::models::WorkspaceBinding::new(
            tmp.path().to_string_lossy().to_string(),
            "pid".to_string(),
            "agent1".to_string(),
            WorkspaceMode::Copy,
            tmp.path().to_string_lossy().to_string(),
            None,
        )
        .unwrap();
        atomic_write_json(
            Utf8Path::from_path(&binding_path).unwrap(),
            &binding.to_record(),
        )
        .unwrap();

        let actor = resolve_workspace_actor(tmp.path(), Some("pid"));
        assert_eq!(actor, Some("agent1".to_string()));
    }

    #[test]
    fn resolve_actor_rejects_wrong_project_id() {
        let tmp = tempfile::tempdir().unwrap();
        let binding_path = tmp.path().join(".ccb-workspace.json");
        let binding = crate::models::WorkspaceBinding::new(
            tmp.path().to_string_lossy().to_string(),
            "pid".to_string(),
            "agent1".to_string(),
            WorkspaceMode::Copy,
            tmp.path().to_string_lossy().to_string(),
            None,
        )
        .unwrap();
        atomic_write_json(
            Utf8Path::from_path(&binding_path).unwrap(),
            &binding.to_record(),
        )
        .unwrap();

        let actor = resolve_workspace_actor(tmp.path(), Some("other"));
        assert_eq!(actor, None);
    }

    #[test]
    fn resolve_actor_rejects_outside_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let binding_path = tmp.path().join(".ccb-workspace.json");
        let workspace = tmp.path().join("ws");
        std::fs::create_dir(&workspace).unwrap();
        let binding = crate::models::WorkspaceBinding::new(
            tmp.path().to_string_lossy().to_string(),
            "pid".to_string(),
            "agent1".to_string(),
            WorkspaceMode::Copy,
            workspace.to_string_lossy().to_string(),
            None,
        )
        .unwrap();
        atomic_write_json(
            Utf8Path::from_path(&binding_path).unwrap(),
            &binding.to_record(),
        )
        .unwrap();

        let actor = resolve_workspace_actor(tmp.path(), Some("pid"));
        assert_eq!(actor, None);
    }
}
