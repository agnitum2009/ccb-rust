//! Mirrors Python `lib/ccbd/reload_runtime_mount_start.py`.

use crate::app::CcbdApp;
use crate::services::project_namespace::ProjectNamespace;
use crate::services::start_policy::recovery_start_options;
use crate::start_flow::service::StartFlowResult;
use ccbr_agents::models::WindowSpec;

/// Injected start-flow implementation.
type RunStartFlowFn<'a> = &'a dyn Fn(
    &std::path::Path,
    &str,
    &str,
    &str,
    &[String],
    bool,
    bool,
    Option<&std::collections::HashMap<String, String>>,
    Option<Vec<WindowSpec>>,
) -> Result<StartFlowResult, String>;

/// Call the start flow for an additive mount.
#[allow(clippy::too_many_arguments)]
pub fn call_start_flow_for_additive_mount(
    app: &mut CcbdApp,
    namespace: &ProjectNamespace,
    agent_panes: &std::collections::HashMap<String, String>,
    requested_agents: &[String],
    restore: bool,
    auto_permission: bool,
    run_start_flow_fn: RunStartFlowFn<'_>,
) -> Result<StartFlowResult, String> {
    let project_root = std::path::Path::new(&namespace.project_root);
    let namespace_agent_panes = if agent_panes.is_empty() {
        None
    } else {
        Some(agent_panes)
    };
    let active_panes: Vec<String> = agent_panes.values().cloned().collect();
    let _ = active_panes;
    run_start_flow_fn(
        project_root,
        &namespace.project_id,
        &namespace.tmux_socket_path,
        &namespace.tmux_session_name,
        requested_agents,
        restore,
        auto_permission,
        namespace_agent_panes,
        namespace_window_specs(app, namespace),
    )
}

/// Load the start policy and return (restore, auto_permission).
pub fn start_options(supervisor: &CcbdApp, fallback_app: Option<&CcbdApp>) -> (bool, bool) {
    let store = Some(&supervisor.start_policy_store)
        .or_else(|| fallback_app.map(|a| &a.start_policy_store));
    let policy = store.and_then(|s| s.load().ok().flatten());
    recovery_start_options(policy.as_ref())
}

fn namespace_window_specs(app: &CcbdApp, namespace: &ProjectNamespace) -> Option<Vec<WindowSpec>> {
    app.current_config.as_ref().and_then(|config| {
        let expected_names: std::collections::HashSet<String> =
            namespace.windows.iter().map(|w| w.name.clone()).collect();
        config.windows.as_ref().map(|ws| {
            ws.iter()
                .filter(|w| expected_names.contains(&w.name))
                .cloned()
                .collect()
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::start_policy::StartPolicy;
    use crate::start_flow::service::StartFlowService;
    use std::collections::HashMap;

    #[test]
    fn test_start_options_with_policy() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            crate::stop_flow::service::StopFlowService::with_stub(),
        );
        app.start_policy_store
            .save(&StartPolicy {
                auto_permission: true,
                recovery_restore: false,
                source: "test".to_string(),
                created_at: "now".to_string(),
            })
            .unwrap();
        let (restore, auto) = start_options(&app, None);
        assert!(!restore);
        assert!(auto);
    }

    #[test]
    fn test_start_options_fallback() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            crate::stop_flow::service::StopFlowService::with_stub(),
        );
        let (restore, auto) = start_options(&app, None);
        assert!(!restore);
        assert!(!auto);
    }

    #[test]
    fn test_call_start_flow_for_additive_mount() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            crate::stop_flow::service::StopFlowService::with_stub(),
        );
        let mut panes = HashMap::new();
        panes.insert("claude".to_string(), "%1".to_string());
        let namespace = ProjectNamespace {
            project_root: dir.path().to_string_lossy().to_string(),
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
            agent_panes: panes.clone(),
            active_panes: vec!["%1".to_string()],
            namespace_epoch: 1,
            created_at: "now".to_string(),
        };
        let called = std::rc::Rc::new(std::cell::RefCell::new(None));
        let called2 = called.clone();
        let run_fn = move |_root: &std::path::Path,
                           _project_id: &str,
                           _socket: &str,
                           _session: &str,
                           agents: &[String],
                           _restore: bool,
                           _auto: bool,
                           _panes: Option<&HashMap<String, String>>,
                           _windows: Option<Vec<WindowSpec>>| {
            called2.borrow_mut().replace(agents.to_vec());
            Ok(StartFlowResult {
                status: "ok".to_string(),
                agent_results: vec![],
                actions_taken: vec![],
            })
        };
        let result = call_start_flow_for_additive_mount(
            &mut app,
            &namespace,
            &panes,
            &["claude".to_string()],
            false,
            false,
            &run_fn,
        );
        assert!(result.is_ok());
        assert_eq!(called.borrow().as_ref().unwrap(), &["claude".to_string()]);
    }
}
