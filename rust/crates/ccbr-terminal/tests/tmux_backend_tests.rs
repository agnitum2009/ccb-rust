use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use ccbr_terminal::{TerminalBackend, TmuxBackend, TmuxLayoutBackend};

fn test_backend() -> TmuxBackend {
    // Provide explicit empty socket values so the constructor does not pick up
    // CCB_TMUX_SOCKET* environment variables left over from other tests.
    TmuxBackend::new(Some(String::new()), Some(String::new()))
}

fn ok_output(stdout: &str) -> ccbr_terminal::TmuxOutput {
    let status = std::process::Command::new("true").status().unwrap();
    ccbr_terminal::TmuxOutput {
        stdout: stdout.to_string(),
        stderr: String::new(),
        status,
    }
}

#[test]
fn test_tmux_backend_run_strips_outer_tmux_environment() {
    let captured = Arc::new(Mutex::new(None));
    let cap = captured.clone();

    std::env::set_var("TMUX", "/tmp/tmux-1000/default,123,0");
    std::env::set_var("TMUX_PANE", "%77");
    std::env::set_var("CCB_TMUX_SOCKET", "outer");
    std::env::set_var("CCB_TMUX_SOCKET_PATH", "/tmp/outer.sock");
    std::env::set_var("TERM", "xterm-ghostty");

    let backend = TmuxBackend::new(None, Some("/tmp/project.sock".into())).with_runner(
        move |args, _check, _capture, _input, _timeout, env| {
            *cap.lock().unwrap() = Some((
                args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                env.iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<Vec<_>>(),
            ));
            Ok(ok_output(""))
        },
    );

    backend
        .tmux_run(&["display-message", "-p", "ok"], false, false, None, None)
        .unwrap();

    let (args, env) = captured.lock().unwrap().take().unwrap();
    assert_eq!(&args[..4], &["tmux", "-f", "/dev/null", "-S"]);
    assert_eq!(args[4], "/tmp/project.sock");

    let env_keys: HashSet<_> = env.iter().map(|(k, _)| k.as_str()).collect();
    assert!(!env_keys.contains("TMUX"));
    assert!(!env_keys.contains("TMUX_PANE"));
    assert!(!env_keys.contains("CCB_TMUX_SOCKET"));
    assert!(!env_keys.contains("CCB_TMUX_SOCKET_PATH"));
    assert_eq!(
        env.iter()
            .find(|(k, _)| k == "TERM")
            .map(|(_, v)| v.as_str()),
        Some("xterm-256color")
    );
}

fn fake_response(args: &[String]) -> String {
    if args.iter().any(|a| a == "split-window") {
        "%42\n".to_string()
    } else if args.last().map(|s| s.as_str()) == Some("#{pane_dead}") {
        "0\n".to_string()
    } else if args.last().map(|s| s.as_str()) == Some("#{pane_width}x#{pane_height}") {
        "80x24\n".to_string()
    } else if args.last().map(|s| s.as_str()) == Some("#{pane_id}") {
        "%1\n".to_string()
    } else if args.last().map(|s| s.as_str()) == Some("#{window_zoomed_flag}") {
        "0\n".to_string()
    } else {
        String::new()
    }
}

#[test]
fn test_tmux_split_pane_builds_command_and_parses_pane_id() {
    let calls = Arc::new(Mutex::new(Vec::<(
        Vec<String>,
        bool,
        bool,
        Option<Vec<u8>>,
        Option<std::time::Duration>,
        Vec<(String, String)>,
    )>::new()));
    let calls_c = calls.clone();

    let backend = test_backend().with_runner(move |args, check, capture, input, timeout, env| {
        calls_c
            .lock()
            .unwrap()
            .push((args.clone(), check, capture, input, timeout, env));
        Ok(ok_output(&fake_response(&args)))
    });

    let pane_id = TmuxLayoutBackend::split_pane(&backend, "%1", "right", 50).unwrap();
    assert_eq!(pane_id, "%42");

    let split_call = calls
        .lock()
        .unwrap()
        .iter()
        .find(|(args, ..)| args.iter().any(|a| a == "split-window"))
        .cloned()
        .expect("split-window call should be recorded");

    assert!(split_call.1, "check should be true");
    assert!(split_call.2, "capture should be true");

    let argv = split_call.0;
    let pos = argv
        .iter()
        .position(|a| a == "split-window")
        .expect("split-window subcommand should be present");
    assert_eq!(&argv[pos..pos + 2], &["split-window", "-h"]);
    assert!(!argv.iter().any(|a| a.starts_with("-p")));
    assert!(argv.contains(&"-t".to_string()) && argv.contains(&"%1".to_string()));
    assert!(argv.contains(&"-P".to_string()));
    assert!(argv.contains(&"-F".to_string()) && argv.contains(&"#{pane_id}".to_string()));
}

