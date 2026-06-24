//! Mirrors Python `lib/ccbd/start_flow_runtime/service.py`.
//! 1:1 file alignment stub.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Summary of a start flow operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartFlowSummary {
    pub project_root: String,
    pub project_id: String,
    pub started: Vec<String>,
    pub socket_path: String,
    #[serde(default)]
    pub cleanup_summaries: Vec<String>,
    #[serde(default)]
    pub actions_taken: Vec<String>,
    #[serde(default)]
    pub agent_results: Vec<AgentResult>,
}

/// Result from a single agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub agent_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Context for start flow operation
pub struct StartFlowContext {
    pub project_root: PathBuf,
    pub project_id: String,
    pub requested_agents: Vec<String>,
    pub restore: bool,
    pub auto_permission: bool,
}

/// Configuration for start flow
pub trait StartFlowConfig {
    fn entry_window(&self) -> &str;
    fn topology_signature(&self) -> &str;
    fn windows(&self) -> Vec<WindowConfig>;
}

/// Runtime service interface
pub trait RuntimeService {
    fn start_agent(&mut self, agent_name: &str) -> Result<(), String>;
}

/// Dependencies for start flow
pub trait StartFlowDeps {
    fn build_project_layout_plan_fn(&self, config: &dyn StartFlowConfig, requested_agents: Vec<String>) -> LayoutPlan;
    fn tmux_namespace_runtime(&self, socket_path: Option<&str>, session_name: Option<&str>, workspace_window: Option<&str>) -> (TmuxBackend, String);
    fn start_agent_runtime_impl(&self, context: &StartContext, command: &StartCommand, runtime_service: &mut dyn RuntimeService, agent_name: &str, spec: &AgentSpec, plan: &AgentPlan, binding: &AgentBinding, raw_binding: &RawBinding, stale_binding: bool, assigned_pane_id: Option<&str>, style_index: usize, project_id: &str, tmux_socket_path: Option<&str>, namespace_epoch: Option<u64>, workspace_window_id: Option<&str>, workspace_epoch: Option<u64>, window_name: &str) -> AgentExecution;
    fn same_tmux_socket_path_fn(&self, path1: Option<&str>, path2: Option<&str>) -> bool;
}

/// Layout plan
#[derive(Debug, Clone)]
pub struct LayoutPlan {
    pub target_agent_names: Vec<String>,
}

/// Tmux backend
#[derive(Debug, Clone)]
pub struct TmuxBackend {
    pub socket_path: Option<String>,
}

/// Start context
#[derive(Debug, Clone)]
pub struct StartContext {
    pub project_id: String,
    pub agent_names: Vec<String>,
}

/// Start command
#[derive(Debug, Clone)]
pub struct StartCommand {
    pub agent_names: Vec<String>,
}

/// Agent spec
#[derive(Debug, Clone)]
pub struct AgentSpec {
    pub name: String,
    pub provider: String,
}

/// Agent plan
#[derive(Debug, Clone)]
pub struct AgentPlan {
    pub window_name: String,
}

/// Agent binding
#[derive(Debug, Clone)]
pub struct AgentBinding {
    pub binding_type: String,
}

/// Raw binding
#[derive(Debug, Clone)]
pub struct RawBinding {
    pub data: String,
}

/// Agent execution result
#[derive(Debug, Clone)]
pub struct AgentExecution {
    pub actions_taken: Vec<String>,
    pub agent_result: AgentResult,
    pub active_panes: Vec<String>,
}

