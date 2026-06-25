use crate::app::CcbdApp;
use ccbr_agents::models::normalize_agent_name;
use serde_json::{json, Value};
use std::time::Duration;

const OPENCODE_CLEAR_SUBMIT_DELAY: Duration = Duration::from_millis(300);

trait ClearContextBackend {
    fn pane_exists(&mut self, pane_id: &str) -> bool;
    fn ensure_not_in_copy_mode(&mut self, pane_id: &str);
    fn tmux_run_checked(&mut self, args: &[&str]) -> Result<(), String>;
}

impl ClearContextBackend for ccbr_terminal::TmuxBackend {
    fn pane_exists(&mut self, pane_id: &str) -> bool {
        ccbr_terminal::TmuxBackend::pane_exists(self, pane_id)
    }

    fn ensure_not_in_copy_mode(&mut self, pane_id: &str) {
        ccbr_terminal::TmuxBackend::ensure_not_in_copy_mode(self, pane_id);
    }

    fn tmux_run_checked(&mut self, args: &[&str]) -> Result<(), String> {
        let output = self
            .tmux_run(args, true, true, None, None)
            .map_err(|e| e.to_string())?;
        if output.success() {
            Ok(())
        } else {
            Err(output.stderr.trim().to_string())
        }
    }
}

pub fn handle_project_clear(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_names = requested_agent_names(app, payload)?;
    let namespace = app
        .project_namespace
        .load()
        .ok_or_else(|| "project namespace is not mounted".to_string())?;
    let mut backend =
        ccbr_terminal::TmuxBackend::new(None, Some(namespace.tmux_socket_path.clone()));
    let results: Vec<Value> = agent_names
        .iter()
        .map(|name| {
            clear_agent_context(app, &mut backend, name, |duration| {
                std::thread::sleep(duration)
            })
        })
        .collect();
    Ok(json!({
        "status": "ok",
        "agent_names": agent_names,
        "results": results,
    }))
}

fn requested_agent_names(app: &CcbdApp, payload: &Value) -> Result<Vec<String>, String> {
    let raw_names: Vec<String> = payload
        .get("agent_names")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    if raw_names.is_empty() {
        return Ok(config_agent_names(app));
    }
    if raw_names
        .iter()
        .any(|name| name.eq_ignore_ascii_case("all"))
    {
        if raw_names.len() > 1 {
            return Err("clear target \"all\" cannot be combined with agent names".to_string());
        }
        return Ok(config_agent_names(app));
    }
    let known = config_agent_names(app);
    let known_set: std::collections::HashSet<&str> = known.iter().map(String::as_str).collect();
    let mut names = Vec::new();
    for raw in raw_names {
        let name = normalize_agent_name(&raw).map_err(|e| format!("invalid_request: {e}"))?;
        if !known_set.contains(name.as_str()) {
            return Err(format!("unknown agent: {name}"));
        }
        if !names.iter().any(|item| item == &name) {
            names.push(name);
        }
    }
    Ok(names)
}

fn config_agent_names(app: &CcbdApp) -> Vec<String> {
    if let Some(config) = app.current_config.as_ref() {
        let mut names = Vec::new();
        for name in &config.default_agents {
            if config.agents.contains_key(name) && !names.iter().any(|item| item == name) {
                names.push(name.clone());
            }
        }
        let mut extras: Vec<String> = config
            .agents
            .keys()
            .filter(|name| !names.iter().any(|item| item == *name))
            .cloned()
            .collect();
        extras.sort();
        names.extend(extras);
        if !names.is_empty() {
            return names;
        }
    }
    if let Some(namespace) = app.project_namespace.load() {
        if !namespace.agent_names.is_empty() {
            return namespace.agent_names.clone();
        }
    }
    let mut names: Vec<String> = app
        .registry
        .all_entries()
        .into_iter()
        .map(|entry| entry.agent_name.clone())
        .collect();
    names.sort();
    names
}