#[test]
fn test_tmux_split_pane_can_start_command_atomically() {
    let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let calls_c = calls.clone();

    let backend =
        test_backend().with_runner(move |args, _check, _capture, _input, _timeout, _env| {
            calls_c.lock().unwrap().push(args.clone());
            Ok(ok_output(&fake_response(&args)))
        });

    let pane_id = backend
        .split_pane(
            "%1",
            "right",
            50,
            Some("while :; do sleep 3600; done"),
            Some("/tmp/demo"),
        )
        .unwrap();
    assert_eq!(pane_id, "%42");

    let argv = calls
        .lock()
        .unwrap()
        .iter()
        .find(|args| args.iter().any(|a| a == "split-window"))
        .cloned()
        .expect("split-window call should be recorded");

    assert_eq!(
        &argv[argv.len() - 3..],
        &["sh", "-lc", "while :; do sleep 3600; done"]
    );
    assert!(argv.contains(&"-c".to_string()));
    let c_idx = argv.iter().position(|a| a == "-c").unwrap();
    assert_eq!(argv[c_idx + 1], "/tmp/demo");
}

fn show_option_response(args: &[String]) -> String {
    if args.iter().any(|a| a == "show-option") {
        "/bin/bash\n".to_string()
    } else {
        fake_response(args)
    }
}

#[test]
fn test_tmux_create_pane_keeps_provider_start_on_respawn_path() {
    let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let calls_c = calls.clone();

    let backend =
        test_backend().with_runner(move |args, _check, _capture, _input, _timeout, _env| {
            calls_c.lock().unwrap().push(args.clone());
            Ok(ok_output(&show_option_response(&args)))
        });

    let pane_id = TerminalBackend::create_pane(
        &backend,
        "codex --dangerously-bypass-approvals",
        "/tmp/demo",
        "right",
        50,
        Some("%1"),
    )
    .unwrap();
    assert_eq!(pane_id, "%42");

    let split_argv = calls
        .lock()
        .unwrap()
        .iter()
        .find(|args| args.iter().any(|a| a == "split-window"))
        .cloned()
        .expect("split-window call should be recorded");

    assert!(!split_argv.iter().any(|a| a.contains("codex")));
    assert_eq!(&split_argv[split_argv.len() - 2..], &["-F", "#{pane_id}"]);
    assert!(calls.lock().unwrap().iter().any(|args| {
        args.iter().any(|a| a == "respawn-pane") && args.iter().any(|a| a == "-k")
    }));
}

#[test]
fn test_tmux_create_detached_pane_starts_placeholder_before_respawn() {
    let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let calls_c = calls.clone();

    let backend =
        test_backend().with_runner(move |args, _check, _capture, _input, _timeout, _env| {
            calls_c.lock().unwrap().push(args.clone());
            let resp = if args.iter().any(|a| a == "show-option") {
                "/bin/bash\n".to_string()
            } else if args.iter().any(|a| a == "list-panes") {
                "%42\n".to_string()
            } else if args.iter().any(|a| a == "display-message") {
                // No current tmux pane exists in this test scenario.
                String::new()
            } else {
                String::new()
            };
            Ok(ok_output(&resp))
        });

    let pane_id = TerminalBackend::create_pane(
        &backend,
        "codex --dangerously-bypass-approvals",
        "/tmp/demo",
        "right",
        50,
        None,
    )
    .unwrap();
    assert_eq!(pane_id, "%42");

    let new_session = calls
        .lock()
        .unwrap()
        .iter()
        .find(|args| args.iter().any(|a| a == "new-session"))
        .cloned()
        .expect("new-session call should be recorded");
    assert_eq!(
        &new_session[new_session.len() - 3..],
        &["sh", "-lc", "while :; do sleep 3600; done"]
    );
    assert!(calls.lock().unwrap().iter().any(|args| {
        args.iter().any(|a| a == "respawn-pane") && args.iter().any(|a| a == "-k")
    }));
}

