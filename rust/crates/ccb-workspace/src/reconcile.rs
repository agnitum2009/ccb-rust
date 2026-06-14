//! Workspace reconciliation.
//!
//! Mirrors `workspace.reconcile` from Python v7.5.2.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use camino::Utf8PathBuf;
use ccb_agents::models::{AgentSpec, ProjectConfig, WorkspaceMode};
use ccb_agents::store::AgentSpecStore;
use ccb_project::identity::compute_project_id;
use ccb_project::resolver::ProjectContext;
use ccb_storage::paths::PathLayout;

use crate::git_worktree::{
    branch_is_merged_into_head, delete_branch, is_registered_worktree, remove_registered_worktree,
    workspace_is_dirty,
};
use crate::models::normalize_path;
use crate::planner::WorkspacePlanner;
use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorktreeAlert {
    pub agent_name: String,
    pub branch_name: Option<String>,
    pub workspace_path: String,
    pub dirty: Option<bool>,
    pub merged: Option<bool>,
    pub registered: bool,
    pub exists: bool,
    pub reason: String,
}

impl WorktreeAlert {
    pub fn needs_merge(&self) -> bool {
        self.dirty == Some(true) || self.merged == Some(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRetirement {
    pub agent_name: String,
    pub branch_name: Option<String>,
    pub workspace_path: String,
    pub reason: String,
    pub removed_agent_state: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceGuardSummary {
    pub warnings: Vec<WorktreeAlert>,
    pub blockers: Vec<WorktreeAlert>,
    pub retired: Vec<WorkspaceRetirement>,
}

pub fn reconcile_start_workspaces(
    project_root: &Path,
    config: &ProjectConfig,
) -> Result<WorkspaceGuardSummary> {
    let root = resolve_path(project_root);
    let paths = PathLayout::new(
        Utf8PathBuf::from_path_buf(root.clone()).unwrap_or_else(|_| Utf8PathBuf::from("/")),
    );
    let project_ctx = project_context(&root);
    let persisted_specs = load_persisted_specs(&paths)?;
    let desired_specs = config.agents.clone();

    let mut warnings: Vec<WorktreeAlert> = Vec::new();
    let mut blockers: Vec<WorktreeAlert> = Vec::new();
    let mut pending_retirements: Vec<(AgentSpec, String, bool, bool)> = Vec::new();
    let mut pending_state_cleanup: Vec<(String, String)> = Vec::new();

    for (agent_name, persisted_spec) in &persisted_specs {
        let desired_spec = desired_specs.get(agent_name);
        if persisted_spec.workspace_mode == WorkspaceMode::GitWorktree {
            let persisted_plan = WorkspacePlanner::new().plan(persisted_spec, &project_ctx)?;
            let reason = retirement_reason(persisted_spec, desired_spec, &project_ctx);
            if reason.is_none() {
                continue;
            }
            if persisted_plan.workspace_scope == "external" {
                if desired_spec.is_none() {
                    pending_state_cleanup
                        .push((agent_name.clone(), "removed_from_config".to_string()));
                }
                continue;
            }
            let alert = inspect_worktree(
                &root,
                &project_ctx,
                persisted_spec,
                reason.as_ref().unwrap(),
            )?;
            if alert.needs_merge() {
                blockers.push(alert);
                continue;
            }
            let remove_workspace = !workspace_referenced_by_other_desired_agent(
                persisted_spec,
                &desired_specs,
                &project_ctx,
            )?;
            pending_retirements.push((
                persisted_spec.clone(),
                reason.unwrap(),
                desired_spec.is_none(),
                remove_workspace,
            ));
            continue;
        }
        if desired_spec.is_none() {
            pending_state_cleanup.push((agent_name.clone(), "removed_from_config".to_string()));
        }
    }

    for spec in desired_specs.values() {
        if spec.workspace_mode != WorkspaceMode::GitWorktree {
            continue;
        }
        let plan = WorkspacePlanner::new().plan(spec, &project_ctx)?;
        if plan.workspace_scope == "external" {
            continue;
        }
        let alert = inspect_worktree(&root, &project_ctx, spec, "active_worktree")?;
        if alert.needs_merge() {
            warnings.push(alert);
        }
    }

    if !blockers.is_empty() {
        return Ok(WorkspaceGuardSummary {
            warnings,
            blockers,
            retired: Vec::new(),
        });
    }

    let mut retired: Vec<WorkspaceRetirement> = Vec::new();
    for (spec, reason, remove_agent_state, remove_workspace) in pending_retirements {
        retired.push(retire_worktree_spec(
            &root,
            &paths,
            &project_ctx,
            &spec,
            &reason,
            remove_agent_state,
            remove_workspace,
        )?);
    }
    for (agent_name, reason) in pending_state_cleanup {
        remove_agent_state_dir(&paths, &agent_name)?;
        retired.push(WorkspaceRetirement {
            agent_name,
            branch_name: None,
            workspace_path: String::new(),
            reason,
            removed_agent_state: true,
        });
    }

    Ok(WorkspaceGuardSummary {
        warnings,
        blockers: Vec::new(),
        retired,
    })
}

pub fn prepare_reset_workspaces(project_root: &Path, apply: bool) -> Result<WorkspaceGuardSummary> {
    let root = resolve_path(project_root);
    let paths = PathLayout::new(
        Utf8PathBuf::from_path_buf(root.clone()).unwrap_or_else(|_| Utf8PathBuf::from("/")),
    );
    let project_ctx = project_context(&root);

    let mut blockers: Vec<WorktreeAlert> = Vec::new();
    let mut pending_retirements: Vec<AgentSpec> = Vec::new();
    for spec in collect_reset_worktree_specs(&root, &paths, &project_ctx)? {
        let alert = inspect_worktree(&root, &project_ctx, &spec, "reset_context")?;
        if alert.needs_merge() {
            blockers.push(alert);
            continue;
        }
        pending_retirements.push(spec);
    }

    if !blockers.is_empty() {
        return Ok(WorkspaceGuardSummary {
            blockers,
            ..WorkspaceGuardSummary::default()
        });
    }

    if !apply {
        return Ok(WorkspaceGuardSummary::default());
    }

    let retired = pending_retirements
        .iter()
        .map(|spec| {
            retire_worktree_spec(
                &root,
                &paths,
                &project_ctx,
                spec,
                "reset_context",
                false,
                true,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(WorkspaceGuardSummary {
        retired,
        ..WorkspaceGuardSummary::default()
    })
}

pub fn inspect_kill_worktrees(project_root: &Path) -> Result<WorkspaceGuardSummary> {
    let root = resolve_path(project_root);
    let paths = PathLayout::new(
        Utf8PathBuf::from_path_buf(root.clone()).unwrap_or_else(|_| Utf8PathBuf::from("/")),
    );
    let project_ctx = project_context(&root);
    let warnings: Vec<WorktreeAlert> = load_persisted_specs(&paths)?
        .values()
        .filter(|spec| spec.workspace_mode == WorkspaceMode::GitWorktree)
        .filter_map(|spec| {
            let plan = WorkspacePlanner::new().plan(spec, &project_ctx).ok()?;
            if plan.workspace_scope == "external" {
                return None;
            }
            let alert = inspect_worktree(&root, &project_ctx, spec, "kill_warning").ok()?;
            if alert.needs_merge() {
                Some(alert)
            } else {
                None
            }
        })
        .collect();
    Ok(WorkspaceGuardSummary {
        warnings,
        ..WorkspaceGuardSummary::default()
    })
}

pub fn format_workspace_blockers(action: &str, blockers: &[WorktreeAlert]) -> String {
    let mut lines = vec![format!(
        "{action} blocked by unmerged or dirty worktree state:"
    )];
    for item in blockers {
        let branch = item.branch_name.as_deref().unwrap_or("<none>");
        let dirty = state_text(item.dirty);
        let merged = state_text(item.merged);
        lines.push(format!(
            "- agent={} reason={} branch={} dirty={} merged_into_head={} path={}",
            item.agent_name, item.reason, branch, dirty, merged, item.workspace_path
        ));
    }
    lines.push("merge or clean the listed worktree branches and retry".to_string());
    lines.join("\n")
}

fn retirement_reason(
    persisted_spec: &AgentSpec,
    desired_spec: Option<&AgentSpec>,
    project_ctx: &ProjectContext,
) -> Option<String> {
    if desired_spec.is_none() {
        return Some("removed_from_config".to_string());
    }
    let desired_spec = desired_spec.unwrap();
    if desired_spec.workspace_mode != WorkspaceMode::GitWorktree {
        return Some("workspace_mode_changed".to_string());
    }
    let planner = WorkspacePlanner::new();
    let current = planner.plan(desired_spec, project_ctx).ok()?;
    let persisted = planner.plan(persisted_spec, project_ctx).ok()?;
    if current.workspace_path != persisted.workspace_path
        || current.branch_name != persisted.branch_name
    {
        return Some("worktree_identity_changed".to_string());
    }
    None
}

fn inspect_worktree(
    project_root: &Path,
    project_ctx: &ProjectContext,
    spec: &AgentSpec,
    reason: &str,
) -> Result<WorktreeAlert> {
    let plan = WorkspacePlanner::new().plan(spec, project_ctx)?;
    let merged = if let Some(branch) = &plan.branch_name {
        branch_is_merged_into_head(project_root, branch)?
    } else {
        None
    };
    Ok(WorktreeAlert {
        agent_name: spec.name.clone(),
        branch_name: plan.branch_name.clone(),
        workspace_path: plan.workspace_path.to_string_lossy().to_string(),
        dirty: workspace_is_dirty(&plan.workspace_path)?,
        merged,
        registered: is_registered_worktree(project_root, &plan.workspace_path),
        exists: plan.workspace_path.exists(),
        reason: reason.to_string(),
    })
}

fn workspace_referenced_by_other_desired_agent(
    retired_spec: &AgentSpec,
    desired_specs: &HashMap<String, AgentSpec>,
    project_ctx: &ProjectContext,
) -> Result<bool> {
    let planner = WorkspacePlanner::new();
    let retired_plan = planner.plan(retired_spec, project_ctx)?;
    let retired_identity = workspace_identity(&retired_plan);
    for desired_spec in desired_specs.values() {
        if desired_spec.name == retired_spec.name {
            continue;
        }
        if desired_spec.workspace_mode != WorkspaceMode::GitWorktree {
            continue;
        }
        let desired_plan = planner.plan(desired_spec, project_ctx)?;
        if desired_plan.workspace_scope == "external" {
            continue;
        }
        if workspace_identity(&desired_plan) == retired_identity {
            return Ok(true);
        }
    }
    Ok(false)
}

fn workspace_identity(plan: &crate::models::WorkspacePlan) -> (String, String) {
    (
        resolve_path(&plan.workspace_path)
            .to_string_lossy()
            .to_string(),
        plan.branch_name.clone().unwrap_or_default(),
    )
}

fn retire_worktree_spec(
    project_root: &Path,
    paths: &PathLayout,
    project_ctx: &ProjectContext,
    spec: &AgentSpec,
    reason: &str,
    remove_agent_state: bool,
    remove_workspace: bool,
) -> Result<WorkspaceRetirement> {
    let plan = WorkspacePlanner::new().plan(spec, project_ctx)?;
    if remove_workspace && plan.workspace_scope != "external" {
        let removed = remove_registered_worktree(project_root, &plan.workspace_path)?;
        if !removed
            && path_within(&plan.workspace_path, paths.workspaces_dir().as_std_path())
            && plan.workspace_path.exists()
        {
            std::fs::remove_dir_all(&plan.workspace_path)?;
        }
        if let Some(branch) = &plan.branch_name {
            let _ = delete_branch(project_root, branch);
        }
    }
    if remove_agent_state {
        remove_agent_state_dir(paths, &spec.name)?;
    }
    Ok(WorkspaceRetirement {
        agent_name: spec.name.clone(),
        branch_name: plan.branch_name.clone(),
        workspace_path: plan.workspace_path.to_string_lossy().to_string(),
        reason: reason.to_string(),
        removed_agent_state: remove_agent_state,
    })
}

fn remove_agent_state_dir(paths: &PathLayout, agent_name: &str) -> Result<()> {
    for target in [
        paths.agent_dir(agent_name),
        paths.agent_mailbox_dir(agent_name),
    ] {
        let std_path = target.as_std_path();
        if std_path.is_symlink() || std_path.is_file() {
            std::fs::remove_file(std_path)?;
        } else if std_path.is_dir() {
            std::fs::remove_dir_all(std_path)?;
        }
    }
    Ok(())
}

fn load_persisted_specs(paths: &PathLayout) -> Result<HashMap<String, AgentSpec>> {
    let store = AgentSpecStore::new(paths.clone());
    let mut specs = HashMap::new();
    let agents_dir = paths.agents_dir();
    let std_dir = agents_dir.as_std_path();
    if !std_dir.is_dir() {
        return Ok(specs);
    }
    let mut children: Vec<_> = std::fs::read_dir(std_dir)?.flatten().collect();
    children.sort_by_key(|a| a.file_name());
    for entry in children {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if let Ok(Some(spec)) = store.load(&name) {
            specs.insert(spec.name.clone(), spec);
        }
    }
    Ok(specs)
}

fn collect_reset_worktree_specs(
    project_root: &Path,
    paths: &PathLayout,
    project_ctx: &ProjectContext,
) -> Result<Vec<AgentSpec>> {
    let mut specs: Vec<AgentSpec> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();

    let mut append = |spec: &AgentSpec| -> Result<()> {
        if spec.workspace_mode != WorkspaceMode::GitWorktree {
            return Ok(());
        }
        let plan = WorkspacePlanner::new().plan(spec, project_ctx)?;
        if plan.workspace_scope == "external" {
            return Ok(());
        }
        let identity = (
            resolve_path(&plan.workspace_path)
                .to_string_lossy()
                .to_string(),
            plan.branch_name.clone().unwrap_or_default(),
        );
        if seen.contains(&identity) {
            return Ok(());
        }
        seen.insert(identity);
        specs.push(spec.clone());
        Ok(())
    };

    for spec in load_persisted_specs(paths)?.values() {
        append(spec)?;
    }
    for spec in load_current_config_specs(project_root)? {
        append(&spec)?;
    }
    Ok(specs)
}

fn load_current_config_specs(project_root: &Path) -> Result<Vec<AgentSpec>> {
    let root = resolve_path(project_root);
    let layout = PathLayout::new(
        Utf8PathBuf::from_path_buf(root).unwrap_or_else(|_| Utf8PathBuf::from("/")),
    );
    let result =
        ccb_agents::config::load_project_config(&layout).map_err(crate::WorkspaceError::Agents)?;
    Ok(result.config.agents.values().cloned().collect())
}

fn project_context(project_root: &Path) -> ProjectContext {
    let root = resolve_path(project_root);
    ProjectContext {
        cwd: Utf8PathBuf::from_path_buf(root.clone()).unwrap_or_else(|_| Utf8PathBuf::from("/")),
        project_root: Utf8PathBuf::from_path_buf(root.clone())
            .unwrap_or_else(|_| Utf8PathBuf::from("/")),
        config_dir: Utf8PathBuf::from_path_buf(root.clone())
            .unwrap_or_else(|_| Utf8PathBuf::from("/"))
            .join(".ccb"),
        project_id: compute_project_id(root.to_string_lossy().as_ref()),
        source: "workspace-reconcile".to_string(),
    }
}

fn path_within(path: &Path, parent: &Path) -> bool {
    resolve_path(path)
        .strip_prefix(resolve_path(parent))
        .is_ok()
}

fn resolve_path(path: &Path) -> PathBuf {
    normalize_path(path)
}

fn state_text(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccb_agents::models::{AgentSpec, WorkspaceMode};

    fn spec(name: &str, mode: WorkspaceMode) -> AgentSpec {
        AgentSpec {
            name: name.to_string(),
            provider: "codex".to_string(),
            target: ".".to_string(),
            workspace_mode: mode,
            ..AgentSpec::default_with_name(name)
        }
    }

    #[test]
    fn format_blockers_includes_action_and_item() {
        let alert = WorktreeAlert {
            agent_name: "agent1".to_string(),
            branch_name: Some("ccb/agent1".to_string()),
            workspace_path: "/tmp/project/.ccb/workspaces/agent1".to_string(),
            dirty: Some(true),
            merged: Some(false),
            registered: true,
            exists: true,
            reason: "active_worktree".to_string(),
        };
        let text = format_workspace_blockers("start", &[alert]);
        assert!(text.contains("start blocked"));
        assert!(text.contains("agent=agent1"));
        assert!(text.contains("dirty=true"));
        assert!(text.contains("merged_into_head=false"));
    }

    #[test]
    fn worktree_alert_needs_merge_logic() {
        let dirty = WorktreeAlert {
            dirty: Some(true),
            merged: None,
            ..Default::default()
        };
        assert!(dirty.needs_merge());
        let unmerged = WorktreeAlert {
            dirty: Some(false),
            merged: Some(false),
            ..Default::default()
        };
        assert!(unmerged.needs_merge());
        let clean = WorktreeAlert {
            dirty: Some(false),
            merged: Some(true),
            ..Default::default()
        };
        assert!(!clean.needs_merge());
    }

    #[test]
    fn retirement_reason_detects_removed() {
        let persisted = spec("agent1", WorkspaceMode::GitWorktree);
        let reason = retirement_reason(
            &persisted,
            None,
            &project_context(Path::new("/tmp/project")),
        );
        assert_eq!(reason, Some("removed_from_config".to_string()));
    }

    #[test]
    fn prepare_reset_without_apply_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let summary = prepare_reset_workspaces(tmp.path(), false).unwrap();
        assert!(summary.blockers.is_empty());
        assert!(summary.retired.is_empty());
    }

    #[test]
    fn inspect_kill_worktrees_empty_without_specs() {
        let tmp = tempfile::tempdir().unwrap();
        let summary = inspect_kill_worktrees(tmp.path()).unwrap();
        assert!(summary.warnings.is_empty());
    }
}
