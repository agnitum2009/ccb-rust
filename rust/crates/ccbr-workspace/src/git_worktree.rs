//! Git worktree helpers.
//!
//! Mirrors `workspace.git_worktree` from Python v7.5.2.

use std::path::{Path, PathBuf};
use std::process::Output;

use crate::models::normalize_path;
use crate::Result;

pub fn can_use_git_worktree(repo_root: &Path) -> bool {
    git(repo_root, &["rev-parse", "--show-toplevel"])
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn has_missing_registered_worktree(repo_root: &Path, workspace_path: &Path) -> bool {
    let target = normalize_path(workspace_path);
    if target.exists() {
        return false;
    }
    list_registered_worktrees(repo_root)
        .map(|worktrees| worktrees.into_iter().any(|path| path == target))
        .unwrap_or(false)
}

pub fn prune_missing_worktrees_under(repo_root: &Path, workspaces_root: &Path) -> Result<bool> {
    let missing: Vec<PathBuf> = list_registered_worktrees(repo_root)?
        .into_iter()
        .filter(|path| path_within(path, workspaces_root) && !path.exists())
        .collect();
    if missing.is_empty() {
        return Ok(false);
    }
    run_git(
        repo_root,
        &["worktree", "prune"],
        "failed to prune stale git worktrees",
    )?;
    Ok(true)
}

pub fn unregister_worktrees_under(repo_root: &Path, workspaces_root: &Path) -> Result<()> {
    let existing: Vec<PathBuf> = list_registered_worktrees(repo_root)?
        .into_iter()
        .filter(|path| path_within(path, workspaces_root) && path.exists())
        .collect();
    for path in existing {
        run_git(
            repo_root,
            &["worktree", "remove", "--force", &path.to_string_lossy()],
            &format!("failed to remove git worktree {}", path.display()),
        )?;
    }
    prune_missing_worktrees_under(repo_root, workspaces_root)?;
    Ok(())
}

pub fn branch_exists(repo_root: &Path, branch_name: &str) -> Result<bool> {
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

pub fn branch_is_merged_into_head(repo_root: &Path, branch_name: &str) -> Result<Option<bool>> {
    if !branch_exists(repo_root, branch_name)? {
        return Ok(None);
    }
    let output = git(
        repo_root,
        &["merge-base", "--is-ancestor", branch_name, "HEAD"],
    )?;
    match output.status.code() {
        Some(0) => Ok(Some(true)),
        Some(1) => Ok(Some(false)),
        _ => Err(crate::WorkspaceError::Workspace(
            detail(&output)
                .unwrap_or_else(|| format!("failed to inspect merge state for {branch_name}")),
        )),
    }
}

pub fn delete_branch(repo_root: &Path, branch_name: &str) -> Result<bool> {
    if !branch_exists(repo_root, branch_name)? {
        return Ok(false);
    }
    run_git(
        repo_root,
        &["branch", "-d", branch_name],
        &format!("failed to delete merged branch {branch_name}"),
    )?;
    Ok(true)
}

pub fn list_registered_worktrees(repo_root: &Path) -> Result<Vec<PathBuf>> {
    if !can_use_git_worktree(repo_root) {
        return Ok(Vec::new());
    }
    let output = git(repo_root, &["worktree", "list", "--porcelain"])?;
    if !output.status.success() {
        return Err(crate::WorkspaceError::Workspace(
            detail(&output).unwrap_or_else(|| "failed to list git worktrees".to_string()),
        ));
    }
    let mut worktrees = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("worktree ") {
            worktrees.push(normalize_path(Path::new(rest.trim())));
        }
    }
    Ok(worktrees)
}

pub fn is_registered_worktree(repo_root: &Path, workspace_path: &Path) -> bool {
    let target = normalize_path(workspace_path);
    list_registered_worktrees(repo_root)
        .map(|worktrees| worktrees.into_iter().any(|path| path == target))
        .unwrap_or(false)
}

pub fn is_git_workspace_root(workspace_path: &Path) -> bool {
    workspace_path.join(".git").exists()
}

pub fn workspace_is_dirty(workspace_path: &Path) -> Result<Option<bool>> {
    let target = normalize_path(workspace_path);
    if !target.exists() || !is_git_workspace_root(&target) {
        return Ok(None);
    }
    let output = std::process::Command::new("git")
        .args(["-C", &target.to_string_lossy(), "status", "--porcelain"])
        .output()?;
    if !output.status.success() {
        return Err(crate::WorkspaceError::Workspace(
            detail(&output).unwrap_or_else(|| {
                format!("failed to inspect workspace status: {}", target.display())
            }),
        ));
    }
    Ok(Some(
        !String::from_utf8_lossy(&output.stdout).trim().is_empty(),
    ))
}

pub fn remove_registered_worktree(repo_root: &Path, workspace_path: &Path) -> Result<bool> {
    let target = normalize_path(workspace_path);
    if is_registered_worktree(repo_root, &target) {
        if target.exists() {
            run_git(
                repo_root,
                &["worktree", "remove", "--force", &target.to_string_lossy()],
                &format!("failed to remove git worktree {}", target.display()),
            )?;
        } else {
            prune_missing_worktrees_under(repo_root, target.parent().unwrap_or(repo_root))?;
        }
        return Ok(true);
    }
    Ok(false)
}

fn path_within(path: &Path, parent: &Path) -> bool {
    let normalized_path = normalize_path(path);
    let normalized_parent = normalize_path(parent);
    normalized_path.strip_prefix(&normalized_parent).is_ok()
}

fn git(repo_root: &Path, args: &[&str]) -> Result<Output> {
    Ok(std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()?)
}

fn detail(output: &Output) -> Option<String> {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = format!("{}{}", stderr, stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn run_git(repo_root: &Path, args: &[&str], error: &str) -> Result<()> {
    let output = git(repo_root, args)?;
    if !output.status.success() {
        let detail = detail(&output).unwrap_or_default();
        return Err(crate::WorkspaceError::Workspace(format!(
            "{error}: {detail}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_within_detects_descendant() {
        assert!(path_within(Path::new("/a/b/c"), Path::new("/a/b")));
        assert!(!path_within(Path::new("/a/bc"), Path::new("/a/b")));
    }

    #[test]
    fn can_use_git_worktree_false_outside_repo() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!can_use_git_worktree(tmp.path()));
    }

    #[test]
    fn list_registered_worktrees_empty_outside_repo() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(list_registered_worktrees(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn workspace_is_dirty_none_outside_repo() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(workspace_is_dirty(tmp.path()).unwrap(), None);
    }

    #[test]
    #[ignore]
    fn branch_exists_finds_default_branch() {
        // Requires a git repository; run manually with a real repo.
    }
}
