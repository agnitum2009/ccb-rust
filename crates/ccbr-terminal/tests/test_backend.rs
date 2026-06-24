use std::time::{SystemTime, UNIX_EPOCH};

use ccbr_terminal::{TerminalBackend, TmuxBackend};

fn unique_socket_path() -> String {
    let tmp = std::env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    tmp.join(format!(
        "ccbr-terminal-test-{}-{}.sock",
        std::process::id(),
        ts
    ))
    .to_string_lossy()
    .to_string()
}

fn unique_session_name() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("ccbr-term-test-{}-{}", std::process::id(), ts)
}

fn cleanup(backend: &TmuxBackend, session: &str) {
    let _ = backend.kill_pane(session);
    let _ = std::fs::remove_file(backend.socket_path().unwrap_or(""));
}

#[test]
fn test_backend_send_text_to_session_name() {
    let socket_path = unique_socket_path();
    let backend = TmuxBackend::new(None, Some(socket_path.clone()));
    let session = unique_session_name();

    // Create a detached session so we have a valid session target.
    backend
        .tmux_run(
            &[
                "new-session",
                "-d",
                "-s",
                &session,
                "-c",
                "/tmp",
                "sh",
                "-lc",
                "while :; do sleep 3600; done",
            ],
            true,
            false,
            None,
            None,
        )
        .unwrap();

    // send_text accepts a session name (not only a pane id).
    backend.send_text(&session, "echo hello").unwrap();

    cleanup(&backend, &session);
}

#[test]
fn test_backend_send_text_to_pane_target() {
    let socket_path = unique_socket_path();
    let backend = TmuxBackend::new(None, Some(socket_path.clone()));
    let session = unique_session_name();

    backend
        .tmux_run(
            &[
                "new-session",
                "-d",
                "-s",
                &session,
                "-c",
                "/tmp",
                "sh",
                "-lc",
                "while :; do sleep 3600; done",
            ],
            true,
            false,
            None,
            None,
        )
        .unwrap();

    let pane_id = backend
        .tmux_run_capture(&["list-panes", "-t", &session, "-F", "#{pane_id}"])
        .unwrap()
        .trim()
        .to_string();

    // send_text accepts a pane id and uses the buffer-based path for multi-line text.
    backend
        .send_text(&pane_id, "printf 'one\\ntwo\\n'")
        .unwrap();

    cleanup(&backend, &session);
}

#[test]
fn test_backend_create_pane_detached_root() {
    let socket_path = unique_socket_path();
    let backend = TmuxBackend::new(None, Some(socket_path.clone()));

    // create_pane with no parent allocates a detached session root pane.
    let pane_id = backend.create_pane("", "/tmp", "right", 50, None).unwrap();
    assert!(pane_id.starts_with('%'));
    assert!(backend.is_alive(&pane_id).unwrap());

    // Resolve the session from the pane and clean it up.
    let session = backend
        .tmux_run_capture(&["display-message", "-p", "-t", &pane_id, "#{session_name}"])
        .unwrap()
        .trim()
        .to_string();
    cleanup(&backend, &session);
}

#[test]
fn test_backend_create_pane_with_parent_and_command() {
    let socket_path = unique_socket_path();
    let backend = TmuxBackend::new(None, Some(socket_path.clone()));
    let session = unique_session_name();

    backend
        .tmux_run(
            &[
                "new-session",
                "-d",
                "-s",
                &session,
                "-c",
                "/tmp",
                "sh",
                "-lc",
                "while :; do sleep 3600; done",
            ],
            true,
            false,
            None,
            None,
        )
        .unwrap();

    let parent = backend
        .tmux_run_capture(&["list-panes", "-t", &session, "-F", "#{pane_id}"])
        .unwrap()
        .trim()
        .to_string();

    let child = backend
        .create_pane("echo hi", "/tmp", "right", 50, Some(&parent))
        .unwrap();
    assert!(child.starts_with('%'));
    assert!(backend.is_alive(&child).unwrap());

    cleanup(&backend, &session);
}

#[test]
fn test_backend_is_alive_and_kill_pane() {
    let socket_path = unique_socket_path();
    let backend = TmuxBackend::new(None, Some(socket_path.clone()));
    let session = unique_session_name();

    backend
        .tmux_run(
            &[
                "new-session",
                "-d",
                "-s",
                &session,
                "-c",
                "/tmp",
                "sh",
                "-lc",
                "while :; do sleep 3600; done",
            ],
            true,
            false,
            None,
            None,
        )
        .unwrap();

    let pane_id = backend
        .tmux_run_capture(&["list-panes", "-t", &session, "-F", "#{pane_id}"])
        .unwrap()
        .trim()
        .to_string();

    assert!(backend.is_alive(&pane_id).unwrap());
    backend.kill_pane(&pane_id).unwrap();
    assert!(!backend.is_alive(&pane_id).unwrap());

    cleanup(&backend, &session);
}
