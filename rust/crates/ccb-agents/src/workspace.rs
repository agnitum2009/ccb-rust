use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use camino::Utf8PathBuf;
use ccb_storage::atomic::atomic_write_json;
use ccb_storage::paths::PathLayout;
use serde::{Deserialize, Serialize};

use crate::models::{AgentSpec, WorkspaceMode};
use crate::store::AgentSpecStore;

pub const SCHEMA_VERSION: u32 = crate::models::SCHEMA_VERSION;

#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub cwd: PathBuf,
    pub project_root: PathBuf,
    pub config_dir: PathBuf,
    pub project_id: String,
    pub source: String,
}

impl ProjectContext {
    pub fn new(project_root: impl Into<PathBuf>, project_id: impl Into<String>) -> Self {
        let root = project_root.into();
        Self {
            cwd: root.clone(),
            project_root: root.clone(),
            config_dir: root.join(".ccbr"),
            project_id: project_id.into(),
            source: "workspace".into(),
        }
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct WorkspaceRef {
    pub workspace_mode: WorkspaceMode,
    pub workspace_path: PathBuf,
    pub binding_path: Option<PathBuf>,
    pub branch_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}
fn default_record_type() -> String {
    "workspace_binding".into()
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub ok: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub diagnostics: HashMap<String, String>,
}

pub struct WorkspacePlanner;

const DEFAULT_BRANCH_TEMPLATE: &str = "ccb/{agent_name}";
const ALLOWED_BRANCH_VARS: &[&str] = &["agent_name", "project_slug", "date"];

impl WorkspacePlanner {
    pub fn new() -> Self {
        Self
    }

    pub fn plan(
        &self,
        agent_spec: &AgentSpec,
        project_ctx: &ProjectContext,
    ) -> crate::Result<WorkspacePlan> {
        let layout = PathLayout::new(
            Utf8PathBuf::from_path_buf(project_ctx.project_root.clone())
                .unwrap_or_else(|_| Utf8PathBuf::from("/")),
        );
        let (workspace_path, binding_path, unsafe_shared, branch_name, scope) = match agent_spec
            .workspace_mode
        {
            WorkspaceMode::Inplace => (
                project_ctx.project_root.clone(),
                None,
                true,
                None,
                "inplace".into(),
            ),
            _ if agent_spec.workspace_path.is_some() => {
                let path = expand_user_path(agent_spec.workspace_path.as_ref().unwrap());
                (PathBuf::from(path), None, false, None, "external".into())
            }
            _ if agent_spec.workspace_group.is_some() => {
                let group = agent_spec.workspace_group.as_ref().unwrap();
                let path: PathBuf = layout.workspace_group_path(group).into();
                let binding: PathBuf = layout.workspace_group_binding_path(group).into();
                (
                    path,
                    Some(binding),
                    false,
                    Some(format!("ccb/group/{group}")),
                    "group".into(),
                )
            }
            _ => {
                let path = layout
                    .workspace_path(&agent_spec.name, agent_spec.workspace_root.as_deref())
                    .into();
                let binding: PathBuf = layout
                    .workspace_binding_path(&agent_spec.name, agent_spec.workspace_root.as_deref())
                    .into();
                let branch = self.render_branch_name(agent_spec, &layout.project_slug())?;
                let scope = "agent".into();
                let branch_name = if agent_spec.workspace_mode == WorkspaceMode::Copy {
                    None
                } else {
                    Some(branch)
                };
                (path, Some(binding), false, branch_name, scope)
            }
        };

        Ok(WorkspacePlan {
            project_id: project_ctx.project_id.clone(),
            project_root: project_ctx.project_root.clone(),
            project_slug: layout.project_slug(),
            agent_name: crate::models::normalize_agent_name(&agent_spec.name)?,
            workspace_mode: agent_spec.workspace_mode,
            workspace_path,
            binding_path,
            source_root: project_ctx.project_root.clone(),
            branch_name,
            branch_template: agent_spec
                .branch_template
                .clone()
                .unwrap_or_else(|| DEFAULT_BRANCH_TEMPLATE.into()),
            unsafe_shared_workspace: unsafe_shared,
            workspace_scope: scope,
        })
    }

    fn render_branch_name(
        &self,
        agent_spec: &AgentSpec,
        project_slug: &str,
    ) -> crate::Result<String> {
        let template = agent_spec
            .branch_template
            .clone()
            .unwrap_or_else(|| DEFAULT_BRANCH_TEMPLATE.into());
        let vars = extract_template_vars(&template);
        let unknown: Vec<String> = vars
            .iter()
            .filter(|v| !ALLOWED_BRANCH_VARS.contains(&v.as_str()))
            .cloned()
            .collect();
        if !unknown.is_empty() {
            return Err(crate::AgentError::Validation(format!(
                "branch_template contains unsupported variables: {}",
                unknown.join(", ")
            )));
        }
        let date = format_date_today();
        let rendered = template
            .replace("{agent_name}", &agent_spec.name)
            .replace("{project_slug}", project_slug)
            .replace("{date}", &date);
        let branch_name = rendered.trim();
        if branch_name.is_empty() {
            return Err(crate::AgentError::Validation(
                "branch_template rendered empty branch name".into(),
            ));
        }
        Ok(branch_name.into())
    }
}

impl Default for WorkspacePlanner {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WorkspaceValidator {
    binding_store: WorkspaceBindingStore,
}

impl WorkspaceValidator {
    pub fn new() -> Self {
        Self {
            binding_store: WorkspaceBindingStore::new(),
        }
    }

    pub fn validate(&self, plan: &WorkspacePlan) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let diagnostics: HashMap<String, String> = [
            (
                "workspace_path".into(),
                plan.workspace_path.to_string_lossy().into_owned(),
            ),
            (
                "workspace_mode".into(),
                format!("{:?}", plan.workspace_mode),
            ),
        ]
        .into_iter()
        .collect();
        self.validate_workspace_mode(plan, &mut errors);
        self.validate_branch_requirements(plan, &mut errors);
        self.validate_binding(plan, &mut errors, &mut warnings);
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
                errors.push("inplace workspace_path must equal project_root".into());
            }
            if !plan.unsafe_shared_workspace {
                errors.push("inplace mode must be marked unsafe_shared_workspace".into());
            }
        } else if plan.workspace_path == plan.project_root {
            errors.push("non-inplace workspace must not reuse project_root".into());
        }
    }

