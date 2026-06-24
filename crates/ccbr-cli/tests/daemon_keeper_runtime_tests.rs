//! Mirrors Python `test/test_cli_daemon_keeper_runtime.py`.

use ccbr_cli::services::daemon_runtime::keeper::{
    ensure_keeper_started_for_context, spawn_keeper_process_with, KeeperContext, KeeperSpawn,
    OwnershipGuard,
};
use std::path::Path;
use std::sync::{Arc, Mutex};

struct TestGuard;

impl OwnershipGuard for TestGuard {
    fn with_startup_lock<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        f()
    }
}

fn save_keeper_state(context: &KeeperContext, keeper_pid: u32, state: &str) {
    let path = context.paths.ccbrd_dir().as_std_path().join("keeper.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        serde_json::json!({
            "schema_version": 1,
            "record_type": "ccbrd_keeper",
            "project_id": context.project_id,
            "keeper_pid": keeper_pid,
            "started_at": "2026-05-23T00:00:00Z",
            "last_check_at": "2026-05-23T00:00:00Z",
            "state": state,
        })
        .to_string(),
    )
    .unwrap();
}

#[test]
fn spawn_keeper_process_uses_lib_root_keeper_main() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = KeeperContext::from_project_root(tmp.path());

    let captured = Arc::new(Mutex::new(None::<KeeperSpawn>));
    let captured_clone = captured.clone();
    spawn_keeper_process_with(&context, |spawn| {
        *captured_clone.lock().unwrap() = Some(spawn);
        Ok(())
    })
    .unwrap();

    let spawn = captured.lock().unwrap().take().unwrap();
    let lib_root = ccbr_cli::services::daemon_runtime::keeper::keeper_lib_root();
    let expected_script = lib_root.join("ccbrd").join("keeper_main.py");

    assert_eq!(spawn.program, Path::new("python"));
    assert_eq!(spawn.args[0], expected_script.to_string_lossy().to_string());
    assert_eq!(spawn.args[1], "--project");
    assert_eq!(
        spawn.args[2],
        context.project_root.to_string_lossy().to_string()
    );
    let pythonpath = spawn
        .env
        .get("PYTHONPATH")
        .expect("PYTHONPATH should be set");
    assert!(
        pythonpath.contains(&lib_root.to_string_lossy().to_string()),
        "PYTHONPATH ({}) should contain lib root ({})",
        pythonpath,
        lib_root.display()
    );
}

#[test]
fn ensure_keeper_started_replaces_state_for_unrelated_live_pid() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = KeeperContext::from_project_root(tmp.path());
    save_keeper_state(&context, 28, "running");

    let spawn_calls = Arc::new(Mutex::new(Vec::new()));
    let spawn_calls_clone = spawn_calls.clone();
    let context_for_spawn = context.clone();

    let started = ensure_keeper_started_for_context(
        &context,
        |_paths| (),
        |_paths, _manager| TestGuard,
        |pid| pid == 28 || pid == 777,
        |pid| {
            if pid == 28 {
                vec!["[idle_inject/4]".to_string()]
            } else if pid == 777 {
                vec![
                    "python3".to_string(),
                    "/repo/lib/ccbrd/keeper_main.py".to_string(),
                    "--project".to_string(),
                    context.project_root.to_string_lossy().to_string(),
                ]
            } else {
                Vec::new()
            }
        },
        move |ctx| {
            spawn_calls_clone
                .lock()
                .unwrap()
                .push(ctx.project_root.clone());
            save_keeper_state(&context_for_spawn, 777, "running");
        },
        0.1,
    );

    assert!(started, "keeper should be reported as started");
    assert_eq!(
        spawn_calls.lock().unwrap().len(),
        1,
        "spawn should be called once for unrelated PID"
    );
}

#[test]
fn ensure_keeper_started_reuses_matching_keeper_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = KeeperContext::from_project_root(tmp.path());
    save_keeper_state(&context, 777, "running");

    let spawn_calls = Arc::new(Mutex::new(Vec::new()));
    let spawn_calls_clone = spawn_calls.clone();

    let started = ensure_keeper_started_for_context(
        &context,
        |_paths| (),
        |_paths, _manager| TestGuard,
        |pid| pid == 777,
        |_pid| {
            vec![
                "python3".to_string(),
                "/repo/lib/ccbrd/keeper_main.py".to_string(),
                "--project".to_string(),
                context.project_root.to_string_lossy().to_string(),
            ]
        },
        move |_ctx| {
            spawn_calls_clone.lock().unwrap().push(());
        },
        0.1,
    );

    assert!(started, "keeper should be reported as started");
    assert!(
        spawn_calls.lock().unwrap().is_empty(),
        "spawn should not be called when matching state exists"
    );
}
