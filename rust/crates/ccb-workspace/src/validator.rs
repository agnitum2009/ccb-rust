//! Workspace plan validation.
//!
//! Mirrors `workspace.validator` from Python v7.5.2.

use std::collections::HashMap;
use std::path::PathBuf;

use ccb_agents::models::WorkspaceMode;

use crate::binding::WorkspaceBindingStore;
use crate::models::{ValidationResult, WorkspacePlan};

pub struct WorkspaceValidator {
    binding_store: WorkspaceBindingStore,
}

impl WorkspaceValidator {
    pub fn new() -> Self {
        Self {
            binding_store: WorkspaceBindingStore::new(),
        }
    }

    pub fn with_binding_store(binding_store: WorkspaceBindingStore) -> Self {
        Self { binding_store }
    }

    pub fn validate(&self, plan: &WorkspacePlan) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut diagnostics: HashMap<String, String> = [
            (
                "workspace_path".to_string(),
                plan.workspace_path.to_string_lossy().to_string(),
            ),
            (
                "workspace_mode".to_string(),
                format!("{:?}", plan.workspace_mode),
            ),
        ]
        .into_iter()
        .collect();
        self.validate_workspace_mode(plan, &mut errors);
        self.validate_branch_requirements(plan, &mut errors);
        self.validate_binding(plan, &mut errors, &mut warnings, &mut diagnostics);
        ValidationResult {
            ok: errors.is_empty(),
            errors,
            warnings,
            diagnostics,
        }
    }

    fn validate_workspace_mode(&self, plan: &WorkspacePlan, errors: &mut Vec<String>) {
        if plan.workspace_mode == WorkspaceMode::Inplace {
            if plan.workspace_path != plan.project_root {
                errors.push("inplace workspace_path must equal project_root".to_string());
            }
            if !plan.unsafe_shared_workspace {
                errors.push("inplace mode must be marked unsafe_shared_workspace".to_string());
            }
        } else if plan.workspace_path == plan.project_root {
            errors.push("non-inplace workspace must not reuse project_root".to_string());
        }
    }

    fn validate_branch_requirements(&self, plan: &WorkspacePlan, errors: &mut Vec<String>) {
        if plan.branch_name.is_none()
            && plan.workspace_mode == WorkspaceMode::GitWorktree
            && plan.workspace_scope != "external"
        {
            errors.push("git-worktree mode requires branch_name".to_string());
        }
    }

    fn validate_binding(
        &self,
        plan: &WorkspacePlan,
        errors: &mut Vec<String>,
        warnings: &mut Vec<String>,
        _diagnostics: &mut HashMap<String, String>,
    ) {
        if plan.workspace_path.exists() {
            if let Some(binding_path) = &plan.binding_path {
                if !binding_path.exists() {
                    warnings.push("workspace binding file is missing".to_string());
                } else {
                    match self.binding_store.load(binding_path) {
                        Ok(binding) => self.validate_binding_matches_plan(&binding, plan, errors),
                        Err(e) => errors.push(format!("failed to load workspace binding: {e}")),
                    }
                }
            }
        }
    }

    fn validate_binding_matches_plan(
        &self,
        binding: &crate::models::WorkspaceBinding,
        plan: &WorkspacePlan,
        errors: &mut Vec<String>,
    ) {
        if PathBuf::from(&binding.target_project).expand_home() != plan.project_root {
            errors.push("workspace binding target_project does not match project_root".to_string());
        }
        if binding.project_id != plan.project_id {
            errors.push("workspace binding project_id does not match project_id".to_string());
        }
        if PathBuf::from(&binding.workspace_path).expand_home() != plan.workspace_path {
            errors
                .push("workspace binding workspace_path does not match workspace_path".to_string());
        }
        if binding.agent_name != plan.agent_name && plan.workspace_scope != "group" {
            errors.push("workspace binding agent_name does not match agent_name".to_string());
        }
    }
}

impl Default for WorkspaceValidator {
    fn default() -> Self {
        Self::new()
    }
}

trait ExpandHome {
    fn expand_home(&self) -> PathBuf;
}

impl ExpandHome for PathBuf {
    fn expand_home(&self) -> PathBuf {
        let s = self.to_string_lossy();
        if let Some(rest) = s.strip_prefix('~') {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home + rest);
            }
        }
        self.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccb_agents::models::WorkspaceMode;

    fn plan(mode: WorkspaceMode, path: &str, shared: bool) -> WorkspacePlan {
        WorkspacePlan::new(
            "pid".to_string(),
            PathBuf::from("/tmp/project"),
            "slug".to_string(),
            "agent1".to_string(),
            mode,
            PathBuf::from(path),
            None,
            PathBuf::from("/tmp/project"),
            None,
            None,
            shared,
            None,
        )
        .unwrap()
    }

    #[test]
    fn inplace_ok() {
        let plan = plan(WorkspaceMode::Inplace, "/tmp/project", true);
        let result = WorkspaceValidator::new().validate(&plan);
        assert!(result.ok);
    }

    #[test]
    fn inplace_requires_shared_flag() {
        let plan = plan(WorkspaceMode::Inplace, "/tmp/project", false);
        let result = WorkspaceValidator::new().validate(&plan);
        assert!(!result.ok);
        assert!(result
            .errors
            .iter()
            .any(|e| e.contains("unsafe_shared_workspace")));
    }

    #[test]
    fn inplace_rejects_different_path() {
        let plan = plan(WorkspaceMode::Inplace, "/tmp/other", true);
        let result = WorkspaceValidator::new().validate(&plan);
        assert!(!result.ok);
        assert!(result.errors.iter().any(|e| e.contains("project_root")));
    }

    #[test]
    fn non_inplace_reuses_project_root() {
        let plan = plan(WorkspaceMode::Copy, "/tmp/project", false);
        let result = WorkspaceValidator::new().validate(&plan);
        assert!(!result.ok);
        assert!(result
            .errors
            .iter()
            .any(|e| e.contains("must not reuse project_root")));
    }

    #[test]
    fn git_worktree_requires_branch() {
        let plan = WorkspacePlan::new(
            "pid".to_string(),
            PathBuf::from("/tmp/project"),
            "slug".to_string(),
            "agent1".to_string(),
            WorkspaceMode::GitWorktree,
            PathBuf::from("/tmp/project/.ccbr/workspaces/agent1"),
            None,
            PathBuf::from("/tmp/project"),
            None,
            None,
            false,
            Some("agent".to_string()),
        )
        .unwrap();
        let result = WorkspaceValidator::new().validate(&plan);
        assert!(!result.ok);
        assert!(result.errors.iter().any(|e| e.contains("branch_name")));
    }

    #[test]
    fn external_git_worktree_allows_missing_branch() {
        let plan = WorkspacePlan::new(
            "pid".to_string(),
            PathBuf::from("/tmp/project"),
            "slug".to_string(),
            "agent1".to_string(),
            WorkspaceMode::GitWorktree,
            PathBuf::from("/external"),
            None,
            PathBuf::from("/tmp/project"),
            None,
            None,
            false,
            Some("external".to_string()),
        )
        .unwrap();
        let result = WorkspaceValidator::new().validate(&plan);
        assert!(result.ok);
    }
}
