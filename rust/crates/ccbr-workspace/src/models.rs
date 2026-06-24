//! Workspace data models.
//!
//! Mirrors `workspace.models` from Python v7.5.2.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccbr_agents::models::{normalize_agent_name, WorkspaceMode};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 2;

/// Expand a leading `~` to `$HOME`.
pub(crate) fn expand_user_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home + rest);
        }
    }
    path.to_path_buf()
}

/// Expand a leading `~` in a string path.
pub(crate) fn expand_user_path_str(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    path.to_string()
}

/// Python-compatible `Path.expanduser().resolve(strict=False)`.
pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    let expanded = expand_user_path(path);
    if let Ok(resolved) = std::fs::canonicalize(&expanded) {
        resolved
    } else if let Ok(absolute) = std::path::absolute(&expanded) {
        absolute
    } else {
        expanded
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlan {
    pub project_id: String,
    pub project_root: PathBuf,
    pub project_slug: String,
    pub agent_name: String,
    pub workspace_mode: WorkspaceMode,
    pub workspace_path: PathBuf,
    pub binding_path: Option<PathBuf>,
    pub source_root: PathBuf,
    pub branch_name: Option<String>,
    pub branch_template: String,
    pub unsafe_shared_workspace: bool,
    pub workspace_scope: String,
}

impl WorkspacePlan {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_id: String,
        project_root: PathBuf,
        project_slug: String,
        agent_name: String,
        workspace_mode: WorkspaceMode,
        workspace_path: PathBuf,
        binding_path: Option<PathBuf>,
        source_root: PathBuf,
        branch_name: Option<String>,
        branch_template: Option<String>,
        unsafe_shared_workspace: bool,
        workspace_scope: Option<String>,
    ) -> crate::Result<Self> {
        let project_root = normalize_path(&project_root);
        let workspace_path = normalize_path(&workspace_path);
        let source_root = normalize_path(&source_root);
        let binding_path = binding_path.map(|p| normalize_path(&p));
        let agent_name = normalize_agent_name(&agent_name)?;
        let workspace_scope = workspace_scope
            .unwrap_or_else(|| "agent".to_string())
            .trim()
            .to_lowercase();
        let branch_template = branch_template.unwrap_or_else(|| "ccb/{agent_name}".to_string());
        Ok(Self {
            project_id,
            project_root,
            project_slug,
            agent_name,
            workspace_mode,
            workspace_path,
            binding_path,
            source_root,
            branch_name,
            branch_template,
            unsafe_shared_workspace,
            workspace_scope,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRef {
    pub workspace_mode: WorkspaceMode,
    pub workspace_path: PathBuf,
    pub binding_path: Option<PathBuf>,
    pub branch_name: Option<String>,
}

impl WorkspaceRef {
    pub fn new(
        workspace_mode: WorkspaceMode,
        workspace_path: PathBuf,
        binding_path: Option<PathBuf>,
        branch_name: Option<String>,
    ) -> crate::Result<Self> {
        Ok(Self {
            workspace_mode,
            workspace_path: normalize_path(&workspace_path),
            binding_path: binding_path.map(|p| normalize_path(&p)),
            branch_name,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceBinding {
    pub target_project: String,
    pub project_id: String,
    pub agent_name: String,
    pub workspace_mode: WorkspaceMode,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_name: Option<String>,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_record_type")]
    pub record_type: String,
}

impl WorkspaceBinding {
    pub fn new(
        target_project: String,
        project_id: String,
        agent_name: String,
        workspace_mode: WorkspaceMode,
        workspace_path: String,
        branch_name: Option<String>,
    ) -> crate::Result<Self> {
        Ok(Self {
            target_project,
            project_id,
            agent_name: normalize_agent_name(&agent_name)?,
            workspace_mode,
            workspace_path,
            branch_name,
            schema_version: SCHEMA_VERSION,
            record_type: "workspace_binding".to_string(),
        })
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": self.schema_version,
            "record_type": self.record_type,
            "target_project": self.target_project,
            "project_id": self.project_id,
            "agent_name": self.agent_name,
            "workspace_mode": self.workspace_mode,
            "workspace_path": self.workspace_path,
            "branch_name": self.branch_name,
        })
    }
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

fn default_record_type() -> String {
    "workspace_binding".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub ok: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub diagnostics: HashMap<String, String>,
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self {
            ok: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            diagnostics: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_matches_python() {
        assert_eq!(SCHEMA_VERSION, 2);
    }

    #[test]
    fn workspace_plan_normalizes_paths_and_agent_name() {
        let plan = WorkspacePlan::new(
            "pid".to_string(),
            PathBuf::from("/tmp/project"),
            "slug".to_string(),
            "AgentOne".to_string(),
            WorkspaceMode::Inplace,
            PathBuf::from("/tmp/project"),
            None,
            PathBuf::from("/tmp/project"),
            None,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(plan.agent_name, "agentone");
        assert!(plan.workspace_path.is_absolute());
        assert_eq!(plan.workspace_scope, "agent");
    }

    #[test]
    fn workspace_binding_to_record_round_trip() {
        let binding = WorkspaceBinding::new(
            "/tmp/project".to_string(),
            "pid".to_string(),
            "AgentOne".to_string(),
            WorkspaceMode::Copy,
            "/tmp/project/.ccbr/workspaces/agentone".to_string(),
            Some("feature".to_string()),
        )
        .unwrap();
        let record = binding.to_record();
        assert_eq!(
            record.get("schema_version").and_then(|v| v.as_u64()),
            Some(2)
        );
        assert_eq!(
            record.get("record_type").and_then(|v| v.as_str()),
            Some("workspace_binding")
        );
        assert_eq!(
            record.get("agent_name").and_then(|v| v.as_str()),
            Some("agentone")
        );
    }

    #[test]
    fn validation_result_defaults() {
        let r = ValidationResult::default();
        assert!(r.ok);
        assert!(r.errors.is_empty());
        assert!(r.warnings.is_empty());
        assert!(r.diagnostics.is_empty());
    }
}
