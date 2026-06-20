//! Mirrors Python `test/test_terminal_runtime_backend_selection.py`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use ccb_terminal::backend::TmuxBackend;
use ccb_terminal::backend_selection::{TerminalBackendSelection, TerminalLayoutService};
use ccb_terminal::layouts::LayoutResult;
use ccb_terminal::registry::UserSession;

#[test]
fn test_backend_selection_caches_detected_backend() {
    let count = Arc::new(AtomicUsize::new(0));
    let count_factory = count.clone();
    let mut selection = TerminalBackendSelection::with_deps(
        || Some("tmux".to_string()),
        move || {
            count_factory.fetch_add(1, Ordering::SeqCst);
            TmuxBackend::new(None, None)
        },
    );

    let _ = selection.get_backend().unwrap();
    let _ = selection.get_backend().unwrap();

    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_backend_selection_uses_session_terminal_field() {
    let selection = TerminalBackendSelection::with_deps(|| None, || unreachable!());

    let session = UserSession {
        terminal: Some("tmux".to_string()),
        tmux_socket_name: Some("sock-demo".to_string()),
        tmux_socket_path: None,
        pane_id: None,
        tmux_session: None,
    };
    let backend = selection.get_backend_for_session(&session);
    let base = backend.tmux_base();
    assert!(base.contains(&"-L".to_string()));
    assert!(base.contains(&"sock-demo".to_string()));

    let session2 = UserSession {
        terminal: Some("tmux".to_string()),
        tmux_socket_name: None,
        tmux_socket_path: Some("/tmp/ccb.sock".to_string()),
        pane_id: None,
        tmux_session: None,
    };
    let backend2 = selection.get_backend_for_session(&session2);
    let base2 = backend2.tmux_base();
    assert!(base2.contains(&"-S".to_string()));
    assert!(base2.contains(&"/tmp/ccb.sock".to_string()));

    assert_eq!(
        selection.get_pane_id_from_session(&UserSession {
            pane_id: Some("%1".to_string()),
            ..Default::default()
        }),
        Some("%1".to_string())
    );
    assert_eq!(
        selection.get_pane_id_from_session(&UserSession {
            tmux_session: Some("%old".to_string()),
            ..Default::default()
        }),
        Some("%old".to_string())
    );
}

#[test]
fn test_terminal_layout_service_delegates_to_runtime_layout() {
    let captured: Arc<std::sync::Mutex<HashMap<String, String>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));
    let captured_closure = captured.clone();

    let service = TerminalLayoutService::new(
        || TmuxBackend::new(None, None),
        |_cwd| "ccb-demo-1".to_string(),
        Some({
            let mut env = HashMap::new();
            env.insert("TMUX".to_string(), "/tmp/tmux".to_string());
            env
        }),
    )
    .with_layout_fn(Box::new(
        move |providers, cwd, backend, detached_session_name, inside_tmux| {
            let mut panes = HashMap::new();
            panes.insert("a1".to_string(), "%root".to_string());
            let mut c = captured_closure.lock().unwrap();
            c.insert("providers".to_string(), providers.join(","));
            c.insert("cwd".to_string(), cwd.to_string());
            c.insert(
                "socket".to_string(),
                backend.tmux_base().get(1).cloned().unwrap_or_default(),
            );
            c.insert(
                "detached_session_name".to_string(),
                detached_session_name.to_string(),
            );
            c.insert("inside_tmux".to_string(), inside_tmux.to_string());
            Ok(LayoutResult {
                panes,
                root_pane_id: "%root".to_string(),
                needs_attach: false,
                created_panes: Vec::new(),
            })
        },
    ));

    let result = service
        .create_auto_layout(vec!["a1".to_string()], "/tmp/demo")
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(result.panes["a1"], "%root");
    assert_eq!(c["providers"], "a1");
    assert_eq!(c["cwd"], "/tmp/demo");
    assert_eq!(c["detached_session_name"], "ccb-demo-1");
    assert_eq!(c["inside_tmux"], "true");
}