fn clear_agent_context<B, F>(
    app: &CcbdApp,
    backend: &mut B,
    agent_name: &str,
    mut sleep_fn: F,
) -> Value
where
    B: ClearContextBackend,
    F: FnMut(Duration),
{
    let Some(pane_id) = agent_pane_id(app, agent_name) else {
        let reason = if app.registry.get(agent_name).is_some() {
            "pane_missing"
        } else {
            "runtime_missing"
        };
        return json!({"agent": agent_name, "status": "skipped", "reason": reason});
    };
    if !backend.pane_exists(&pane_id) {
        return json!({
            "agent": agent_name,
            "status": "skipped",
            "reason": "pane_missing",
            "pane_id": pane_id,
        });
    }
    let provider = agent_provider(app, agent_name);
    match send_clear_sequence(backend, &pane_id, &provider, &mut sleep_fn) {
        Ok(()) => json!({
            "agent": agent_name,
            "status": "cleared",
            "pane_id": pane_id,
            "command": "/clear",
        }),
        Err(reason) => json!({
            "agent": agent_name,
            "status": "failed",
            "reason": truncate_reason(&reason),
            "pane_id": pane_id,
        }),
    }
}

fn agent_pane_id(app: &CcbdApp, agent_name: &str) -> Option<String> {
    app.project_namespace
        .load()
        .and_then(|ns| ns.agent_panes.get(agent_name).cloned())
        .or_else(|| {
            app.registry
                .get(agent_name)
                .and_then(|entry| entry.pane_id.clone())
        })
        .filter(|pane_id| pane_id.trim().starts_with('%'))
}

fn agent_provider(app: &CcbdApp, agent_name: &str) -> String {
    app.current_config
        .as_ref()
        .and_then(|config| config.agents.get(agent_name))
        .map(|spec| spec.provider.trim().to_lowercase())
        .filter(|provider| !provider.is_empty())
        .or_else(|| {
            app.registry
                .get(agent_name)
                .map(|entry| entry.provider.trim().to_lowercase())
        })
        .unwrap_or_default()
}

fn send_clear_sequence<B, F>(
    backend: &mut B,
    pane_id: &str,
    provider: &str,
    sleep_fn: &mut F,
) -> Result<(), String>
where
    B: ClearContextBackend,
    F: FnMut(Duration),
{
    backend.ensure_not_in_copy_mode(pane_id);
    backend.tmux_run_checked(&["send-keys", "-t", pane_id, "C-u"])?;
    backend.tmux_run_checked(&["send-keys", "-t", pane_id, "-l", "/clear"])?;
    if provider == "opencode" {
        sleep_fn(OPENCODE_CLEAR_SUBMIT_DELAY);
    }
    backend.tmux_run_checked(&["send-keys", "-t", pane_id, "Enter"])?;
    Ok(())
}

