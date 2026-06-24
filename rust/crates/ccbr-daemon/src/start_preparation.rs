//! Mirrors Python `lib/ccbd/start_preparation.py`.
//!
//! Prepares a set of agents for launch: persists specs, plans/materializes
//! workspaces, prepares provider homes/hooks, resolves bindings, and seeds
//! restore state.

use std::path::{Path, PathBuf};

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use ccbr_agents::models::{AgentSpec, ProjectConfig, RestoreMode};
use ccbr_agents::policy::resolve_agent_launch_policy;
use ccbr_agents::store::{AgentRestoreStore, AgentRuntimeStore, AgentSpecStore};
use ccbr_provider_profiles::materializer::validate_provider_runtime_home_uniqueness;
use ccbr_providers::workspace_preparation::prepare_provider_workspace;
use ccbr_storage::paths::PathLayout;
use ccbr_workspace::binding::WorkspaceBindingStore;
use ccbr_workspace::materializer::WorkspaceMaterializer;
use ccbr_workspace::models::WorkspacePlan;
use ccbr_workspace::planner::WorkspacePlanner;
use ccbr_workspace::validator::WorkspaceValidator;

/// A fully-prepared agent ready for runtime launch.
#[derive(Debug, Clone)]
pub struct PreparedStartAgent {
    pub agent_name: String,
    pub spec: AgentSpec,
    pub plan: WorkspacePlan,
    pub window_name: Option<String>,
    pub raw_binding: Option<AgentBinding>,
    pub binding: Option<AgentBinding>,
    pub stale_binding: bool,
}

/// Opaque runtime binding resolved for an agent.
#[derive(Debug, Clone)]
pub struct AgentBinding {
    pub agent_name: String,
    pub workspace_path: PathBuf,
}

/// Launch-time context supplied by the caller.
#[derive(Debug, Clone)]
pub struct StartContext {
    /// Explicit restore mode requested by the user (defaults to the spec's
    /// own default when `None`).
    pub restore_mode: Option<RestoreMode>,
    /// Whether the agent should run with auto-permission.
    pub auto_permission: bool,
    /// Project identifier.
    pub project_id: String,
    /// Project root path.
    pub project_root: PathBuf,
}

/// Resolves a raw runtime binding for a provider/agent/workspace tuple.
pub trait ResolveAgentBindingFn {
    fn resolve(
        &self,
        provider: &str,
        agent_name: &str,
        workspace_path: &Utf8Path,
        project_root: &Path,
        ensure_usable: bool,
    ) -> Option<AgentBinding>;
}

/// Filters a raw binding through the project tmux namespace.
pub trait ProjectBindingFilterFn {
    #[allow(clippy::too_many_arguments)]
    fn filter(
        &self,
        raw_binding: Option<&AgentBinding>,
        cmd_enabled: bool,
        tmux_socket_path: Option<&str>,
        tmux_session_name: Option<&str>,
        workspace_window_id: Option<&str>,
        agent_name: &str,
        project_id: &str,
        window_name: Option<&str>,
    ) -> Option<AgentBinding>;
}

/// Builds an initial restore-state record from an effective restore mode name.
pub trait RestoreStateBuilder {
    fn build(&self, mode: &str) -> ccbr_agents::models::AgentRestoreState;
}

