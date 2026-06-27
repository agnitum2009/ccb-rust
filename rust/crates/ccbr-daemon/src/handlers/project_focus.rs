use crate::app::CcbdApp;
use crate::services::project_namespace::ProjectNamespace;
use ccbr_agents::models::{normalize_agent_name, validate_window_name, ProjectConfig};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
struct FocusPlan {
    kind: &'static str,
    window: String,
    agent: Option<String>,
    namespace_epoch: u64,
    commands: Vec<Vec<String>>,
}

pub fn handle_project_focus_window(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let window = payload.get("window").and_then(|v| v.as_str()).unwrap_or("");
    let namespace_epoch = payload.get("namespace_epoch").and_then(|v| v.as_u64());
    let plan = focus_window_plan(app, window, namespace_epoch)?;
    execute_focus_plan(app, &plan)?;
    Ok(focus_response(&plan))
}

pub fn handle_project_focus_agent(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent = payload.get("agent").and_then(|v| v.as_str()).unwrap_or("");
    let namespace_epoch = payload.get("namespace_epoch").and_then(|v| v.as_u64());
    let plan = focus_agent_plan(app, agent, namespace_epoch)?;
    execute_focus_plan(app, &plan)?;
    Ok(focus_response(&plan))
}

pub fn handle_project_sidebar_click(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let schema_version = payload
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    let view_payload = crate::handlers::project_view::handle_project_view(
        app,
        &json!({"schema_version": schema_version}),
    )?;
    let Some(view) = view_payload.get("view") else {
        return Ok(json!({"focused": false, "target": null}));
    };
    let mouse_y = int_field(payload, "mouse_y");
    let pane_top = int_field(payload, "pane_top");
    let pane_height = int_field(payload, "pane_height");
    let Some((kind, name)) = resolve_sidebar_click_target(view, mouse_y, pane_top, pane_height)
    else {
        return Ok(json!({"focused": false, "target": null}));
    };
    let namespace_epoch = view
        .get("namespace")
        .and_then(|v| v.as_object())
        .and_then(|ns| ns.get("epoch"))
        .and_then(|v| v.as_u64());
    let plan = if kind == "window" {
        focus_window_plan(app, &name, namespace_epoch)?
    } else {
        focus_agent_plan(app, &name, namespace_epoch)?
    };
    execute_focus_plan(app, &plan)?;
    let mut response = focus_response(&plan);
    response["target"] = json!(format!("{kind}:{name}"));
    Ok(response)
}

fn focus_window_plan(
    app: &CcbdApp,
    window: &str,
    requested_epoch: Option<u64>,
) -> Result<FocusPlan, String> {
    let window = validate_window_name(window).map_err(|e| format!("invalid_request: {e}"))?;
    let ns = namespace(app)?;
    validate_epoch(requested_epoch, ns.namespace_epoch)?;
    let agents = window_agents(app.current_config.as_ref(), ns, &window)?;
    let agent = agents.first().cloned();
    let mut commands = vec![vec![
        "select-window".to_string(),
        "-t".to_string(),
        format!("{}:{window}", ns.tmux_session_name),
    ]];
    if let Some(agent_name) = agent.as_deref() {
        if let Some(pane_id) = ns.agent_panes.get(agent_name) {
            commands.push(vec![
                "select-pane".to_string(),
                "-t".to_string(),
                pane_id.clone(),
            ]);
        }
    }
    Ok(FocusPlan {
        kind: "window",
        window,
        agent,
        namespace_epoch: ns.namespace_epoch,
        commands,
    })
}

fn focus_agent_plan(
    app: &CcbdApp,
    agent: &str,
    requested_epoch: Option<u64>,
) -> Result<FocusPlan, String> {
    let agent = normalize_agent_name(agent).map_err(|e| format!("invalid_request: {e}"))?;
    let ns = namespace(app)?;
    validate_epoch(requested_epoch, ns.namespace_epoch)?;
    let window = agent_window(app.current_config.as_ref(), ns, &agent)?;
    let pane_id = ns
        .agent_panes
        .get(&agent)
        .cloned()
        .ok_or_else(|| format!("target_missing: agent pane {agent} is not available"))?;
    Ok(FocusPlan {
        kind: "agent",
        window: window.clone(),
        agent: Some(agent),
        namespace_epoch: ns.namespace_epoch,
        commands: vec![
            vec![
                "select-window".to_string(),
                "-t".to_string(),
                format!("{}:{window}", ns.tmux_session_name),
            ],
            vec!["select-pane".to_string(), "-t".to_string(), pane_id],
        ],
    })
}

fn namespace(app: &CcbdApp) -> Result<&ProjectNamespace, String> {
    app.project_namespace
        .load()
        .ok_or_else(|| "target_missing: project namespace is not available".to_string())
}

