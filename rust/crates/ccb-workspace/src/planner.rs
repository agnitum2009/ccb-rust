//! Workspace planning.
//!
//! Mirrors `workspace.planner` from Python v7.5.2.

use std::collections::HashSet;
use std::path::PathBuf;

use camino::Utf8PathBuf;
use ccb_agents::models::{AgentSpec, WorkspaceMode};
use ccb_project::resolver::ProjectContext;
use ccb_storage::paths::PathLayout;

use crate::models::{expand_user_path_str, WorkspacePlan};
use crate::Result;

const DEFAULT_BRANCH_TEMPLATE: &str = "ccb/{agent_name}";
const ALLOWED_BRANCH_VARS: &[&str] = &["agent_name", "project_slug", "date"];

pub struct WorkspacePlanner;

impl WorkspacePlanner {
    pub fn new() -> Self {
        Self
    }

    pub fn plan(
        &self,
        agent_spec: &AgentSpec,
        project_ctx: &ProjectContext,
    ) -> Result<WorkspacePlan> {
        let layout = PathLayout::new(
            Utf8PathBuf::from_path_buf(project_ctx.project_root.as_std_path().to_path_buf())
                .unwrap_or_else(|_| Utf8PathBuf::from("/")),
        );
        let (workspace_path, binding_path, unsafe_shared, branch_name, scope) =
            match agent_spec.workspace_mode {
                WorkspaceMode::Inplace => (
                    PathBuf::from(project_ctx.project_root.as_str()),
                    None,
                    true,
                    None,
                    "inplace".to_string(),
                ),
                _ if agent_spec.workspace_path.is_some() => {
                    let raw = agent_spec.workspace_path.as_ref().unwrap();
                    (
                        PathBuf::from(expand_user_path_str(raw)),
                        None,
                        false,
                        None,
                        "external".to_string(),
                    )
                }
                _ if agent_spec.workspace_group.is_some() => {
                    let group = agent_spec.workspace_group.as_ref().unwrap();
                    (
                        PathBuf::from(layout.workspace_group_path(group).as_str()),
                        Some(PathBuf::from(
                            layout.workspace_group_binding_path(group).as_str(),
                        )),
                        false,
                        Some(format!("ccb/group/{group}")),
                        "group".to_string(),
                    )
                }
                _ => {
                    let path = PathBuf::from(
                        layout
                            .workspace_path(&agent_spec.name, agent_spec.workspace_root.as_deref())
                            .as_str(),
                    );
                    let binding = PathBuf::from(
                        layout
                            .workspace_binding_path(
                                &agent_spec.name,
                                agent_spec.workspace_root.as_deref(),
                            )
                            .as_str(),
                    );
                    let branch = self.render_branch_name(agent_spec, &layout.project_slug())?;
                    let branch_name = if agent_spec.workspace_mode == WorkspaceMode::Copy {
                        None
                    } else {
                        Some(branch)
                    };
                    (path, Some(binding), false, branch_name, "agent".to_string())
                }
            };

        WorkspacePlan::new(
            project_ctx.project_id.clone(),
            PathBuf::from(project_ctx.project_root.as_str()),
            layout.project_slug(),
            agent_spec.name.clone(),
            agent_spec.workspace_mode,
            workspace_path,
            binding_path,
            PathBuf::from(project_ctx.project_root.as_str()),
            branch_name,
            agent_spec.branch_template.clone(),
            unsafe_shared,
            Some(scope),
        )
    }

