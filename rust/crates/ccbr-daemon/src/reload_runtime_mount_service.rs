//! Mirrors Python `lib/ccbd/reload_runtime_mount_service.py`.

use crate::app::CcbdApp;
use crate::reload_apply_models::ServiceGraph;
use crate::reload_runtime_mount_models::AdditiveRuntimeMountResult;
use crate::reload_runtime_mount_start::call_start_flow_for_additive_mount;
use crate::reload_runtime_mount_state::{
    agent_names, agent_panes_from_record, changed_agents, runtime_guard_agents, runtime_snapshots,
    summary_record, summary_started, RuntimeRegistry,
};
use crate::reload_runtime_mount_validation::{
    blocked_mount_reason, existing_runtime_agents, AgentConfig as ValidationAgentConfig,
    AgentRecord, GraphConfig as ValidationGraphConfig, NamespaceState,
    RuntimeSupervisor as ValidationRuntimeSupervisor, ServiceGraph as ValidationServiceGraph,
};
use crate::services::project_namespace::ProjectNamespace;
use crate::services::project_namespace_runtime::models::NamespacePatchApplyResult;
use crate::services::registry::AgentRegistry;
use crate::start_flow::service::StartFlowResult;
use std::collections::HashMap;

/// Custom start-flow implementation injected for additive mounts.
type RunStartFlowFn<'a> = &'a dyn Fn(
    &std::path::Path,
    &str,
    &str,
    &str,
    &[String],
    bool,
    bool,
    Option<&HashMap<String, String>>,
    Option<Vec<ccbr_agents::models::WindowSpec>>,
) -> Result<StartFlowResult, String>;

/// Prepared additive mount context: pane map, requested agents, preserved agents.
type MountContext = (HashMap<String, String>, Vec<String>, Vec<String>);

/// Registry adapter used during additive runtime mounts.
pub struct AdditiveMountRegistry<'a> {
    pub inner: &'a AgentRegistry,
}

impl RuntimeRegistry for AdditiveMountRegistry<'_> {
    fn get(&self, agent_name: &str) -> Option<AgentRecord> {
        self.inner.get(agent_name).map(registry_entry_to_record)
    }

    fn list_all(&self) -> Vec<AgentRecord> {
        self.inner
            .all_entries()
            .iter()
            .map(|e| registry_entry_to_record(e))
            .collect()
    }

    fn list_names(&self) -> Vec<String> {
        self.inner
            .all_entries()
            .iter()
            .map(|e| e.agent_name.clone())
            .collect()
    }
}

fn records_from_snapshots(
    snapshots: &HashMap<String, Option<serde_json::Value>>,
) -> HashMap<String, Option<AgentRecord>> {
    snapshots
        .iter()
        .map(|(agent, record)| {
            let rec = record.as_ref().map(|v| AgentRecord {
                state: v
                    .get("state")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
                health: v
                    .get("health")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
                desired_state: v
                    .get("desired_state")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
                reconcile_state: v
                    .get("reconcile_state")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
                fields: HashMap::new(),
            });
            (agent.clone(), rec)
        })
        .collect()
}

fn registry_entry_to_record(entry: &crate::services::registry::AgentRuntimeEntry) -> AgentRecord {
    AgentRecord {
        state: Some(entry.state.clone()),
        health: Some(entry.health.clone()),
        desired_state: Some(entry.state.clone()),
        reconcile_state: Some(entry.state.clone()),
        fields: std::collections::HashMap::new(),
    }
}