#[test]
fn test_tmux_find_pane_by_title_marker_parses_list_panes() {
    let captured = Arc::new(Mutex::new(None));
    let cap = captured.clone();

    let backend =
        test_backend().with_runner(move |args, _check, capture, _input, _timeout, _env| {
            *cap.lock().unwrap() = Some((args.clone(), capture));
            Ok(ok_output("%1\tCCB-opencode-abc\n%2\tOTHER\n"))
        });

    assert_eq!(
        backend.find_pane_by_title_marker("CCB-opencode"),
        Some("%1".to_string())
    );
    assert_eq!(backend.find_pane_by_title_marker("NOPE"), None);

    let (args, capture) = captured.lock().unwrap().take().unwrap();
    assert!(capture);
    let pos = args.iter().position(|a| a == "list-panes").unwrap();
    assert_eq!(
        &args[pos..],
        &["list-panes", "-a", "-F", "#{pane_id}\t#{pane_title}"]
    );
}

#[test]
fn test_tmux_find_pane_by_title_marker_rejects_ambiguous_prefix() {
    let backend =
        test_backend().with_runner(move |_args, _check, _capture, _input, _timeout, _env| {
            Ok(ok_output("%1\tCCB-codex-abc\n%2\tCCB-codex-def\n"))
        });

    assert_eq!(backend.find_pane_by_title_marker("CCB-codex"), None);
}

#[test]
fn test_tmux_describe_pane_reads_title_and_user_options() {
    let captured = Arc::new(Mutex::new(None));
    let cap = captured.clone();

    let backend =
        test_backend().with_runner(move |args, _check, capture, _input, _timeout, _env| {
            *cap.lock().unwrap() = Some((args.clone(), capture));
            Ok(ok_output("%7\tagent2\t0\tagent2\tproj-7\n"))
        });

    let info = backend
        .describe_pane("%7", &["@ccbr_agent", "@ccbr_project_id"])
        .expect("describe_pane should return a map");
    assert_eq!(info.get("pane_id"), Some(&"%7".to_string()));
    assert_eq!(info.get("pane_title"), Some(&"agent2".to_string()));
    assert_eq!(info.get("pane_dead"), Some(&"0".to_string()));
    assert_eq!(info.get("@ccbr_agent"), Some(&"agent2".to_string()));
    assert_eq!(info.get("@ccbr_project_id"), Some(&"proj-7".to_string()));

    let (args, capture) = captured.lock().unwrap().take().unwrap();
    assert!(capture);
    let pos = args.iter().position(|a| a == "display-message").unwrap();
    assert_eq!(
        &args[pos..],
        &[
            "display-message",
            "-p",
            "-t",
            "%7",
            "#{pane_id}\t#{pane_title}\t#{pane_dead}\t#{@ccbr_agent}\t#{@ccbr_project_id}"
        ]
    );
}

#[test]
fn test_tmux_is_pane_alive_uses_pane_dead_zero() {
    let backend =
        test_backend().with_runner(move |args, _check, capture, _input, _timeout, _env| {
            assert!(args.iter().any(|a| a == "display-message"));
            assert!(capture);
            assert_eq!(args.last().map(|s| s.as_str()), Some("#{pane_dead}"));
            Ok(ok_output("0\n"))
        });
    assert!(backend.is_pane_alive("%9"));
}

#[test]
fn test_tmux_is_pane_alive_uses_pane_dead_one() {
    let backend = test_backend()
        .with_runner(move |_args, _check, _capture, _input, _timeout, _env| Ok(ok_output("1\n")));
    assert!(!backend.is_pane_alive("%9"));
}