/// Prepare the requested agents for starting.
///
/// Mirrors Python `start_preparation.prepare_start_agents`.
#[allow(clippy::too_many_arguments)]
pub fn prepare_start_agents(
    targets: &[String],
    config: &ProjectConfig,
    paths: &PathLayout,
    context: &StartContext,
    project_root: &Path,
    project_id: &str,
    tmux_socket_path: Option<&str>,
    tmux_session_name: Option<&str>,
    workspace_window_id: Option<&str>,
    resolve_agent_binding_fn: &dyn ResolveAgentBindingFn,
    project_binding_filter_fn: &dyn ProjectBindingFilterFn,
    restore_state_builder: &dyn RestoreStateBuilder,
) -> anyhow::Result<Vec<PreparedStartAgent>> {
    validate_provider_runtime_home_uniqueness(
        paths,
        config
            .agents
            .values()
            .map(|s| (s.name.as_str(), s.provider.as_str(), &s.provider_profile)),
    )
    .context("failed to validate provider runtime home uniqueness")?;

    let spec_store = AgentSpecStore::new(paths.clone());
    let runtime_store = AgentRuntimeStore::new(paths.clone());
    let restore_store = AgentRestoreStore::new(paths.clone());
    let planner = WorkspacePlanner::new();
    let materializer = WorkspaceMaterializer::new();
    let binding_store = WorkspaceBindingStore::new();
    let validator = WorkspaceValidator::with_binding_store(binding_store);

    let mut prepared = Vec::new();

    for agent_name in targets {
        let spec = config
            .agents
            .get(agent_name)
            .with_context(|| format!("Agent not found: {agent_name}"))?;

        let window_name = window_name_for_agent(config, agent_name);

        spec_store
            .save(spec)
            .with_context(|| format!("failed to save spec for {agent_name}"))?;

        let project_ctx = project_context(context);
        let plan = planner
            .plan(spec, &project_ctx)
            .with_context(|| format!("failed to plan workspace for {agent_name}"))?;
        let materialized = materializer
            .materialize(&plan)
            .with_context(|| format!("failed to materialize workspace for {agent_name}"))?;

        if plan.binding_path.is_some() {
            let store = WorkspaceBindingStore::new();
            store
                .save(&plan)
                .with_context(|| format!("failed to save workspace binding for {agent_name}"))?;
        }

        let runtime_dir = paths.agent_provider_runtime_dir(&spec.name, &spec.provider);
        let provider_workspace_path =
            provider_workspace_path_for_prepare(spec, &plan, &runtime_dir);

        prepare_provider_workspace(
            paths,
            spec,
            &provider_workspace_path,
            &runtime_dir.join("completion"),
            agent_name,
            true,
            context.auto_permission,
        )
        .with_context(|| format!("failed to prepare provider workspace for {agent_name}"))?;

        let validation = validator.validate(&plan);
        if !validation.ok {
            anyhow::bail!(
                "workspace validation failed for {agent_name}: {}",
                validation.errors.join("; ")
            );
        }

        let raw_binding = resolve_agent_binding_fn.resolve(
            &spec.provider,
            agent_name,
            &provider_workspace_path,
            project_root,
            false,
        );

        let binding = if let Some(socket_path) = tmux_socket_path {
            project_binding_filter_fn.filter(
                raw_binding.as_ref(),
                config.cmd_enabled,
                Some(socket_path),
                tmux_session_name,
                workspace_window_id,
                agent_name,
                project_id,
                window_name.as_deref(),
            )
        } else {
            resolve_agent_binding_fn.resolve(
                &spec.provider,
                agent_name,
                &provider_workspace_path,
                project_root,
                true,
            )
        };

        let stale_binding = raw_binding.is_some() && binding.is_none();

        if restore_store.load(agent_name)?.is_none() {
            let runtime = runtime_store.load(agent_name)?;
            let policy =
                resolve_agent_launch_policy(spec, runtime.as_ref(), context.restore_mode, None);
            let mode_str = effective_restore_mode_name(policy.restore_mode);
            restore_store
                .save(agent_name, &restore_state_builder.build(mode_str))
                .with_context(|| format!("failed to save restore state for {agent_name}"))?;
        }

        prepared.push(PreparedStartAgent {
            agent_name: agent_name.clone(),
            spec: spec.clone(),
            plan: WorkspacePlan::new(
                plan.project_id.clone(),
                materialized.workspace_path.clone(),
                plan.project_slug.clone(),
                plan.agent_name.clone(),
                plan.workspace_mode,
                materialized.workspace_path,
                plan.binding_path.clone(),
                plan.source_root.clone(),
                plan.branch_name.clone(),
                Some(plan.branch_template.clone()),
                plan.unsafe_shared_workspace,
                Some(plan.workspace_scope.clone()),
            )
            .with_context(|| format!("failed to rebuild plan for {agent_name}"))?,
            window_name,
            raw_binding,
            binding,
            stale_binding,
        });
    }

    Ok(prepared)
}

fn window_name_for_agent(config: &ProjectConfig, agent_name: &str) -> Option<String> {
    config.windows.as_ref().and_then(|windows| {
        windows
            .iter()
            .find(|w| w.agent_names.contains(&agent_name.to_string()))
            .map(|w| w.name.trim())
            .filter(|n| !n.is_empty())
            .map(String::from)
    })
}

fn project_context(context: &StartContext) -> ccbr_project::resolver::ProjectContext {
    let project_root = Utf8PathBuf::from_path_buf(context.project_root.clone())
        .unwrap_or_else(|_| Utf8PathBuf::from("/"));
    let config_dir = project_root.join(".ccbr");
    ccbr_project::resolver::ProjectContext {
        cwd: project_root.clone(),
        project_root,
        config_dir,
        project_id: context.project_id.clone(),
        source: "daemon".to_string(),
    }
}

fn provider_workspace_path_for_prepare(
    spec: &AgentSpec,
    plan: &WorkspacePlan,
    _runtime_dir: &Utf8Path,
) -> Utf8PathBuf {
    // TODO: provider-specific `resolve_run_cwd` (e.g. Codex nested cwd) is not
    // yet ported from Python. For now we always use the materialized workspace
    // path, which matches the common case.
    let _ = spec.runtime_mode;
    Utf8PathBuf::from_path_buf(plan.workspace_path.clone())
        .unwrap_or_else(|_| Utf8PathBuf::from("/tmp"))
}

fn effective_restore_mode_name(mode: ccbr_agents::policy::EffectiveRestoreMode) -> &'static str {
    match mode {
        ccbr_agents::policy::EffectiveRestoreMode::Fresh => "fresh",
        ccbr_agents::policy::EffectiveRestoreMode::Provider => "provider",
        ccbr_agents::policy::EffectiveRestoreMode::Attach => "attach",
        ccbr_agents::policy::EffectiveRestoreMode::Memory => "memory",
        ccbr_agents::policy::EffectiveRestoreMode::Auto => "auto",
    }
}