fn truncate_reason(reason: &str) -> String {
    reason.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::project_namespace::{NamespaceWindow, ProjectNamespace};
    use crate::services::registry::AgentRuntimeEntry;
    use crate::start_flow::service::StartFlowService;
    use crate::stop_flow::service::StopFlowService;
    use std::collections::{HashMap, HashSet};
    use tempfile::TempDir;

    #[derive(Default)]
    struct FakeClearBackend {
        existing_panes: HashSet<String>,
        calls: Vec<Vec<String>>,
    }

    impl ClearContextBackend for FakeClearBackend {
        fn pane_exists(&mut self, pane_id: &str) -> bool {
            self.existing_panes.contains(pane_id)
        }

        fn ensure_not_in_copy_mode(&mut self, pane_id: &str) {
            self.calls
                .push(vec!["copy-mode-quit".to_string(), pane_id.to_string()]);
        }

        fn tmux_run_checked(&mut self, args: &[&str]) -> Result<(), String> {
            self.calls
                .push(args.iter().map(|item| item.to_string()).collect());
            Ok(())
        }
    }

    fn app_with_agents(dir: &TempDir) -> CcbdApp {
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        let mut config = ccbr_agents::config::build_default_project_config();
        config.default_agents = vec!["agent1".into(), "agent2".into()];
        config
            .agents
            .retain(|name, _| name == "agent1" || name == "agent2");
        if let Some(spec) = config.agents.get_mut("agent2") {
            spec.provider = "opencode".into();
        }
        app.current_config = Some(config);
        app.registry.register(AgentRuntimeEntry {
            agent_name: "agent1".into(),
            provider: "codex".into(),
            state: "idle".into(),
            health: "healthy".into(),
            pane_id: Some("%1".into()),
            workspace_path: None,
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });
        app.registry.register(AgentRuntimeEntry {
            agent_name: "agent2".into(),
            provider: "opencode".into(),
            state: "idle".into(),
            health: "healthy".into(),
            pane_id: Some("%2".into()),
            workspace_path: None,
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });
        app.project_namespace
            .mount(ProjectNamespace {
                project_root: dir.path().display().to_string(),
                project_id: app.project_id().to_string(),
                tmux_socket_path: app.tmux_socket_path(),
                tmux_socket_name: "tmux".into(),
                tmux_session_name: "ccbr-test".into(),
                agent_names: vec!["agent1".into(), "agent2".into()],
                windows: vec![NamespaceWindow {
                    name: "main".into(),
                    window_id: Some("@1".into()),
                    agents: vec!["agent1".into(), "agent2".into()],
                }],
                agent_panes: HashMap::from([
                    ("agent1".to_string(), "%1".to_string()),
                    ("agent2".to_string(), "%2".to_string()),
                ]),
                active_panes: vec!["%1".into(), "%2".into()],
                namespace_epoch: 4,
                created_at: chrono::Utc::now().to_rfc3339(),
            })
            .unwrap();
        app
    }

    #[test]
    fn project_clear_context_targets_all_agent_panes_with_provider_clear() {
        let dir = TempDir::new().unwrap();
        let app = app_with_agents(&dir);
        let mut backend = FakeClearBackend {
            existing_panes: HashSet::from(["%1".to_string(), "%2".to_string()]),
            calls: Vec::new(),
        };
        let mut sleeps = Vec::new();

        let results: Vec<Value> = requested_agent_names(&app, &json!({}))
            .unwrap()
            .iter()
            .map(|agent| {
                clear_agent_context(&app, &mut backend, agent, |duration| sleeps.push(duration))
            })
            .collect();

        assert_eq!(
            results,
            vec![
                json!({"agent": "agent1", "status": "cleared", "pane_id": "%1", "command": "/clear"}),
                json!({"agent": "agent2", "status": "cleared", "pane_id": "%2", "command": "/clear"}),
            ]
        );
        assert_eq!(
            backend.calls,
            vec![
                vec!["copy-mode-quit", "%1"],
                vec!["send-keys", "-t", "%1", "C-u"],
                vec!["send-keys", "-t", "%1", "-l", "/clear"],
                vec!["send-keys", "-t", "%1", "Enter"],
                vec!["copy-mode-quit", "%2"],
                vec!["send-keys", "-t", "%2", "C-u"],
                vec!["send-keys", "-t", "%2", "-l", "/clear"],
                vec!["send-keys", "-t", "%2", "Enter"],
            ]
        );
        assert_eq!(sleeps, vec![OPENCODE_CLEAR_SUBMIT_DELAY]);
    }

    #[test]
    fn project_clear_context_dedupes_requested_agents_and_rejects_unknown() {
        let dir = TempDir::new().unwrap();
        let app = app_with_agents(&dir);

        assert_eq!(
            requested_agent_names(&app, &json!({"agent_names": ["agent2", "agent2"]})).unwrap(),
            vec!["agent2"]
        );
        assert!(
            requested_agent_names(&app, &json!({"agent_names": ["all", "agent1"]}))
                .unwrap_err()
                .contains("cannot be combined")
        );
        assert_eq!(
            requested_agent_names(&app, &json!({"agent_names": ["missing"]})).unwrap_err(),
            "unknown agent: missing"
        );
    }

    #[test]
    fn project_clear_context_reports_missing_panes() {
        let dir = TempDir::new().unwrap();
        let app = app_with_agents(&dir);
        let mut backend = FakeClearBackend {
            existing_panes: HashSet::from(["%1".to_string()]),
            calls: Vec::new(),
        };

        let result = clear_agent_context(&app, &mut backend, "agent2", |_| {});

        assert_eq!(
            result,
            json!({"agent": "agent2", "status": "skipped", "reason": "pane_missing", "pane_id": "%2"})
        );
        assert!(backend.calls.is_empty());
    }
}