/// Run additive agent mounts during a reload.
pub fn run_additive_agent_mounts(
    app: &mut CcbdApp,
    graph: &ServiceGraph,
    namespace: &ProjectNamespace,
    patch_result: &NamespacePatchApplyResult,
    run_start_flow_fn: Option<RunStartFlowFn<'_>>,
) -> AdditiveRuntimeMountResult {
    let prepared = match prepare_mount_context(graph, namespace, patch_result) {
        Ok(p) => p,
        Err(result) => return result,
    };

    let (agent_panes, requested_agents, preserved_agents) = prepared;

    if requested_agents.is_empty() {
        return noop_mount_result(&preserved_agents);
    }

    let registry = AdditiveMountRegistry {
        inner: &app.registry,
    };
    let before_new = runtime_snapshots(&registry, &requested_agents);
    let before_new_records = records_from_snapshots(&before_new);
    let existing = existing_runtime_agents(&before_new_records, &requested_agents);
    if !existing.is_empty() {
        return blocked_mount_result(
            "runtime_authority_already_exists",
            &format!(
                "runtime mounts can only target agents without existing runtime authority: {}",
                existing.join(",")
            ),
            Some(&requested_agents),
            None,
            None,
            Some(&preserved_agents),
        );
    }

    let guarded_agents = runtime_guard_agents(&registry, &requested_agents, &preserved_agents);
    let before_preserved = runtime_snapshots(&registry, &guarded_agents);

    match run_start_flow_and_validate(
        app,
        namespace,
        &agent_panes,
        &requested_agents,
        &guarded_agents,
        &before_preserved,
        &before_new,
        run_start_flow_fn,
    ) {
        Ok(result) => result,
        Err((reason, error)) => failed_mount_result(
            &reason,
            error.as_ref(),
            &requested_agents,
            &guarded_agents,
            &before_preserved,
            &before_new,
            None,
        ),
    }
}

fn prepare_mount_context(
    graph: &ServiceGraph,
    namespace: &ProjectNamespace,
    patch_result: &NamespacePatchApplyResult,
) -> Result<MountContext, AdditiveRuntimeMountResult> {
    if patch_result.status != "applied" {
        return Err(blocked_mount_result(
            "namespace_patch_not_applied",
            "runtime mounts require an applied namespace patch",
            None,
            None,
            None,
            None,
        ));
    }

    let agent_panes = patch_agent_panes(patch_result);
    let preserved_agents = patch_preserved_agents(patch_result);
    let requested_agents: Vec<String> = agent_panes.keys().cloned().collect();

    if requested_agents.is_empty() {
        return Ok((agent_panes, requested_agents, preserved_agents));
    }

    let validation_graph = service_graph_to_validation(graph);
    let validation_namespace = project_namespace_to_validation(namespace);
    if let Some((reason, message)) = blocked_mount_reason(
        &validation_graph,
        Some(&validation_namespace),
        &agent_panes,
        &preserved_agents,
    ) {
        return Err(blocked_mount_result(
            &reason,
            &message,
            Some(&requested_agents),
            None,
            None,
            Some(&preserved_agents),
        ));
    }

    Ok((agent_panes, requested_agents, preserved_agents))
}