#[test]
fn test_tmux_is_pane_alive_uses_pane_dead_empty() {
    let backend = test_backend()
        .with_runner(move |_args, _check, _capture, _input, _timeout, _env| Ok(ok_output("")));
    assert!(!backend.is_pane_alive("%9"));
}

#[test]
fn test_tmux_send_text_always_deletes_buffer() {
    let calls = Arc::new(Mutex::new(Vec::<(Vec<String>, Option<Vec<u8>>)>::new()));
    let calls_c = calls.clone();

    let backend =
        test_backend().with_runner(move |args, _check, _capture, input, _timeout, _env| {
            let is_paste = args.iter().any(|a| a == "paste-buffer");
            calls_c.lock().unwrap().push((args.clone(), input));
            if is_paste {
                return Err(std::io::Error::other("paste-buffer failed"));
            }
            // ensure_not_in_copy_mode asks for pane_in_mode.
            if args.last().map(|s| s.as_str()) == Some("#{pane_in_mode}") {
                return Ok(ok_output("0\n"));
            }
            Ok(ok_output(""))
        });

    assert!(TerminalBackend::send_text(&backend, "%1", "hello").is_err());

    let guard = calls.lock().unwrap();
    assert!(guard.iter().any(|(args, _)| {
        args.windows(2)
            .any(|w| w[0] == "load-buffer" && w[1] == "-b")
    }));
    assert!(guard.iter().any(|(args, _)| {
        args.iter().any(|a| a == "paste-buffer") && args.iter().any(|a| a == "-p")
    }));
    assert!(guard.iter().any(|(args, _)| {
        args.windows(2)
            .any(|w| w[0] == "delete-buffer" && w[1] == "-b")
    }));
}

#[test]
fn test_tmux_strict_pane_helpers_reject_session_names() {
    let backend = test_backend();

    assert!(backend.send_text_to_pane("mysession", "hello").is_err());
    assert!(backend.is_tmux_pane_alive("mysession").is_err());
    assert!(backend.kill_tmux_pane("mysession").is_err());
    assert!(backend.activate_tmux_pane("mysession").is_err());
}

#[test]
fn test_tmux_kill_pane_prefers_pane_id_over_session() {
    let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let calls_c = calls.clone();

    let backend =
        test_backend().with_runner(move |args, _check, _capture, _input, _timeout, _env| {
            calls_c.lock().unwrap().push(args.clone());
            Ok(ok_output(""))
        });

    TerminalBackend::kill_pane(&backend, "%1").unwrap();
    {
        let guard = calls.lock().unwrap();
        assert_eq!(
            guard.last().map(|args| args[3..].to_vec()),
            Some(vec![
                "kill-pane".to_string(),
                "-t".to_string(),
                "%1".to_string()
            ])
        );
    }

    TerminalBackend::kill_pane(&backend, "mysession").unwrap();
    {
        let guard = calls.lock().unwrap();
        assert_eq!(
            guard.last().map(|args| args[3..].to_vec()),
            Some(vec![
                "kill-session".to_string(),
                "-t".to_string(),
                "mysession".to_string()
            ])
        );
    }
}

#[test]
fn test_tmux_strict_kill_and_activate_only_use_pane_targets() {
    let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let calls_c = calls.clone();

    let backend =
        test_backend().with_runner(move |args, _check, capture, _input, _timeout, _env| {
            calls_c.lock().unwrap().push(args.clone());
            if capture {
                Ok(ok_output("demo-session\n"))
            } else {
                Ok(ok_output(""))
            }
        });

    backend.kill_tmux_pane("%7").unwrap();
    backend.activate_tmux_pane("%7").unwrap();

    let guard = calls.lock().unwrap();
    assert_eq!(
        guard[0][3..].to_vec(),
        vec!["kill-pane".to_string(), "-t".to_string(), "%7".to_string()]
    );
    assert_eq!(
        guard[1][3..].to_vec(),
        vec![
            "select-pane".to_string(),
            "-t".to_string(),
            "%7".to_string()
        ]
    );
}
