//! Mirrors Python `lib/ccbd/start_preparation.py`.
//! 1:1 file alignment stub.

use std::collections::HashMap;
use std::path::PathBuf;

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

#[derive(Debug, Clone)]
pub struct AgentSpec {
    pub provider: String,
}

#[derive(Debug, Clone)]
pub struct WorkspacePlan {
    pub workspace_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AgentBinding {
    pub agent_name: String,
}

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub agents: HashMap<String, AgentSpec>,
    pub windows: Vec<WindowConfig>,
}

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub name: String,
    pub agent_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectPaths {
    pub base_path: PathBuf,
}

impl ProjectPaths {
    pub fn agent_provider_runtime_dir(&self, agent_name: &str, provider: &str) -> PathBuf {
        self.base_path
            .join("runtime")
            .join(provider)
            .join(agent_name)
    }
    pub fn provider_runtime_dir(&self, provider: &str) -> PathBuf {
        self.base_path.join("runtime").join(provider)
    }
}

#[derive(Debug, Clone)]
pub struct StartContext {
    pub restore: bool,
    pub auto_permission: bool,
    pub project_path: PathBuf,
}

/// Prepare agents for starting.
///
/// Arity mirrors the Python `start_preparation.prepare_start_agents` entrypoint.
#[allow(clippy::too_many_arguments)]
pub fn prepare_start_agents(
    targets: &[String],
    config: &ProjectConfig,
    _paths: &ProjectPaths,
    context: &StartContext,
    _project_root: &PathBuf,
    _project_id: &str,
    _tmux_socket_path: Option<&str>,
    _tmux_session_name: Option<&str>,
    _workspace_window_id: Option<&str>,
    _resolve_agent_binding_fn: &dyn ResolveAgentBindingFn,
    _project_binding_filter_fn: &dyn ProjectBindingFilterFn,
    _restore_state_builder: &dyn RestoreStateBuilder,
) -> Result<Vec<PreparedStartAgent>, String> {
    let mut prepared = Vec::new();

    for agent_name in targets {
        let spec = config
            .agents
            .get(agent_name)
            .ok_or_else(|| format!("Agent not found: {}", agent_name))?
            .clone();
        let window_name = window_name_for_agent(config, agent_name);

        let plan = WorkspacePlan {
            workspace_path: context.project_path.join("workspaces").join(&spec.provider),
        };

        std::fs::create_dir_all(&plan.workspace_path)
            .map_err(|e| format!("Failed to create workspace: {}", e))?;

        prepared.push(PreparedStartAgent {
            agent_name: agent_name.clone(),
            spec,
            plan,
            window_name,
            raw_binding: None,
            binding: None,
            stale_binding: false,
        });
    }

    Ok(prepared)
}

/// Get window name for an agent
fn window_name_for_agent(config: &ProjectConfig, agent_name: &str) -> Option<String> {
    for window in &config.windows {
        if window.agent_names.contains(&agent_name.to_string()) {
            let name = window.name.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

pub trait ResolveAgentBindingFn {}
pub trait ProjectBindingFilterFn {}
pub trait RestoreStateBuilder {}