fn validate_epoch(requested: Option<u64>, actual: u64) -> Result<(), String> {
    if requested.is_some_and(|epoch| epoch != actual) {
        return Err("stale_view: ProjectView namespace epoch is stale".to_string());
    }
    Ok(())
}

fn window_agents(
    config: Option<&ProjectConfig>,
    ns: &ProjectNamespace,
    window: &str,
) -> Result<Vec<String>, String> {
    if let Some(config) = config {
        if let Some(spec) = config
            .windows
            .as_ref()
            .into_iter()
            .flatten()
            .find(|spec| spec.name == window)
        {
            return Ok(spec.agent_names.clone());
        }
        if config
            .tool_windows
            .as_ref()
            .into_iter()
            .flatten()
            .any(|spec| spec.name == window)
        {
            return Ok(Vec::new());
        }
    }
    ns.windows
        .iter()
        .find(|spec| spec.name == window)
        .map(|spec| spec.agents.clone())
        .ok_or_else(|| format!("unknown_window: unknown window: {window}"))
}

fn agent_window(
    config: Option<&ProjectConfig>,
    ns: &ProjectNamespace,
    agent: &str,
) -> Result<String, String> {
    if let Some(config) = config {
        if !config.agents.contains_key(agent) {
            return Err(format!("unknown_agent: unknown agent: {agent}"));
        }
        if let Some(spec) = config
            .windows
            .as_ref()
            .into_iter()
            .flatten()
            .find(|spec| spec.agent_names.iter().any(|name| name == agent))
        {
            return Ok(spec.name.clone());
        }
        return Err(format!(
            "unknown_agent: agent is not assigned to a window: {agent}"
        ));
    }
    ns.windows
        .iter()
        .find(|spec| spec.agents.iter().any(|name| name == agent))
        .map(|spec| spec.name.clone())
        .ok_or_else(|| format!("unknown_agent: unknown agent: {agent}"))
}

fn execute_focus_plan(app: &CcbdApp, plan: &FocusPlan) -> Result<(), String> {
    let ns = namespace(app)?;
    let backend = ccbr_terminal::TmuxBackend::new(None, Some(ns.tmux_socket_path.clone()));
    for command in &plan.commands {
        let args: Vec<&str> = command.iter().map(String::as_str).collect();
        let output = backend
            .tmux_run(&args, false, true, None, None)
            .map_err(|e| format!("tmux_focus_failed: {e}"))?;
        if !output.success() {
            let stderr = output.stderr.trim();
            let reason = if command.first().map(String::as_str) == Some("select-window") {
                "target_missing"
            } else {
                "tmux_focus_failed"
            };
            return Err(format!(
                "{reason}: failed to {}{}",
                command.join(" "),
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!(": {stderr}")
                }
            ));
        }
    }
    refresh_sidebar_panes(&backend, app.project_id(), &ns.tmux_session_name);
    Ok(())
}

fn refresh_sidebar_panes(
    backend: &ccbr_terminal::TmuxBackend,
    project_id: &str,
    session_name: &str,
) {
    let Ok(output) = backend.tmux_run(
        &[
            "list-panes",
            "-a",
            "-F",
            "#{session_name}\t#{pane_id}\t#{@ccb_project_id}\t#{@ccb_role}\t#{@ccb_managed_by}",
        ],
        false,
        true,
        None,
        None,
    ) else {
        return;
    };
    if !output.success() {
        return;
    }
    for line in output.stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 5 {
            continue;
        }
        if parts[0].trim() == session_name
            && parts[2].trim() == project_id
            && parts[3].trim() == "sidebar"
            && parts[4].trim() == "ccbrd"
            && parts[1].trim().starts_with('%')
        {
            let _ = backend.tmux_run(
                &["send-keys", "-t", parts[1].trim(), "C-l"],
                false,
                true,
                None,
                None,
            );
        }
    }
}

fn focus_response(plan: &FocusPlan) -> Value {
    json!({
        "status": "ok",
        "focused": true,
        "kind": plan.kind,
        "window": plan.window,
        "agent": plan.agent,
        "namespace_epoch": plan.namespace_epoch,
    })
}

fn resolve_sidebar_click_target(
    view: &Value,
    mouse_y: i64,
    pane_top: i64,
    pane_height: i64,
) -> Option<(String, String)> {
    let relative_y = relative_coordinate(mouse_y, pane_top, pane_height);
    if relative_y <= 0 || relative_y >= std::cmp::max(1, pane_height - 1) {
        return None;
    }
    let row_index = (relative_y - 1) as usize;
    sidebar_tree_targets(view).get(row_index).cloned()
}

