use std::collections::HashMap;

use ccbr_terminal::panes::{TmuxRunOutput, TmuxRunner};
use ccbr_terminal::respawn::TmuxRespawnService;

fn ok(stdout: &str) -> TmuxRunOutput {
    TmuxRunOutput {
        stdout: stdout.to_string(),
        stderr: String::new(),
        returncode: 0,
    }
}

fn err(stderr: &str) -> TmuxRunOutput {
    TmuxRunOutput {
        stdout: String::new(),
        stderr: stderr.to_string(),
        returncode: 1,
    }
}

#[test]
fn test_tmux_respawn_service_retries_transient_tmux_failures() {
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<String>>::new()));
    let calls_clone = calls.clone();
    let respawn_attempts = std::sync::Arc::new(std::sync::Mutex::new(0));
    let attempts_clone = respawn_attempts.clone();
    let runner: Box<dyn TmuxRunner> = Box::new(
        move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
            calls_clone
                .lock()
                .unwrap()
                .push(args.iter().map(|s| s.to_string()).collect());
            if args == ["show-option", "-gqv", "default-shell"] {
                return Ok(ok("/bin/bash\n"));
            }
            if !args.is_empty() && args[0] == "respawn-pane" {
                let mut attempts = attempts_clone.lock().unwrap();
                *attempts += 1;
                if *attempts == 1 {
                    return Ok(err("no server running on /tmp/ccbr-runtime/test.sock\n"));
                }
            }
            Ok(ok(""))
        },
    );
    let service = TmuxRespawnService::new(
        runner,
        |_pane_id| {},
        HashMap::from_iter([("SHELL".to_string(), "/bin/bash".to_string())]),
    );

    service
        .respawn_pane("%9", "echo hi", None, None, false)
        .unwrap();

    assert_eq!(*respawn_attempts.lock().unwrap(), 2);
    let calls = calls.lock().unwrap();
    assert!(calls.iter().any(|c| {
        c.len() >= 5 && c[0] == "respawn-pane" && c[1] == "-k" && c[2] == "-t" && c[3] == "%9"
    }));
}

#[test]
fn test_tmux_respawn_service_does_not_retry_non_transient_failure() {
    let respawn_attempts = std::sync::Arc::new(std::sync::Mutex::new(0));
    let attempts_clone = respawn_attempts.clone();
    let runner: Box<dyn TmuxRunner> = Box::new(
        move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
            if args == ["show-option", "-gqv", "default-shell"] {
                return Ok(ok("/bin/bash\n"));
            }
            if !args.is_empty() && args[0] == "respawn-pane" {
                let mut attempts = attempts_clone.lock().unwrap();
                *attempts += 1;
                return Ok(err("pane not found\n"));
            }
            Ok(ok(""))
        },
    );
    let service = TmuxRespawnService::new(
        runner,
        |_pane_id| {},
        HashMap::from_iter([("SHELL".to_string(), "/bin/bash".to_string())]),
    );

    let result = service.respawn_pane("%9", "echo hi", None, None, false);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("pane not found"));
    assert_eq!(*respawn_attempts.lock().unwrap(), 1);
}

#[test]
fn test_tmux_respawn_service_retries_all_transient_failure_patterns() {
    for pattern in [
        "fork failed\n",
        "no server running on /tmp/ccbr-runtime/test.sock\n",
        "server exited unexpectedly\n",
    ] {
        let respawn_attempts = std::sync::Arc::new(std::sync::Mutex::new(0));
        let attempts_clone = respawn_attempts.clone();
        let runner: Box<dyn TmuxRunner> = Box::new(
            move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
                if args == ["show-option", "-gqv", "default-shell"] {
                    return Ok(ok("/bin/bash\n"));
                }
                if !args.is_empty() && args[0] == "respawn-pane" {
                    let mut attempts = attempts_clone.lock().unwrap();
                    *attempts += 1;
                    if *attempts == 1 {
                        return Ok(err(pattern));
                    }
                }
                Ok(ok(""))
            },
        );
        let service = TmuxRespawnService::new(
            runner,
            |_pane_id| {},
            HashMap::from_iter([("SHELL".to_string(), "/bin/bash".to_string())]),
        );
        service
            .respawn_pane("%9", "echo hi", None, None, false)
            .unwrap();
        assert_eq!(*respawn_attempts.lock().unwrap(), 2, "pattern: {pattern:?}");
    }
}

#[test]
fn test_tmux_respawn_service_uses_shared_ready_budget_for_transient_failures() {
    let respawn_attempts = std::sync::Arc::new(std::sync::Mutex::new(0));
    let attempts_clone = respawn_attempts.clone();
    let runner: Box<dyn TmuxRunner> = Box::new(
        move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
            if args == ["show-option", "-gqv", "default-shell"] {
                return Ok(ok("/bin/bash\n"));
            }
            if !args.is_empty() && args[0] == "respawn-pane" {
                *attempts_clone.lock().unwrap() += 1;
                return Ok(err("no server running on /tmp/ccbr-runtime/test.sock\n"));
            }
            Ok(ok(""))
        },
    );
    std::env::set_var("CCBR_TMUX_OBJECT_READY_TIMEOUT_S", "0.15");
    std::env::set_var("CCBR_TMUX_OBJECT_READY_POLL_INTERVAL_S", "0.01");
    let service = TmuxRespawnService::new(
        runner,
        |_pane_id| {},
        HashMap::from_iter([("SHELL".to_string(), "/bin/bash".to_string())]),
    );
    let result = service.respawn_pane("%9", "echo hi", None, None, false);
    std::env::remove_var("CCBR_TMUX_OBJECT_READY_TIMEOUT_S");
    std::env::remove_var("CCBR_TMUX_OBJECT_READY_POLL_INTERVAL_S");
    assert!(result.is_err());
    let attempts = *respawn_attempts.lock().unwrap();
    assert!(
        (10..=30).contains(&attempts),
        "expected retry budget to be shared, got {attempts} attempts"
    );
}