    fn validate_branch_requirements(&self, plan: &WorkspacePlan, errors: &mut Vec<String>) {
        if plan.branch_name.is_none()
            && plan.workspace_mode == WorkspaceMode::GitWorktree
            && plan.workspace_scope != "external"
        {
            errors.push("git-worktree mode requires branch_name".into());
        }
    }

    fn validate_binding(
        &self,
        plan: &WorkspacePlan,
        errors: &mut Vec<String>,
        warnings: &mut Vec<String>,
    ) {
        if plan.workspace_path.exists() {
            if let Some(binding_path) = &plan.binding_path {
                if !binding_path.exists() {
                    warnings.push("workspace binding file is missing".into());
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
        binding: &WorkspaceBinding,
        plan: &WorkspacePlan,
        errors: &mut Vec<String>,
    ) {
        if PathBuf::from(&binding.target_project).expand_home() != plan.project_root {
            errors.push("workspace binding target_project does not match project_root".into());
        }
        if binding.project_id != plan.project_id {
            errors.push("workspace binding project_id does not match project_id".into());
        }
        if PathBuf::from(&binding.workspace_path).expand_home() != plan.workspace_path {
            errors.push("workspace binding workspace_path does not match workspace_path".into());
        }
        if binding.agent_name != plan.agent_name && plan.workspace_scope != "group" {
            errors.push("workspace binding agent_name does not match agent_name".into());
        }
    }
}

impl Default for WorkspaceValidator {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WorkspaceBindingStore;

impl WorkspaceBindingStore {
    pub fn new() -> Self {
        Self
    }

    pub fn load(&self, path: &Path) -> crate::Result<WorkspaceBinding> {
        let text = std::fs::read_to_string(path)?;
        let record: serde_json::Value = serde_json::from_str(&text)?;
        if record.get("schema_version").and_then(|v| v.as_u64()) != Some(SCHEMA_VERSION as u64) {
            return Err(crate::AgentError::Validation(
                "workspace binding schema_version must be 2".into(),
            ));
        }
        if record.get("record_type").and_then(|v| v.as_str()) != Some("workspace_binding") {
            return Err(crate::AgentError::Validation(
                "workspace binding record_type must be workspace_binding".into(),
            ));
        }
        let mode = record
            .get("workspace_mode")
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "git-worktree" => WorkspaceMode::GitWorktree,
                "copy" => WorkspaceMode::Copy,
                _ => WorkspaceMode::Inplace,
            })
            .unwrap_or(WorkspaceMode::Inplace);
        Ok(WorkspaceBinding {
            target_project: record
                .get("target_project")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            project_id: record
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            agent_name: record
                .get("agent_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            workspace_mode: mode,
            workspace_path: record
                .get("workspace_path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            branch_name: record
                .get("branch_name")
                .and_then(|v| v.as_str())
                .map(|s| s.into()),
            schema_version: SCHEMA_VERSION,
            record_type: "workspace_binding".into(),
        })
    }

    pub fn save(&self, plan: &WorkspacePlan) -> crate::Result<Option<PathBuf>> {
        let binding_path = match &plan.binding_path {
            Some(p) => p,
            None => return Ok(None),
        };
        let binding = WorkspaceBinding {
            target_project: plan.project_root.to_string_lossy().into_owned(),
            project_id: plan.project_id.clone(),
            agent_name: plan.agent_name.clone(),
            workspace_mode: plan.workspace_mode,
            workspace_path: plan.workspace_path.to_string_lossy().into_owned(),
            branch_name: plan.branch_name.clone(),
            schema_version: SCHEMA_VERSION,
            record_type: "workspace_binding".into(),
        };
        let utf8_path = camino::Utf8Path::from_path(binding_path).ok_or_else(|| {
            crate::AgentError::Workspace("binding path is not valid utf-8".into())
        })?;
        atomic_write_json(utf8_path, &binding)?;
        Ok(Some(binding_path.clone()))
    }
}

impl Default for WorkspaceBindingStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
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

    pub fn materialize(&self, plan: &WorkspacePlan) -> crate::Result<MaterializationResult> {
        match plan.workspace_mode {
            WorkspaceMode::Inplace => {
                std::fs::create_dir_all(&plan.workspace_path)?;
                Ok(MaterializationResult {
                    workspace_path: plan.workspace_path.clone(),
                    created: false,
                    mode: "inplace".into(),
                })
            }
            WorkspaceMode::Copy => self.materialize_copy(plan),
            WorkspaceMode::GitWorktree => self.materialize_git_worktree(plan),
        }
    }

    fn materialize_copy(&self, plan: &WorkspacePlan) -> crate::Result<MaterializationResult> {
        if self.is_placeholder_workspace(plan) {
            self.clear_placeholder_workspace(plan)?;
        }
        if plan.workspace_path.exists() {
            return Ok(MaterializationResult {
                workspace_path: plan.workspace_path.clone(),
                created: false,
                mode: "copy".into(),
            });
        }
        std::fs::create_dir_all(plan.workspace_path.parent().unwrap_or(&plan.workspace_path))?;
        copy_dir_contents(&plan.source_root, &plan.workspace_path)?;
        Ok(MaterializationResult {
            workspace_path: plan.workspace_path.clone(),
            created: true,
            mode: "copy".into(),
        })
    }

    fn materialize_git_worktree(
        &self,
        plan: &WorkspacePlan,
    ) -> crate::Result<MaterializationResult> {
        if plan.branch_name.is_none() && plan.workspace_scope != "external" {
            return Err(crate::AgentError::Validation(
                "git-worktree workspace requires branch_name".into(),
            ));
        }
        if !can_use_git_worktree(&plan.project_root) {
            return Err(crate::AgentError::Workspace(format!(
                "git-worktree workspace requires a git repository: {}",
                plan.project_root.display()
            )));
        }
        if plan.workspace_scope == "external" {
            self.validate_external_git_workspace(plan)?;
            return Ok(MaterializationResult {
                workspace_path: plan.workspace_path.clone(),
                created: false,
                mode: "git-worktree".into(),
            });
        }
        if self.is_existing_git_workspace(&plan.workspace_path) {
            self.validate_existing_git_workspace(plan)?;
            return Ok(MaterializationResult {
                workspace_path: plan.workspace_path.clone(),
                created: false,
                mode: "git-worktree".into(),
            });
        }
        if self.is_placeholder_workspace(plan) {
            self.clear_placeholder_workspace(plan)?;
        } else if plan.workspace_path.exists()
            && std::fs::read_dir(&plan.workspace_path)?.next().is_some()
        {
            return Err(crate::AgentError::Workspace(format!(
                "workspace path is not empty and is not a git worktree: {}",
                plan.workspace_path.display()
            )));
        }
        std::fs::create_dir_all(plan.workspace_path.parent().unwrap_or(&plan.workspace_path))?;
        self.prune_stale_worktree_registration(plan)?;
        self.run_git_worktree_add(plan, false)?;
        self.validate_existing_git_workspace(plan)?;
        Ok(MaterializationResult {
            workspace_path: plan.workspace_path.clone(),
            created: true,
            mode: "git-worktree".into(),
        })
    }

    fn run_git_worktree_add(&self, plan: &WorkspacePlan, force: bool) -> crate::Result<()> {
        let branch = plan.branch_name.as_ref().unwrap();
        let branch_exists = git_branch_exists(&plan.project_root, branch)?;
        let mut args: Vec<String> = vec![
            "-C".into(),
            plan.project_root.to_string_lossy().into_owned(),
            "worktree".into(),
            "add".into(),
        ];
        if force {
            args.push("-f".into());
        }
        if branch_exists {
            args.push(plan.workspace_path.to_string_lossy().into_owned());
            args.push(branch.clone());
        } else {
            args.push("-b".into());
            args.push(branch.clone());
            args.push(plan.workspace_path.to_string_lossy().into_owned());
            args.push("HEAD".into());
        }
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_git(&args_ref)?;
        Ok(())
    }

    fn prune_stale_worktree_registration(&self, plan: &WorkspacePlan) -> crate::Result<()> {
        if plan.workspace_path.exists() {
            return Ok(());
        }
        let (registered, prunable) =
            worktree_registration_status(&plan.project_root, &plan.workspace_path)?;
        if registered && prunable {
            run_git(&[
                "-C",
                &plan.project_root.to_string_lossy(),
                "worktree",
                "prune",
            ])?;
        }
        Ok(())
    }

    fn validate_existing_git_workspace(&self, plan: &WorkspacePlan) -> crate::Result<()> {
        let top_level = git_output(&plan.workspace_path, &["rev-parse", "--show-toplevel"])?;
        if PathBuf::from(top_level).expand_home() != plan.workspace_path {
            return Err(crate::AgentError::Workspace(format!(
                "workspace path is not the git worktree root: {}",
                plan.workspace_path.display()
            )));
        }
        if let Some(expected) = &plan.branch_name {
            let current = git_output(&plan.workspace_path, &["branch", "--show-current"])?;
            if !current.is_empty() && current != *expected {
                return Err(crate::AgentError::Workspace(format!(
                    "workspace branch mismatch for {}: expected {expected}, got {current}",
                    plan.agent_name
                )));
            }
        }
        Ok(())
    }

    fn validate_external_git_workspace(&self, plan: &WorkspacePlan) -> crate::Result<()> {
        if plan.workspace_path == plan.project_root {
            return Err(crate::AgentError::Workspace(
                "external workspace_path must not equal the project root; use workspace_mode=\"inplace\"".into(),
            ));
        }
        if !plan.workspace_path.exists() {
            return Err(crate::AgentError::Workspace(format!(
                "external workspace_path does not exist: {}",
                plan.workspace_path.display()
            )));
        }
        if !self.is_existing_git_workspace(&plan.workspace_path) {
            return Err(crate::AgentError::Workspace(format!(
                "external workspace_path is not a git workspace root: {}",
                plan.workspace_path.display()
            )));
        }
        self.validate_existing_git_workspace(plan)?;
        let project_common = git_common_dir(&plan.project_root)?;
        let workspace_common = git_common_dir(&plan.workspace_path)?;
        if project_common != workspace_common {
            return Err(crate::AgentError::Workspace(format!(
                "external workspace_path is not from the project git repository: {}",
                plan.workspace_path.display()
            )));
        }
        Ok(())
    }

    fn is_existing_git_workspace(&self, path: &Path) -> bool {
        let git_dir = path.join(".git");
        if !git_dir.exists() {
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
                    .unwrap_or(std::ffi::OsStr::new(".ccbr-workspace.json"))
                    .to_string_lossy()
                    .into_owned()]
            })
            .unwrap_or_default()
            .into_iter()
            .collect();
        if let Ok(entries) = std::fs::read_dir(&plan.workspace_path) {
            entries
                .flatten()
                .all(|e| allowed.contains(&e.file_name().to_string_lossy().into_owned()))
        } else {
            false
        }
    }

    fn clear_placeholder_workspace(&self, plan: &WorkspacePlan) -> crate::Result<()> {
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

fn copy_dir_contents(src: &Path, dst: &Path) -> crate::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let name = entry.file_name().to_string_lossy().into_owned();
        if file_type.is_dir() {
            if name == ".git" || name == ".ccbr" || name == "__pycache__" || name == ".pytest_cache"
            {
                continue;
            }
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn expand_user_path(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    raw.to_string()
}

trait ExpandHome {
    fn expand_home(&self) -> PathBuf;
}

impl ExpandHome for Path {
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

// --- Git worktree helpers ---

pub fn can_use_git_worktree(repo_root: &Path) -> bool {
    git(repo_root, &["rev-parse", "--show-toplevel"])
        .map(|r| r.status.success())
        .unwrap_or(false)
}

pub fn list_registered_worktrees(repo_root: &Path) -> crate::Result<Vec<PathBuf>> {
    if !can_use_git_worktree(repo_root) {
        return Ok(Vec::new());
    }
    let output = git(repo_root, &["worktree", "list", "--porcelain"])?;
    if !output.status.success() {
        return Err(crate::AgentError::Workspace(
            "failed to list git worktrees".into(),
        ));
    }
    let mut worktrees = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        if let Some(path_str) = trimmed.strip_prefix("worktree ") {
            worktrees.push(PathBuf::from(path_str.trim()).expand_home());
        }
    }
    Ok(worktrees)
}

pub fn is_registered_worktree(repo_root: &Path, workspace_path: &Path) -> bool {
    let target = workspace_path.expand_home();
    list_registered_worktrees(repo_root)
        .map(|v| v.into_iter().any(|p| p == target))
        .unwrap_or(false)
}

pub fn branch_exists(repo_root: &Path, branch_name: &str) -> crate::Result<bool> {
    git_branch_exists(repo_root, branch_name)
}

fn git_branch_exists(repo_root: &Path, branch_name: &str) -> crate::Result<bool> {
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

pub fn branch_is_merged_into_head(
    repo_root: &Path,
    branch_name: &str,
) -> crate::Result<Option<bool>> {
    if !git_branch_exists(repo_root, branch_name)? {
        return Ok(None);
    }
    let output = git(
        repo_root,
        &["merge-base", "--is-ancestor", branch_name, "HEAD"],
    )?;
    match output.status.code() {
        Some(0) => Ok(Some(true)),
        Some(1) => Ok(Some(false)),
        _ => Err(crate::AgentError::Workspace(format!(
            "failed to inspect merge state for {branch_name}"
        ))),
    }
}

pub fn delete_branch(repo_root: &Path, branch_name: &str) -> crate::Result<bool> {
    if !git_branch_exists(repo_root, branch_name)? {
        return Ok(false);
    }
    run_git(&[
        "-C",
        &repo_root.to_string_lossy(),
        "branch",
        "-d",
        branch_name,
    ])?;
    Ok(true)
}

pub fn remove_registered_worktree(repo_root: &Path, workspace_path: &Path) -> crate::Result<bool> {
    let target = workspace_path.expand_home();
    if is_registered_worktree(repo_root, &target) {
        if target.exists() {
            run_git(&[
                "-C",
                &repo_root.to_string_lossy(),
                "worktree",
                "remove",
                "--force",
                &target.to_string_lossy(),
            ])?;
        } else {
            prune_missing_worktrees_under(repo_root, target.parent().unwrap_or(repo_root))?;
        }
        return Ok(true);
    }
    Ok(false)
}

pub fn prune_missing_worktrees_under(
    repo_root: &Path,
    workspaces_root: &Path,
) -> crate::Result<bool> {
    let missing: Vec<PathBuf> = list_registered_worktrees(repo_root)?
        .into_iter()
        .filter(|p| path_within(p, workspaces_root) && !p.exists())
        .collect();
    if missing.is_empty() {
        return Ok(false);
    }
    run_git(&["-C", &repo_root.to_string_lossy(), "worktree", "prune"])?;
    Ok(true)
}

pub fn unregister_worktrees_under(repo_root: &Path, workspaces_root: &Path) -> crate::Result<()> {
    let existing: Vec<PathBuf> = list_registered_worktrees(repo_root)?
        .into_iter()
        .filter(|p| path_within(p, workspaces_root) && p.exists())
        .collect();
    for path in existing {
        run_git(&[
            "-C",
            &repo_root.to_string_lossy(),
            "worktree",
            "remove",
            "--force",
            &path.to_string_lossy(),
        ])?;
    }
    prune_missing_worktrees_under(repo_root, workspaces_root)?;
    Ok(())
}

pub fn workspace_is_dirty(workspace_path: &Path) -> crate::Result<Option<bool>> {
    let target = workspace_path.expand_home();
    if !target.exists() || !target.join(".git").exists() {
        return Ok(None);
    }
    let output = std::process::Command::new("git")
        .args(["-C", &target.to_string_lossy(), "status", "--porcelain"])
        .output()?;
    if !output.status.success() {
        return Err(crate::AgentError::Workspace(format!(
            "failed to inspect workspace status: {}",
            target.display()
        )));
    }
    Ok(Some(
        !String::from_utf8_lossy(&output.stdout).trim().is_empty(),
    ))
}

fn path_within(path: &Path, parent: &Path) -> bool {
    path.expand_home().starts_with(parent.expand_home())
}

fn git(repo_root: &Path, args: &[&str]) -> crate::Result<std::process::Output> {
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(repo_root).args(args);
    let output = command.output()?;
    Ok(output)
}

fn run_git(args: &[&str]) -> crate::Result<()> {
    let output = std::process::Command::new("git").args(args).output()?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(crate::AgentError::Workspace(format!(
            "git command failed: {detail}"
        )));
    }
    Ok(())
}

fn git_output(cwd: &Path, args: &[&str]) -> crate::Result<String> {
    let output = git(cwd, args)?;
    if !output.status.success() {
        return Err(crate::AgentError::Workspace("git command failed".into()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}

fn git_common_dir(cwd: &Path) -> crate::Result<PathBuf> {
    let raw = git_output(cwd, &["rev-parse", "--git-common-dir"])?;
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        Ok(path.expand_home())
    } else {
        Ok(cwd.join(path).expand_home())
    }
}

fn worktree_registration_status(
    repo_root: &Path,
    workspace_path: &Path,
) -> crate::Result<(bool, bool)> {
    let output = git(repo_root, &["worktree", "list", "--porcelain"])?;
    if !output.status.success() {
        return Ok((false, false));
    }
    let target = workspace_path.expand_home();
    let mut registered = false;
    let mut prunable = false;
    let text = String::from_utf8_lossy(&output.stdout);
    for block in text.split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        if lines.is_empty() || !lines[0].starts_with("worktree ") {
            continue;
        }
        let raw_path = lines[0]["worktree ".len()..].trim();
        let current = PathBuf::from(raw_path).expand_home();
        if current != target {
            continue;
        }
        registered = true;
        prunable = lines.iter().skip(1).any(|l| l.starts_with("prunable "));
        break;
    }
    Ok((registered, prunable))
}

// --- Reconcile ---

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct WorkspaceRetirement {
    pub agent_name: String,
    pub branch_name: Option<String>,
    pub workspace_path: String,
    pub reason: String,
    pub removed_agent_state: bool,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceGuardSummary {
    pub warnings: Vec<WorktreeAlert>,
    pub blockers: Vec<WorktreeAlert>,
    pub retired: Vec<WorkspaceRetirement>,
}

pub fn reconcile_start_workspaces(
    project_root: &Path,
    config: &crate::models::ProjectConfig,
) -> crate::Result<WorkspaceGuardSummary> {
    let root = resolve_path(project_root);
    let paths = PathLayout::new(
        Utf8PathBuf::from_path_buf(root.clone()).unwrap_or_else(|_| Utf8PathBuf::from("/")),
    );
    let project_ctx = project_context(&root);
    let persisted_specs = load_persisted_specs(&paths)?;
    let desired_specs = config.agents.clone();

    let mut warnings = Vec::new();
    let mut blockers = Vec::new();
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
                    pending_state_cleanup.push((agent_name.clone(), "removed_from_config".into()));
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
            pending_state_cleanup.push((agent_name.clone(), "removed_from_config".into()));
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

    let mut retired = Vec::new();
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

fn load_persisted_specs(paths: &PathLayout) -> crate::Result<HashMap<String, AgentSpec>> {
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
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Ok(Some(spec)) = store.load(&name) {
            specs.insert(spec.name.clone(), spec);
        }
    }
    Ok(specs)
}

fn project_context(project_root: &Path) -> ProjectContext {
    let root = resolve_path(project_root);
    ProjectContext::new(&root, "project-id")
}

fn resolve_path(path: &Path) -> PathBuf {
    let expanded = path.expand_home();
    expanded
        .canonicalize()
        .unwrap_or_else(|_| std::path::absolute(&expanded).unwrap_or(expanded))
}

fn retirement_reason(
    persisted_spec: &AgentSpec,
    desired_spec: Option<&AgentSpec>,
    project_ctx: &ProjectContext,
) -> Option<String> {
    if desired_spec.is_none() {
        return Some("removed_from_config".into());
    }
    let desired_spec = desired_spec.unwrap();
    if desired_spec.workspace_mode != WorkspaceMode::GitWorktree {
        return Some("workspace_mode_changed".into());
    }
    let planner = WorkspacePlanner::new();
    let current = planner.plan(desired_spec, project_ctx).ok()?;
    let persisted = planner.plan(persisted_spec, project_ctx).ok()?;
    if current.workspace_path != persisted.workspace_path
        || current.branch_name != persisted.branch_name
    {
        return Some("worktree_identity_changed".into());
    }
    None
}

fn inspect_worktree(
    project_root: &Path,
    project_ctx: &ProjectContext,
    spec: &AgentSpec,
    reason: &str,
) -> crate::Result<WorktreeAlert> {
    let plan = WorkspacePlanner::new().plan(spec, project_ctx)?;
    let merged = if let Some(branch) = &plan.branch_name {
        branch_is_merged_into_head(project_root, branch)?
    } else {
        None
    };
    Ok(WorktreeAlert {
        agent_name: spec.name.clone(),
        branch_name: plan.branch_name.clone(),
        workspace_path: plan.workspace_path.to_string_lossy().into_owned(),
        dirty: workspace_is_dirty(&plan.workspace_path)?,
        merged,
        registered: is_registered_worktree(project_root, &plan.workspace_path),
        exists: plan.workspace_path.exists(),
        reason: reason.into(),
    })
}

fn workspace_referenced_by_other_desired_agent(
    retired_spec: &AgentSpec,
    desired_specs: &HashMap<String, AgentSpec>,
    project_ctx: &ProjectContext,
) -> crate::Result<bool> {
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

fn workspace_identity(plan: &WorkspacePlan) -> (String, String) {
    (
        resolve_path(&plan.workspace_path)
            .to_string_lossy()
            .into_owned(),
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
) -> crate::Result<WorkspaceRetirement> {
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
        workspace_path: plan.workspace_path.to_string_lossy().into_owned(),
        reason: reason.into(),
        removed_agent_state: remove_agent_state,
    })
}

fn remove_agent_state_dir(paths: &PathLayout, agent_name: &str) -> crate::Result<()> {
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
    lines.push("merge or clean the listed worktree branches and retry".into());
    lines.join("\n")
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

fn format_date_today() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = now / 86_400;
    let (y, m, d) = epoch_days_to_ymd(days);
    format!("{y:04}{m:02}{d:02}")
}

fn epoch_days_to_ymd(days: i64) -> (i32, u32, u32) {
    // Based on the "days_from_civil" algorithm by Howard Hinnant (public domain).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (y as i32 + if m <= 2 { 1 } else { 0 }, m as u32, d as u32)
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

    fn spec(name: &str, mode: WorkspaceMode) -> AgentSpec {
        AgentSpec {
            name: name.into(),
            provider: "codex".into(),
            target: ".".into(),
            workspace_mode: mode,
            ..crate::models::AgentSpec::default_with_name(name)
        }
    }

    #[test]
    fn test_workspace_planner_inplace() {
        let spec = spec("agent1", WorkspaceMode::Inplace);
        let ctx = ProjectContext::new("/tmp/project", "pid");
        let plan = WorkspacePlanner::new().plan(&spec, &ctx).unwrap();
        assert_eq!(plan.workspace_mode, WorkspaceMode::Inplace);
        assert!(plan.unsafe_shared_workspace);
    }

    #[test]
    fn test_workspace_validator_inplace() {
        let spec = spec("agent1", WorkspaceMode::Inplace);
        let ctx = ProjectContext::new("/tmp/project", "pid");
        let plan = WorkspacePlanner::new().plan(&spec, &ctx).unwrap();
        let result = WorkspaceValidator::new().validate(&plan);
        assert!(result.ok);
    }

    #[test]
    fn test_workspace_validator_git_worktree_requires_branch() {
        let mut spec = spec("agent1", WorkspaceMode::GitWorktree);
        spec.workspace_path = Some("/external".into());
        let ctx = ProjectContext::new("/tmp/project", "pid");
        let plan = WorkspacePlanner::new().plan(&spec, &ctx).unwrap();
        let result = WorkspaceValidator::new().validate(&plan);
        assert!(result.ok);
    }
}