/// Run the start flow
pub fn run_start_flow(
    project_root: &PathBuf,
    project_id: &str,
    paths: &FlowPaths,
    config: &dyn StartFlowConfig,
    runtime_service: &mut dyn RuntimeService,
    requested_agents: &[String],
    restore: bool,
    auto_permission: bool,
    cleanup_tmux_orphans: bool,
    interactive_tmux_layout: bool,
    tmux_socket_path: Option<&str>,
    tmux_session_name: Option<&str>,
    tmux_workspace_window_name: Option<&str>,
    namespace_epoch: Option<u64>,
    workspace_window_id: Option<&str>,
    workspace_epoch: Option<u64>,
    namespace_agent_panes: Option<&HashMap<String, String>>,
    namespace_active_panes: Option<&[String]>,
    fresh_namespace: bool,
    fresh_workspace: bool,
    deps: &dyn StartFlowDeps,
) -> Result<StartFlowSummary, String> {
    let context = StartContext {
        project_id: project_id.to_string(),
        agent_names: requested_agents.to_vec(),
    };

    let command = StartCommand {
        agent_names: requested_agents.to_vec(),
    };

    let layout_plan = deps.build_project_layout_plan_fn(config, requested_agents.to_vec());
    let targets = layout_plan.target_agent_names;

    let mut actions_taken: Vec<String> = vec![];
    let mut agent_results: Vec<AgentResult> = vec![];

    // Initialize tmux runtime
    let (tmux_backend, root_pane_id) = deps.tmux_namespace_runtime(
        tmux_socket_path,
        tmux_session_name,
        tmux_workspace_window_name,
    );

    // Record namespace action
    record_namespace_action(
        &mut actions_taken,
        tmux_socket_path,
        tmux_session_name,
        namespace_epoch,
    );

    // Prepare agents
    let prepared_agents = prepare_agents(
        deps,
        &targets,
        config,
        paths,
        &context,
        project_root,
        project_id,
        tmux_socket_path,
        tmux_session_name,
        workspace_window_id,
    );

    let prepared_by_agent: HashMap<String, PreparedAgent> = prepared_agents
        .into_iter()
        .map(|item| (item.agent_name.clone(), item))
        .collect();

    // Create tmux layout
    let tmux_layout = create_tmux_layout(
        deps,
        &context,
        config,
        &prepared_by_agent,
        interactive_tmux_layout,
        &tmux_backend,
        &root_pane_id,
        namespace_agent_panes,
        &mut actions_taken,
    );

    // Get active panes
    let active_project_panes = tmux_layout.active_panes.clone();
    let cmd_pane_id = tmux_layout.cmd_pane_id.clone();

    // Bootstrap command pane if needed
    if fresh_namespace || fresh_workspace {
        bootstrap_cmd_pane(
            deps,
            &cmd_pane_id,
            project_root,
            project_id,
            tmux_socket_path,
            namespace_epoch,
            &mut actions_taken,
        );
    }

    // Start agents
    for (style_index, agent_name) in targets.iter().enumerate() {
        let prepared = prepared_by_agent.get(agent_name).ok_or("Agent not prepared")?;

        let execution = deps.start_agent_runtime_impl(
            &context,
            &command,
            runtime_service,
            agent_name,
            &prepared.spec,
            &prepared.plan,
            &prepared.binding,
            &prepared.raw_binding,
            prepared.stale_binding,
            tmux_layout.agent_panes.get(agent_name).map(|s| s.as_str()),
            style_index,
            project_id,
            tmux_socket_path,
            namespace_epoch,
            workspace_window_id,
            workspace_epoch,
            &prepared.window_name,
        );

        actions_taken.extend(execution.actions_taken);
        agent_results.push(execution.agent_result);
    }

    // Cleanup tmux orphans if needed
    let cleanup_summaries = if cleanup_tmux_orphans {
        cleanup_tmux_orphans(
            deps,
            project_id,
            paths,
            &active_project_panes,
            tmux_socket_path,
            &mut actions_taken,
        )
    } else {
        vec![]
    };

    Ok(StartFlowSummary {
        project_root: project_root.to_string_lossy().to_string(),
        project_id: project_id.to_string(),
        started: targets,
        socket_path: paths.ccbd_socket_path.to_string_lossy().to_string(),
        cleanup_summaries,
        actions_taken,
        agent_results,
    })
}

/// Record namespace action
fn record_namespace_action(
    actions_taken: &mut Vec<String>,
    tmux_socket_path: Option<&str>,
    tmux_session_name: Option<&str>,
    namespace_epoch: Option<u64>,
) {
    let action = format!(
        "namespace_action: socket={:?}, session={:?}, epoch={:?}",
        tmux_socket_path, tmux_session_name, namespace_epoch
    );
    actions_taken.push(action);
}

/// Prepare agents for start
fn prepare_agents(
    deps: &dyn StartFlowDeps,
    targets: &[String],
    config: &dyn StartFlowConfig,
    paths: &FlowPaths,
    context: &StartContext,
    project_root: &PathBuf,
    project_id: &str,
    tmux_socket_path: Option<&str>,
    tmux_session_name: Option<&str>,
    workspace_window_id: Option<&str>,
) -> Vec<PreparedAgent> {
    targets
        .iter()
        .map(|agent_name| PreparedAgent {
            agent_name: agent_name.clone(),
            spec: AgentSpec {
                name: agent_name.clone(),
                provider: "tmux".to_string(),
            },
            plan: AgentPlan {
                window_name: agent_name.clone(),
            },
            binding: AgentBinding {
                binding_type: "tmux".to_string(),
            },
            raw_binding: RawBinding {
                data: "{}".to_string(),
            },
            stale_binding: false,
            window_name: agent_name.clone(),
        })
        .collect()
}

