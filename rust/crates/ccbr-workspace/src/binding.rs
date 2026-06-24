//! Workspace binding persistence.
//!
//! Mirrors `workspace.binding` from Python v7.5.2.

use std::path::{Path, PathBuf};

use camino::Utf8Path;
use ccbr_agents::models::WorkspaceMode;
use ccbr_storage::atomic::atomic_write_json;

use crate::models::{WorkspaceBinding, WorkspacePlan};
use crate::Result;

pub struct WorkspaceBindingStore;

impl WorkspaceBindingStore {
    pub fn new() -> Self {
        Self
    }

    pub fn load(&self, path: &Path) -> Result<WorkspaceBinding> {
        let text = std::fs::read_to_string(path)?;
        let record: serde_json::Value = serde_json::from_str(&text)?;
        if record.get("schema_version").and_then(|v| v.as_u64()) != Some(2) {
            return Err(crate::WorkspaceError::Validation(
                "workspace binding schema_version must be 2".to_string(),
            ));
        }
        if record.get("record_type").and_then(|v| v.as_str()) != Some("workspace_binding") {
            return Err(crate::WorkspaceError::Validation(
                "workspace binding record_type must be workspace_binding".to_string(),
            ));
        }
        let mode = record
            .get("workspace_mode")
            .and_then(|v| v.as_str())
            .map(parse_workspace_mode)
            .unwrap_or(WorkspaceMode::Inplace);
        WorkspaceBinding::new(
            record
                .get("target_project")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("agent_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            mode,
            record
                .get("workspace_path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            record
                .get("branch_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        )
    }

    pub fn save(&self, plan: &WorkspacePlan) -> Result<Option<PathBuf>> {
        let binding_path = match &plan.binding_path {
            Some(p) => p,
            None => return Ok(None),
        };
        let binding = WorkspaceBinding::new(
            plan.project_root.to_string_lossy().to_string(),
            plan.project_id.clone(),
            plan.agent_name.clone(),
            plan.workspace_mode,
            plan.workspace_path.to_string_lossy().to_string(),
            plan.branch_name.clone(),
        )?;
        let utf8_path = Utf8Path::from_path(binding_path).ok_or_else(|| {
            crate::WorkspaceError::Workspace("binding path is not valid utf-8".to_string())
        })?;
        atomic_write_json(utf8_path, &binding.to_record())?;
        Ok(Some(binding_path.clone()))
    }
}

impl Default for WorkspaceBindingStore {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_workspace_mode(value: &str) -> WorkspaceMode {
    match value {
        "git-worktree" => WorkspaceMode::GitWorktree,
        "copy" => WorkspaceMode::Copy,
        _ => WorkspaceMode::Inplace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccbr_agents::models::WorkspaceMode;
    use std::io::Write;

    #[test]
    fn binding_store_rejects_bad_schema_version() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("binding.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"{\"schema_version\": 1, \"record_type\": \"workspace_binding\"}")
            .unwrap();
        let store = WorkspaceBindingStore::new();
        assert!(store.load(&path).is_err());
    }

    #[test]
    fn binding_store_loads_valid_record() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("binding.json");
        std::fs::write(
            &path,
            r#"{
  "schema_version": 2,
  "record_type": "workspace_binding",
  "target_project": "/tmp/project",
  "project_id": "pid",
  "agent_name": "agent1",
  "workspace_mode": "copy",
  "workspace_path": "/tmp/project/.ccbr/workspaces/agent1",
  "branch_name": null
}"#,
        )
        .unwrap();
        let store = WorkspaceBindingStore::new();
        let binding = store.load(&path).unwrap();
        assert_eq!(binding.project_id, "pid");
        assert_eq!(binding.agent_name, "agent1");
        assert_eq!(binding.workspace_mode, WorkspaceMode::Copy);
    }

    #[test]
    fn binding_store_save_writes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let binding_path = tmp.path().join(".ccbr-workspace.json");
        let plan = WorkspacePlan::new(
            "pid".to_string(),
            tmp.path().to_path_buf(),
            "slug".to_string(),
            "agent1".to_string(),
            WorkspaceMode::Copy,
            tmp.path().join("workspace"),
            Some(binding_path.clone()),
            tmp.path().to_path_buf(),
            None,
            None,
            false,
            Some("agent".to_string()),
        )
        .unwrap();
        let store = WorkspaceBindingStore::new();
        let saved = store.save(&plan).unwrap();
        assert_eq!(saved, Some(binding_path.clone()));
        assert!(binding_path.exists());
        let loaded = store.load(&binding_path).unwrap();
        assert_eq!(loaded.project_id, "pid");
    }
}