fn sidebar_tree_targets(view: &Value) -> Vec<(String, String)> {
    let mut targets = Vec::new();
    let windows = view
        .get("windows")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten();
    let agents: Vec<&Value> = view
        .get("agents")
        .and_then(|v| v.as_array())
        .map(|items| items.iter().collect())
        .unwrap_or_default();
    for window in windows {
        let window_name = window
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if window_name.is_empty() {
            continue;
        }
        targets.push(("window".to_string(), window_name.to_string()));
        for agent in &agents {
            if agent
                .get("window")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                != window_name
            {
                continue;
            }
            let agent_name = agent
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if !agent_name.is_empty() {
                targets.push(("agent".to_string(), agent_name.to_string()));
            }
        }
    }
    targets
}

fn relative_coordinate(value: i64, pane_start: i64, pane_size: i64) -> i64 {
    if value >= pane_size && value >= pane_start {
        value - pane_start
    } else {
        value
    }
}

fn int_field(payload: &Value, key: &str) -> i64 {
    payload
        .get(key)
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::project_namespace::{NamespaceWindow, ProjectNamespace};
    use crate::start_flow::service::StartFlowService;
    use crate::stop_flow::service::StopFlowService;
    use tempfile::TempDir;

    fn app_with_namespace(dir: &TempDir) -> CcbdApp {
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        let config = ccbr_agents::config::build_default_project_config();
        let agent_names = config.default_agents.clone();
        app.current_config = Some(config);
        app.project_namespace
            .mount(ProjectNamespace {
                project_root: dir.path().display().to_string(),
                project_id: app.project_id().to_string(),
                tmux_socket_path: app.tmux_socket_path(),
                tmux_socket_name: "tmux".into(),
                tmux_session_name: "ccbr-test".into(),
                agent_names: agent_names.clone(),
                windows: vec![
                    NamespaceWindow {
                        name: "main".into(),
                        window_id: Some("@1".into()),
                        agents: agent_names.clone(),
                    },
                    NamespaceWindow {
                        name: "neovim".into(),
                        window_id: Some("@2".into()),
                        agents: vec![],
                    },
                ],
                agent_panes: agent_names
                    .iter()
                    .enumerate()
                    .map(|(index, name)| (name.clone(), format!("%{}", index + 1)))
                    .collect(),
                active_panes: vec!["%1".into()],
                namespace_epoch: 4,
                created_at: chrono::Utc::now().to_rfc3339(),
            })
            .unwrap();
        app
    }

    #[test]
    fn focus_agent_plans_window_and_pane_selection() {
        let dir = TempDir::new().unwrap();
        let app = app_with_namespace(&dir);

        let plan = focus_agent_plan(&app, "agent2", Some(4)).unwrap();

        assert_eq!(plan.kind, "agent");
        assert_eq!(plan.window, "main");
        assert_eq!(plan.agent.as_deref(), Some("agent2"));
        assert_eq!(plan.namespace_epoch, 4);
        assert_eq!(
            plan.commands,
            vec![
                vec!["select-window", "-t", "ccbr-test:main"],
                vec!["select-pane", "-t", "%2"],
            ]
        );
        assert_eq!(focus_response(&plan)["focused"].as_bool(), Some(true));
    }

    #[test]
    fn focus_tool_window_does_not_select_agent_pane() {
        let dir = TempDir::new().unwrap();
        let app = app_with_namespace(&dir);

        let plan = focus_window_plan(&app, "neovim", Some(4)).unwrap();

        assert_eq!(plan.kind, "window");
        assert_eq!(plan.window, "neovim");
        assert!(plan.agent.is_none());
        assert_eq!(
            plan.commands,
            vec![vec!["select-window", "-t", "ccbr-test:neovim"]]
        );
    }

    #[test]
    fn focus_rejects_stale_namespace_epoch() {
        let dir = TempDir::new().unwrap();
        let app = app_with_namespace(&dir);

        let err = focus_agent_plan(&app, "agent1", Some(3)).unwrap_err();

        assert!(err.contains("stale_view"));
    }

    #[test]
    fn sidebar_click_resolves_window_and_agent_rows() {
        let view = json!({
            "windows": [{"name": "main"}, {"name": "tools"}],
            "agents": [
                {"name": "agent1", "window": "main"},
                {"name": "agent2", "window": "main"}
            ],
        });

        assert_eq!(
            resolve_sidebar_click_target(&view, 1, 0, 10),
            Some(("window".to_string(), "main".to_string()))
        );
        assert_eq!(
            resolve_sidebar_click_target(&view, 2, 0, 10),
            Some(("agent".to_string(), "agent1".to_string()))
        );
        assert_eq!(resolve_sidebar_click_target(&view, 0, 0, 10), None);
    }
}