fn patch_agent_panes(patch: &NamespacePatchApplyResult) -> HashMap<String, String> {
    patch
        .diagnostics
        .get("agent_panes")
        .and_then(|v| {
            if let serde_json::Value::Object(obj) = v {
                Some(agent_panes_from_record(obj))
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn patch_preserved_agents(patch: &NamespacePatchApplyResult) -> Vec<String> {
    patch
        .diagnostics
        .get("preserved_before")
        .as_ref()
        .map(|v| agent_names(v))
        .unwrap_or_default()
}

fn service_graph_to_validation(graph: &ServiceGraph) -> ValidationServiceGraph {
    ValidationServiceGraph {
        config: ValidationGraphConfig {
            agents: graph
                .config
                .agents
                .keys()
                .map(|k| (k.clone(), ValidationAgentConfig {}))
                .collect(),
        },
        runtime_supervisor: Some(ValidationRuntimeSupervisor {
            project_id: graph
                .config_identity
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        }),
    }
}

fn project_namespace_to_validation(namespace: &ProjectNamespace) -> NamespaceState {
    NamespaceState {
        project_id: Some(namespace.project_id.clone()),
        ui_attachable: true,
        tmux_socket_path: Some(namespace.tmux_socket_path.clone()),
        tmux_session_name: Some(namespace.tmux_session_name.clone()),
        namespace_epoch: Some(namespace.namespace_epoch as i64),
    }
}

/// Arity mirrors the Python `reload_runtime_mount_service` start-flow helper.
#[allow(clippy::too_many_arguments)]
fn run_start_flow_and_validate(
    app: &mut CcbdApp,
    namespace: &ProjectNamespace,
    agent_panes: &HashMap<String, String>,
    requested_agents: &[String],
    preserved_agents: &[String],
    before_preserved: &HashMap<String, Option<serde_json::Value>>,
    before_new: &HashMap<String, Option<serde_json::Value>>,
    run_start_flow_fn: Option<RunStartFlowFn<'_>>,
) -> Result<AdditiveRuntimeMountResult, (String, Box<dyn std::error::Error>)> {
    let (restore, auto_permission) = crate::reload_runtime_mount_start::start_options(app, None);

    let run_fn = match run_start_flow_fn {
        Some(f) => f,
        None => {
            return Err((
                "runtime_mount_failed".to_string(),
                Box::new(std::io::Error::other("no start flow function provided"))
                    as Box<dyn std::error::Error>,
            ));
        }
    };

    let summary = call_start_flow_for_additive_mount(
        app,
        namespace,
        agent_panes,
        requested_agents,
        restore,
        auto_permission,
        run_fn,
    )
    .map_err(|e| {
        (
            "runtime_mount_failed".to_string(),
            Box::new(std::io::Error::other(e)) as Box<dyn std::error::Error>,
        )
    })?;

    validate_mount_result(
        app,
        requested_agents,
        preserved_agents,
        before_preserved,
        before_new,
        &summary,
    )
    .map_err(|e| {
        (
            "preserved_runtime_authority_changed".to_string(),
            Box::new(e) as Box<dyn std::error::Error>,
        )
    })
}

fn validate_mount_result(
    app: &CcbdApp,
    requested_agents: &[String],
    preserved_agents: &[String],
    before_preserved: &HashMap<String, Option<serde_json::Value>>,
    before_new: &HashMap<String, Option<serde_json::Value>>,
    summary: &StartFlowResult,
) -> Result<AdditiveRuntimeMountResult, std::io::Error> {
    let registry = AdditiveMountRegistry {
        inner: &app.registry,
    };
    let after_preserved = runtime_snapshots(&registry, preserved_agents);
    let after_new = runtime_snapshots(&registry, requested_agents);

    let preserved_changed = changed_agents(before_preserved, &after_preserved);
    if !preserved_changed.is_empty() {
        return Err(std::io::Error::other(format!(
            "preserved runtime authority changed: {}",
            preserved_changed.join(",")
        )));
    }

    let missing: Vec<String> = requested_agents
        .iter()
        .filter(|agent| after_new.get(*agent).unwrap_or(&None).is_none())
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(std::io::Error::other(format!(
            "runtime authority missing after mount: {}",
            missing.join(",")
        )));
    }

    let summary_value = serde_json::to_value(summary).unwrap_or_default();
    let mounted_agents = summary_started(Some(&summary_value), requested_agents);
    let written_agents = changed_agents(before_new, &after_new);

    Ok(mounted_result(
        requested_agents,
        &mounted_agents,
        &written_agents,
        preserved_agents,
        Some(&summary_value),
    ))
}

/// Build a blocked mount result.
pub fn blocked_mount_result(
    reason: &str,
    message: &str,
    requested_agents: Option<&[String]>,
    mounted_agents: Option<&[String]>,
    written_agents: Option<&[String]>,
    preserved_agents: Option<&[String]>,
) -> AdditiveRuntimeMountResult {
    let mut diagnostics = HashMap::new();
    diagnostics.insert("reason".to_string(), serde_json::json!(reason));
    diagnostics.insert("message".to_string(), serde_json::json!(message));
    if let Some(agents) = requested_agents {
        diagnostics.insert("requested_agents".to_string(), serde_json::json!(agents));
    }
    if let Some(agents) = mounted_agents {
        diagnostics.insert("mounted_agents".to_string(), serde_json::json!(agents));
    }
    if let Some(agents) = written_agents {
        diagnostics.insert("written_agents".to_string(), serde_json::json!(agents));
    }
    if let Some(agents) = preserved_agents {
        diagnostics.insert("preserved_agents".to_string(), serde_json::json!(agents));
    }
    AdditiveRuntimeMountResult {
        status: "blocked".to_string(),
        stage: "mount".to_string(),
        diagnostics: Some(diagnostics),
    }
}

/// Build a failed mount result.
pub fn failed_mount_result(
    reason: &str,
    error: &dyn std::error::Error,
    requested_agents: &[String],
    preserved_agents: &[String],
    before_preserved: &HashMap<String, Option<serde_json::Value>>,
    before_new: &HashMap<String, Option<serde_json::Value>>,
    summary: Option<&serde_json::Value>,
) -> AdditiveRuntimeMountResult {
    let after_preserved = before_preserved.clone();
    let after_new = before_new.clone();
    let preserved_changed = changed_agents(before_preserved, &after_preserved);
    let written_agents = changed_agents(before_new, &after_new);
    let mounted_agents = summary_started(summary, &[]);
    let preserved_unchanged: Vec<String> = preserved_agents
        .iter()
        .filter(|agent| !preserved_changed.contains(agent))
        .cloned()
        .collect();

    let mut diagnostics = HashMap::new();
    diagnostics.insert("reason".to_string(), serde_json::json!(reason));
    diagnostics.insert(
        "error_type".to_string(),
        serde_json::json!(std::any::type_name_of_val(error)
            .split("::")
            .last()
            .unwrap_or("Error")),
    );
    diagnostics.insert("error".to_string(), serde_json::json!(error.to_string()));
    diagnostics.insert(
        "requested_agents".to_string(),
        serde_json::json!(requested_agents),
    );
    diagnostics.insert(
        "mounted_agents".to_string(),
        serde_json::json!(mounted_agents),
    );
    diagnostics.insert(
        "written_agents".to_string(),
        serde_json::json!(written_agents),
    );
    diagnostics.insert(
        "preserved_unchanged_agents".to_string(),
        serde_json::json!(preserved_unchanged),
    );
    diagnostics.insert(
        "preserved_changed_agents".to_string(),
        serde_json::json!(preserved_changed),
    );
    if let Some(summary_record) = summary_record(summary) {
        diagnostics.insert("summary".to_string(), summary_record);
    }

    AdditiveRuntimeMountResult {
        status: "failed".to_string(),
        stage: "mount".to_string(),
        diagnostics: Some(diagnostics),
    }
}

/// Build a mounted result.
pub fn mounted_result(
    requested_agents: &[String],
    mounted_agents: &[String],
    written_agents: &[String],
    preserved_agents: &[String],
    summary: Option<&serde_json::Value>,
) -> AdditiveRuntimeMountResult {
    let mut diagnostics = HashMap::new();
    diagnostics.insert(
        "requested_agents".to_string(),
        serde_json::json!(requested_agents),
    );
    diagnostics.insert(
        "mounted_agents".to_string(),
        serde_json::json!(mounted_agents),
    );
    diagnostics.insert(
        "written_agents".to_string(),
        serde_json::json!(written_agents),
    );
    diagnostics.insert(
        "preserved_agents".to_string(),
        serde_json::json!(preserved_agents),
    );
    diagnostics.insert(
        "unload_or_replace_executed".to_string(),
        serde_json::json!(false),
    );
    if let Some(summary_record) = summary_record(summary) {
        diagnostics.insert("summary".to_string(), summary_record);
    }
    AdditiveRuntimeMountResult {
        status: "mounted".to_string(),
        stage: "mount".to_string(),
        diagnostics: Some(diagnostics),
    }
}

/// Build a noop mount result.
pub fn noop_mount_result(preserved_agents: &[String]) -> AdditiveRuntimeMountResult {
    let mut diagnostics = HashMap::new();
    diagnostics.insert(
        "reason".to_string(),
        serde_json::json!("no_requested_agents"),
    );
    diagnostics.insert(
        "preserved_agents".to_string(),
        serde_json::json!(preserved_agents),
    );
    AdditiveRuntimeMountResult {
        status: "noop".to_string(),
        stage: "mount".to_string(),
        diagnostics: Some(diagnostics),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::registry::AgentRuntimeEntry;

    fn sample_namespace() -> ProjectNamespace {
        ProjectNamespace {
            project_root: "/tmp".to_string(),
            project_id: "p1".to_string(),
            tmux_socket_path: "/tmp/tmux.sock".to_string(),
            tmux_socket_name: "tmux".to_string(),
            tmux_session_name: "session".to_string(),
            agent_names: vec!["claude".to_string()],
            windows: vec![crate::services::project_namespace::NamespaceWindow {
                name: "main".to_string(),
                window_id: None,
                agents: vec!["claude".to_string()],
            }],
            agent_panes: {
                let mut m = HashMap::new();
                m.insert("claude".to_string(), "%1".to_string());
                m
            },
            active_panes: vec!["%1".to_string()],
            namespace_epoch: 1,
            created_at: "now".to_string(),
        }
    }

    fn applied_patch(agent_panes: HashMap<String, String>) -> NamespacePatchApplyResult {
        NamespacePatchApplyResult {
            status: "applied".to_string(),
            diagnostics: serde_json::json!({
                "agent_panes": agent_panes,
                "preserved_before": [],
            }),
        }
    }

    fn sample_graph() -> ServiceGraph {
        let mut config = ccbr_agents::models::ProjectConfig::default();
        config.agents.insert(
            "claude".to_string(),
            ccbr_agents::models::AgentSpec::default_with_name("claude"),
        );
        ServiceGraph {
            version: Some("v2".to_string()),
            config,
            config_identity: serde_json::json!({"project_id": "p1"}),
            config_signature: "sig".to_string(),
        }
    }

    #[test]
    fn test_run_additive_agent_mounts_noop_when_empty() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            crate::start_flow::service::StartFlowService::with_stub(),
            crate::stop_flow::service::StopFlowService::with_stub(),
        );
        let mut namespace = sample_namespace();
        namespace.agent_panes.clear();
        let patch = applied_patch(HashMap::new());
        let result = run_additive_agent_mounts(&mut app, &sample_graph(), &namespace, &patch, None);
        assert_eq!(result.status, "noop");
    }

    #[test]
    fn test_run_additive_agent_mounts_blocked_when_not_applied() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            crate::start_flow::service::StartFlowService::with_stub(),
            crate::stop_flow::service::StopFlowService::with_stub(),
        );
        let namespace = sample_namespace();
        let patch = NamespacePatchApplyResult {
            status: "blocked".to_string(),
            diagnostics: serde_json::json!({}),
        };
        let result = run_additive_agent_mounts(&mut app, &sample_graph(), &namespace, &patch, None);
        assert_eq!(result.status, "blocked");
    }

    #[test]
    fn test_run_additive_agent_mounts_with_start_flow() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            crate::start_flow::service::StartFlowService::with_stub(),
            crate::stop_flow::service::StopFlowService::with_stub(),
        );
        app.registry.register(AgentRuntimeEntry {
            agent_name: "claude".to_string(),
            provider: "claude".to_string(),
            state: "stopped".to_string(),
            health: "stopped".to_string(),
            pane_id: None,
            workspace_path: None,
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });
        let namespace = sample_namespace();
        let patch = applied_patch(namespace.agent_panes.clone());
        let run_fn = |_root: &std::path::Path,
                      _project_id: &str,
                      _socket: &str,
                      _session: &str,
                      agents: &[String],
                      _restore: bool,
                      _auto: bool,
                      _panes: Option<&HashMap<String, String>>,
                      _windows: Option<Vec<ccbr_agents::models::WindowSpec>>| {
            Ok(StartFlowResult {
                status: "ok".to_string(),
                agent_results: agents
                    .iter()
                    .map(|a| crate::start_flow::service::StartAgentResult {
                        agent_name: a.clone(),
                        status: "started".to_string(),
                        reason: None,
                        pane_id: Some("%1".to_string()),
                    })
                    .collect(),
                actions_taken: vec![],
            })
        };
        let result =
            run_additive_agent_mounts(&mut app, &sample_graph(), &namespace, &patch, Some(&run_fn));
        assert_eq!(result.status, "mounted");
    }

    #[test]
    fn test_blocked_mount_result_includes_requested_agents() {
        let result = blocked_mount_result(
            "test_reason",
            "test message",
            Some(&["claude".to_string()]),
            None,
            None,
            None,
        );
        assert_eq!(result.status, "blocked");
        assert_eq!(
            result.diagnostics.as_ref().unwrap()["reason"],
            "test_reason"
        );
        assert_eq!(
            result.diagnostics.as_ref().unwrap()["requested_agents"],
            serde_json::json!(["claude"])
        );
    }

    #[test]
    fn test_mounted_result() {
        let result = mounted_result(
            &["claude".to_string()],
            &["claude".to_string()],
            &[],
            &[],
            None,
        );
        assert_eq!(result.status, "mounted");
        assert_eq!(
            result.diagnostics.as_ref().unwrap()["requested_agents"],
            serde_json::json!(["claude"])
        );
    }

    #[test]
    fn test_noop_mount_result() {
        let result = noop_mount_result(&["gemini".to_string()]);
        assert_eq!(result.status, "noop");
        assert_eq!(
            result.diagnostics.as_ref().unwrap()["preserved_agents"],
            serde_json::json!(["gemini"])
        );
    }
}
