//! Workspace materialization.
//!
//! Mirrors `workspace.materializer` from Python v7.5.2.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ccb_agents::models::WorkspaceMode;

use crate::git_worktree::{
    can_use_git_worktree, has_missing_registered_worktree, prune_missing_worktrees_under,
};
use crate::models::{normalize_path, WorkspacePlan};
use crate::Result;

const COPY_IGNORE_NAMES: &[&str] = &[".git", ".ccb", "__pycache__", ".pytest_cache"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationResult {
    pub workspace_path: PathBuf,
    pub created: bool,
    pub mode: String,
}

pub struct WorkspaceMaterializer;

impl WorkspaceMaterializer {
    pub fn new() -> Self {
        Self
    }

    pub fn materialize(&self, plan: &WorkspacePlan) -> Result<MaterializationResult> {
        match plan.workspace_mode {
            WorkspaceMode::Inplace => {
                std::fs::create_dir_all(&plan.workspace_path)?;
                Ok(MaterializationResult {
                    workspace_path: plan.workspace_path.clone(),
                    created: false,
                    mode: "inplace".to_string(),
                })
            }
            WorkspaceMode::Copy => self.materialize_copy(plan),
            WorkspaceMode::GitWorktree => self.materialize_git_worktree(plan),
        }
    }

    fn materialize_copy(&self, plan: &WorkspacePlan) -> Result<MaterializationResult> {
        if self.is_placeholder_workspace(plan) {
            self.clear_placeholder_workspace(plan)?;
        }
        if plan.workspace_path.exists() {
            return Ok(MaterializationResult {
                workspace_path: plan.workspace_path.clone(),
                created: false,
                mode: "copy".to_string(),
            });
        }
        if let Some(parent) = plan.workspace_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        copy_dir_contents(&plan.source_root, &plan.workspace_path)?;
        Ok(MaterializationResult {
            workspace_path: plan.workspace_path.clone(),
            created: true,
            mode: "copy".to_string(),
        })
    }

    fn materialize_git_worktree(&self, plan: &WorkspacePlan) -> Result<MaterializationResult> {
        if plan.branch_name.is_none() && plan.workspace_scope != "external" {
            return Err(crate::WorkspaceError::Validation(
                "git-worktree workspace requires branch_name".to_string(),
            ));
        }
        if !can_use_git_worktree(&plan.project_root) {
            return Err(crate::WorkspaceError::Workspace(format!(
                "git-worktree workspace requires a git repository: {}; use workspace_mode=\"copy\" for an explicit directory copy",
                plan.project_root.display()
            )));
        }
        if plan.workspace_scope == "external" {
            self.validate_external_git_workspace(plan)?;
            return Ok(MaterializationResult {
                workspace_path: plan.workspace_path.clone(),
                created: false,
                mode: "git-worktree".to_string(),
            });
        }
        if self.is_existing_git_workspace(&plan.workspace_path) {
            self.validate_existing_git_workspace(plan)?;
            return Ok(MaterializationResult {
                workspace_path: plan.workspace_path.clone(),
                created: false,
                mode: "git-worktree".to_string(),
            });
        }
        if self.is_placeholder_workspace(plan) {
            self.clear_placeholder_workspace(plan)?;
        } else if plan.workspace_path.exists()
            && std::fs::read_dir(&plan.workspace_path)?.next().is_some()
        {
            return Err(crate::WorkspaceError::Workspace(format!(
                "workspace path is not empty and is not a git worktree: {}",
                plan.workspace_path.display()
            )));
        }
        if let Some(parent) = plan.workspace_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.prune_stale_worktree_registration(plan)?;
        if has_missing_registered_worktree(&plan.project_root, &plan.workspace_path) {
            prune_missing_worktrees_under(
                &plan.project_root,
                plan.workspace_path.parent().unwrap_or(&plan.project_root),
            )?;
        }

        if let Err(e) = self.run_git_worktree_add(plan, false) {
            self.prune_stale_worktree_registration(plan)?;
            if !has_missing_registered_worktree(&plan.project_root, &plan.workspace_path) {
                return Err(e);
            }
            prune_missing_worktrees_under(
                &plan.project_root,
                plan.workspace_path.parent().unwrap_or(&plan.project_root),
            )?;
            self.run_git_worktree_add(plan, true)?;
        }
        self.validate_existing_git_workspace(plan)?;
        Ok(MaterializationResult {
            workspace_path: plan.workspace_path.clone(),
            created: true,
            mode: "git-worktree".to_string(),
        })
    }

    fn run_git_worktree_add(&self, plan: &WorkspacePlan, force: bool) -> Result<()> {
        let branch = plan.branch_name.as_ref().unwrap();
        let branch_exists = git_branch_exists(&plan.project_root, branch)?;
        let mut args: Vec<String> = vec![
            "-C".to_string(),
            plan.project_root.to_string_lossy().to_string(),
            "worktree".to_string(),
            "add".to_string(),
        ];
        if force {
            args.push("-f".to_string());
        }
        if branch_exists {
            args.push(plan.workspace_path.to_string_lossy().to_string());
            args.push(branch.clone());
        } else {
            args.push("-b".to_string());
            args.push(branch.clone());
            args.push(plan.workspace_path.to_string_lossy().to_string());
            args.push("HEAD".to_string());
        }
        run_git(
            &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            &format!("failed to materialize git worktree for {}", plan.agent_name),
        )
    }

    fn prune_stale_worktree_registration(&self, plan: &WorkspacePlan) -> Result<()> {
        if plan.workspace_path.exists() {
            return Ok(());
        }
        let (registered, prunable) = self.worktree_registration_status(plan)?;
        if registered && prunable {
            run_git(
                &[
                    "-C",
                    &plan.project_root.to_string_lossy(),
                    "worktree",
                    "prune",
                ],
                "failed to prune stale git worktree registration",
            )?;
        }
        Ok(())
    }

    fn worktree_registration_status(&self, plan: &WorkspacePlan) -> Result<(bool, bool)> {
        let output = git(&plan.project_root, &["worktree", "list", "--porcelain"])?;
        if !output.status.success() {
            return Ok((false, false));
        }
        let target = normalize_path(&plan.workspace_path);
        let mut registered = false;
        let mut prunable = false;
        let text = String::from_utf8_lossy(&output.stdout);
        for block in text.split("\n\n") {
            let lines: Vec<&str> = block.lines().collect();
            if lines.is_empty() || !lines[0].starts_with("worktree ") {
                continue;
            }
            let raw_path = lines[0]["worktree ".len()..].trim();
            if normalize_path(Path::new(raw_path)) != target {
                continue;
            }
            registered = true;
            prunable = lines
                .iter()
                .skip(1)
                .any(|line| line.starts_with("prunable "));
            break;
        }
        Ok((registered, prunable))
    }

    fn validate_existing_git_workspace(&self, plan: &WorkspacePlan) -> Result<()> {
        let top_level = git_output(&plan.workspace_path, &["rev-parse", "--show-toplevel"])?;
        if normalize_path(Path::new(&top_level)) != plan.workspace_path {
            return Err(crate::WorkspaceError::Workspace(format!(
                "workspace path is not the git worktree root: {}",
                plan.workspace_path.display()
            )));
        }
        if let Some(expected) = &plan.branch_name {
            let current = git_output(&plan.workspace_path, &["branch", "--show-current"])?;
            if !current.is_empty() && &current != expected {
                return Err(crate::WorkspaceError::Workspace(format!(
                    "workspace branch mismatch for {}: expected {expected}, got {current}",
                    plan.agent_name
                )));
            }
        }
        Ok(())
    }

    fn validate_external_git_workspace(&self, plan: &WorkspacePlan) -> Result<()> {
        if plan.workspace_path == plan.project_root {
            return Err(crate::WorkspaceError::Workspace(
                "external workspace_path must not equal the project root; use workspace_mode=\"inplace\"".to_string(),
            ));
        }
        if !plan.workspace_path.exists() {
            return Err(crate::WorkspaceError::Workspace(format!(
                "external workspace_path does not exist: {}",
                plan.workspace_path.display()
            )));
        }
        if !self.is_existing_git_workspace(&plan.workspace_path) {
            return Err(crate::WorkspaceError::Workspace(format!(
                "external workspace_path is not a git workspace root: {}",
                plan.workspace_path.display()
            )));
        }
        self.validate_existing_git_workspace(plan)?;
        let project_common = git_common_dir(&plan.project_root)?;
        let workspace_common = git_common_dir(&plan.workspace_path)?;
        if project_common != workspace_common {
            return Err(crate::WorkspaceError::Workspace(format!(
                "external workspace_path is not from the project git repository: {}",
                plan.workspace_path.display()
            )));
        }
        Ok(())
    }

    fn is_existing_git_workspace(&self, path: &Path) -> bool {
        if !path.join(".git").exists() {
            return false;
        }
        git_output(path, &["rev-parse", "--show-toplevel"]).is_ok()
    }

    fn is_placeholder_workspace(&self, plan: &WorkspacePlan) -> bool {
        if !plan.workspace_path.exists() || !plan.workspace_path.is_dir() {
            return false;
        }
        let allowed: HashSet<String> = plan
            .binding_path
            .as_ref()
            .map(|p| {
                vec![p
                    .file_name()
                    .unwrap_or(std::ffi::OsStr::new(".ccb-workspace.json"))
                    .to_string_lossy()
                    .to_string()]
            })
            .unwrap_or_default()
            .into_iter()
            .collect();
        if let Ok(entries) = std::fs::read_dir(&plan.workspace_path) {
            entries
                .flatten()
                .all(|e| allowed.contains(&e.file_name().to_string_lossy().to_string()))
        } else {
            false
        }
    }

    fn clear_placeholder_workspace(&self, plan: &WorkspacePlan) -> Result<()> {
        if let Some(binding_path) = &plan.binding_path {
            if binding_path.exists() {
                std::fs::remove_file(binding_path)?;
            }
        }
        if plan.workspace_path.exists() {
            let _ = std::fs::remove_dir(&plan.workspace_path);
        }
        Ok(())
    }
}

impl Default for WorkspaceMaterializer {
    fn default() -> Self {
        Self::new()
    }
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let name = entry.file_name().to_string_lossy().to_string();
        if file_type.is_dir() {
            if COPY_IGNORE_NAMES.contains(&name.as_str()) {
                continue;
            }
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn git_branch_exists(repo_root: &Path, branch_name: &str) -> Result<bool> {
    let output = git(
        repo_root,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch_name}"),
        ],
    )?;
    Ok(output.status.success())
}

fn git_output(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = git(cwd, args)?;
    if !output.status.success() {
        return Err(crate::WorkspaceError::Workspace(
            "git command failed".to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_common_dir(cwd: &Path) -> Result<PathBuf> {
    let raw = git_output(cwd, &["rev-parse", "--git-common-dir"])?;
    let path = PathBuf::from(raw);
    Ok(if path.is_absolute() {
        normalize_path(&path)
    } else {
        normalize_path(&cwd.join(path))
    })
}

fn git(repo_root: &Path, args: &[&str]) -> Result<std::process::Output> {
    Ok(std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()?)
}

fn run_git(args: &[&str], error: &str) -> Result<()> {
    let output = std::process::Command::new("git").args(args).output()?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return Err(crate::WorkspaceError::Workspace(format!(
            "{error}: {detail}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(tmp: &tempfile::TempDir, mode: WorkspaceMode) -> WorkspacePlan {
        let root = tmp.path().to_path_buf();
        WorkspacePlan::new(
            "pid".to_string(),
            root.clone(),
            "slug".to_string(),
            "agent1".to_string(),
            mode,
            root.clone(),
            None,
            root.clone(),
            None,
            None,
            true,
            Some("inplace".to_string()),
        )
        .unwrap()
    }

    #[test]
    fn materialize_inplace_creates_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = plan(&tmp, WorkspaceMode::Inplace);
        let result = WorkspaceMaterializer::new().materialize(&plan).unwrap();
        assert!(!result.created);
        assert_eq!(result.mode, "inplace");
    }

    #[test]
    fn materialize_copy_copies_contents() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("file.txt"), "hello").unwrap();
        let dst = tmp.path().join("dst");
        let plan = WorkspacePlan::new(
            "pid".to_string(),
            src.clone(),
            "slug".to_string(),
            "agent1".to_string(),
            WorkspaceMode::Copy,
            dst.clone(),
            None,
            src,
            None,
            None,
            false,
            Some("agent".to_string()),
        )
        .unwrap();
        let result = WorkspaceMaterializer::new().materialize(&plan).unwrap();
        assert!(result.created);
        assert_eq!(
            std::fs::read_to_string(dst.join("file.txt")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn placeholder_workspace_is_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("workspace");
        std::fs::create_dir(&ws).unwrap();
        let binding = ws.join(".ccb-workspace.json");
        std::fs::write(&binding, "{}").unwrap();
        let plan = WorkspacePlan::new(
            "pid".to_string(),
            tmp.path().to_path_buf(),
            "slug".to_string(),
            "agent1".to_string(),
            WorkspaceMode::Copy,
            ws.clone(),
            Some(binding),
            tmp.path().to_path_buf(),
            None,
            None,
            false,
            Some("agent".to_string()),
        )
        .unwrap();
        assert!(WorkspaceMaterializer::new().is_placeholder_workspace(&plan));
    }

    #[test]
    fn copy_ignores_git_and_ccb() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("file.txt"), "hello").unwrap();
        std::fs::create_dir(src.join(".git")).unwrap();
        std::fs::write(src.join(".git/config"), "x").unwrap();
        let dst = tmp.path().join("dst");
        copy_dir_contents(&src, &dst).unwrap();
        assert!(dst.join("file.txt").exists());
        assert!(!dst.join(".git").exists());
    }
}
