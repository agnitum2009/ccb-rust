//! Mirrors Python `test/test_cli_kill_runtime_zombies.py`.

use ccbr_cli::kill_runtime::zombies::{
    find_all_zombie_sessions, kill_global_zombies, ZombieSession,
};
use std::io;

#[test]
fn is_pid_alive_treats_procfs_zombie_as_dead() {
    // The public `is_pid_alive` reads real `/proc`, so we verify the state
    // parser used by it through `proc_pid_state_at` with a fake proc root.
    let tmp = tempfile::TempDir::new().unwrap();
    let proc_root = tmp.path().join("proc");
    let stat_path = proc_root.join("123").join("stat");
    std::fs::create_dir_all(stat_path.parent().unwrap()).unwrap();
    std::fs::write(&stat_path, "123 (python) Z 1 2 3 4 5 ...\n").unwrap();

    assert_eq!(
        ccbr_cli::kill_runtime::processes::proc_pid_state_at(123, &proc_root),
        Some("Z".to_string())
    );
}

#[test]
fn is_pid_alive_keeps_uninterruptible_process_alive() {
    let tmp = tempfile::TempDir::new().unwrap();
    let proc_root = tmp.path().join("proc");
    let stat_path = proc_root.join("123").join("stat");
    std::fs::create_dir_all(stat_path.parent().unwrap()).unwrap();
    std::fs::write(&stat_path, "123 (python) D 1 2 3 4 5 ...\n").unwrap();

    assert_eq!(
        ccbr_cli::kill_runtime::processes::proc_pid_state_at(123, &proc_root),
        Some("D".to_string())
    );
}

#[test]
fn find_all_zombie_sessions_filters_dead_parents() {
    let sessions: Vec<String> = vec![
        "codex-123-worker".to_string(),
        "claude-456-run".to_string(),
        "agy-789-debugger".to_string(),
        "demo-other".to_string(),
    ];
    let is_pid_alive = |pid: u32| pid == 456;

    let zombies = find_all_zombie_sessions(is_pid_alive, Some(&|| sessions.clone()));

    assert_eq!(zombies.len(), 2);
    assert!(zombies.iter().any(|z| z.session == "codex-123-worker"));
    assert!(zombies.iter().any(|z| z.session == "agy-789-debugger"));
}

#[test]
fn kill_global_zombies_reports_partial_failures() {
    let zombies = vec![
        ZombieSession {
            session: "codex-123-worker".to_string(),
            provider: "codex".to_string(),
            parent_pid: 123,
        },
        ZombieSession {
            session: "claude-234-run".to_string(),
            provider: "claude".to_string(),
            parent_pid: 234,
        },
    ];
    let find_fn = |_alive: &dyn Fn(u32) -> bool,
                   _list: Option<&dyn Fn() -> Vec<String>>|
     -> Vec<ZombieSession> { zombies.clone() };
    let kill_fn = |name: &str| name == "codex-123-worker";

    let code = kill_global_zombies(true, |_| false, &find_fn, None, Some(&kill_fn));

    assert_eq!(code, 0);
}

#[test]
fn kill_global_zombies_cancels_without_yes() {
    let zombies = vec![ZombieSession {
        session: "codex-123-worker".to_string(),
        provider: "codex".to_string(),
        parent_pid: 123,
    }];
    let find_fn = |_alive: &dyn Fn(u32) -> bool,
                   _list: Option<&dyn Fn() -> Vec<String>>|
     -> Vec<ZombieSession> { zombies.clone() };
    let mut input = |_prompt: &str| -> io::Result<String> { Ok("n".to_string()) };

    let code = kill_global_zombies(
        false,
        |_| false,
        &find_fn,
        Some(&mut input),
        Some(&|_| true),
    );

    assert_eq!(code, 1);
}