    fn render_branch_name(&self, agent_spec: &AgentSpec, project_slug: &str) -> Result<String> {
        let template = agent_spec
            .branch_template
            .clone()
            .unwrap_or_else(|| DEFAULT_BRANCH_TEMPLATE.to_string());
        let variables = extract_template_vars(&template);
        let unknown: Vec<&String> = variables
            .iter()
            .filter(|v| !ALLOWED_BRANCH_VARS.contains(&v.as_str()))
            .collect();
        if !unknown.is_empty() {
            return Err(crate::WorkspaceError::Validation(format!(
                "branch_template contains unsupported variables: {}",
                unknown
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        let date = chrono::Utc::now().format("%Y%m%d").to_string();
        let rendered = template
            .replace("{agent_name}", &agent_spec.name)
            .replace("{project_slug}", project_slug)
            .replace("{date}", &date);
        let branch_name = rendered.trim();
        if branch_name.is_empty() {
            return Err(crate::WorkspaceError::Validation(
                "branch_template rendered empty branch name".to_string(),
            ));
        }
        Ok(branch_name.to_string())
    }
}

impl Default for WorkspacePlanner {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_template_vars(template: &str) -> HashSet<String> {
    let mut vars = HashSet::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '{' {
            continue;
        }
        let mut name = String::new();
        while let Some(&next) = chars.peek() {
            if next == '}' {
                chars.next();
                break;
            }
            if !next.is_ascii_lowercase() && next != '_' {
                name.clear();
                break;
            }
            name.push(next);
            chars.next();
        }
        if !name.is_empty() {
            vars.insert(name);
        }
    }
    vars
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccb_agents::models::AgentSpec;

    fn spec(name: &str, mode: WorkspaceMode) -> AgentSpec {
        AgentSpec {
            name: name.to_string(),
            provider: "codex".to_string(),
            target: ".".to_string(),
            workspace_mode: mode,
            ..AgentSpec::default_with_name(name)
        }
    }

    fn ctx(root: &str, pid: &str) -> ProjectContext {
        ProjectContext {
            cwd: Utf8PathBuf::from(root),
            project_root: Utf8PathBuf::from(root),
            config_dir: Utf8PathBuf::from(root).join(".ccb"),
            project_id: pid.to_string(),
            source: "test".to_string(),
        }
    }

    #[test]
    fn planner_inplace_uses_project_root() {
        let spec = spec("agent1", WorkspaceMode::Inplace);
        let ctx = ctx("/tmp/project", "pid");
        let plan = WorkspacePlanner::new().plan(&spec, &ctx).unwrap();
        assert_eq!(plan.workspace_mode, WorkspaceMode::Inplace);
        assert!(plan.unsafe_shared_workspace);
        assert_eq!(plan.workspace_scope, "inplace");
        assert!(plan.branch_name.is_none());
    }

    #[test]
    fn planner_external_path_skips_branch() {
        let mut spec = spec("agent1", WorkspaceMode::GitWorktree);
        spec.workspace_path = Some("/external/workspace".to_string());
        let ctx = ctx("/tmp/project", "pid");
        let plan = WorkspacePlanner::new().plan(&spec, &ctx).unwrap();
        assert_eq!(plan.workspace_scope, "external");
        assert!(plan.branch_name.is_none());
        assert!(plan.binding_path.is_none());
    }

    #[test]
    fn planner_renders_default_branch_name() {
        let spec = spec("agent1", WorkspaceMode::GitWorktree);
        let ctx = ctx("/tmp/project", "pid");
        let plan = WorkspacePlanner::new().plan(&spec, &ctx).unwrap();
        assert!(plan.branch_name.as_ref().unwrap().starts_with("ccb/agent1"));
    }

    #[test]
    fn planner_copy_has_no_branch() {
        let spec = spec("agent1", WorkspaceMode::Copy);
        let ctx = ctx("/tmp/project", "pid");
        let plan = WorkspacePlanner::new().plan(&spec, &ctx).unwrap();
        assert_eq!(plan.workspace_mode, WorkspaceMode::Copy);
        assert!(plan.branch_name.is_none());
    }

    #[test]
    fn planner_rejects_unknown_branch_variable() {
        let mut spec = spec("agent1", WorkspaceMode::GitWorktree);
        spec.branch_template = Some("ccb/{unknown}".to_string());
        let ctx = ctx("/tmp/project", "pid");
        assert!(WorkspacePlanner::new().plan(&spec, &ctx).is_err());
    }

    #[test]
    fn extract_vars_finds_placeholders() {
        let vars = extract_template_vars("ccb/{agent_name}/{project_slug}/{date}");
        assert!(vars.contains("agent_name"));
        assert!(vars.contains("project_slug"));
        assert!(vars.contains("date"));
    }
}