/// Create tmux layout
fn create_tmux_layout(
    deps: &dyn StartFlowDeps,
    context: &StartContext,
    config: &dyn StartFlowConfig,
    prepared_agents: &HashMap<String, PreparedAgent>,
    interactive: bool,
    tmux_backend: &TmuxBackend,
    root_pane_id: &str,
    namespace_agent_panes: Option<&HashMap<String, String>>,
    actions_taken: &mut Vec<String>,
) -> TmuxLayout {
    let mut agent_panes = HashMap::new();

    for agent_name in prepared_agents.keys() {
        agent_panes.insert(agent_name.clone(), format!("pane_{}", agent_name));
    }

    TmuxLayout {
        active_panes: vec![],
        cmd_pane_id: root_pane_id.to_string(),
        agent_panes,
    }
}

/// Bootstrap command pane
fn bootstrap_cmd_pane(
    deps: &dyn StartFlowDeps,
    cmd_pane_id: &str,
    project_root: &PathBuf,
    project_id: &str,
    tmux_socket_path: Option<&str>,
    namespace_epoch: Option<u64>,
    actions_taken: &mut Vec<String>,
) {
    actions_taken.push(format!("bootstrap_cmd_pane: pane_id={}", cmd_pane_id));
}

/// Cleanup tmux orphans
fn cleanup_tmux_orphans(
    deps: &dyn StartFlowDeps,
    project_id: &str,
    paths: &FlowPaths,
    active_panes: &[String],
    tmux_socket_path: Option<&str>,
    actions_taken: &mut Vec<String>,
) -> Vec<String> {
    actions_taken.push("cleanup_tmux_orphans".to_string());
    vec![]
}

/// Flow paths
#[derive(Debug, Clone)]
pub struct FlowPaths {
    pub ccbd_socket_path: PathBuf,
}

/// Prepared agent
#[derive(Debug, Clone)]
pub struct PreparedAgent {
    pub agent_name: String,
    pub spec: AgentSpec,
    pub plan: AgentPlan,
    pub binding: AgentBinding,
    pub raw_binding: RawBinding,
    pub stale_binding: bool,
    pub window_name: String,
}

/// Tmux layout
#[derive(Debug, Clone)]
pub struct TmuxLayout {
    pub active_panes: Vec<String>,
    pub cmd_pane_id: String,
    pub agent_panes: HashMap<String, String>,
}

/// Window config
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub name: String,
    pub order: u32,
    pub layout_spec: String,
    pub agent_names: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDeps;

    impl StartFlowDeps for TestDeps {
        fn build_project_layout_plan_fn(&self, _config: &dyn StartFlowConfig, requested_agents: Vec<String>) -> LayoutPlan {
            LayoutPlan { target_agent_names: requested_agents }
        }
        fn tmux_namespace_runtime(&self, _socket_path: Option<&str>, _session_name: Option<&str>, _workspace_window: Option<&str>) -> (TmuxBackend, String) {
            (TmuxBackend { socket_path: None }, "root_pane".to_string())
        }
        fn start_agent_runtime_impl(&self, _context: &StartContext, _command: &StartCommand, _runtime_service: &mut dyn RuntimeService, _agent_name: &str, _spec: &AgentSpec, _plan: &AgentPlan, _binding: &AgentBinding, _raw_binding: &RawBinding, _stale_binding: bool, _assigned_pane_id: Option<&str>, _style_index: usize, _project_id: &str, _tmux_socket_path: Option<&str>, _namespace_epoch: Option<u64>, _workspace_window_id: Option<&str>, _workspace_epoch: Option<u64>, _window_name: &str) -> AgentExecution {
            AgentExecution {
                actions_taken: vec![],
                agent_result: AgentResult {
                    agent_name: "test".to_string(),
                    pane_id: Some("test_pane".to_string()),
                    error: None,
                },
                active_panes: vec![],
            }
        }
        fn same_tmux_socket_path_fn(&self, _path1: Option<&str>, _path2: Option<&str>) -> bool {
            true
        }
    }

    struct TestRuntimeService;

    impl RuntimeService for TestRuntimeService {
        fn start_agent(&mut self, _agent_name: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_run_start_flow() {
        let project_root = PathBuf::from("/test");
        let project_id = "test_project";
        let paths = FlowPaths {
            ccbd_socket_path: PathBuf::from("/socket"),
        };

        let config = TestConfig;
        let mut runtime_service = TestRuntimeService;
        let deps = TestDeps;

        let result = run_start_flow(
            &project_root,
            project_id,
            &paths,
            &config,
            &mut runtime_service,
            &["agent1".to_string()],
            false,
            false,
            false,
            false,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            false,
            &deps,
        );

        assert!(result.is_ok());
        let summary = result.unwrap();
        assert_eq!(summary.project_id, "test_project");
    }

    struct TestConfig;

    impl StartFlowConfig for TestConfig {
        fn entry_window(&self) -> &str {
            "main"
        }
        fn topology_signature(&self) -> &str {
            "test_sig"
        }
        fn windows(&self) -> Vec<WindowConfig> {
            vec![]
        }
    }
}
