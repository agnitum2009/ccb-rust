//! Mirrors Python `test/test_cli_kill_runtime_processes.py`.

use ccb_cli::kill_runtime::processes::{is_pid_alive, kill_pid_tree_once, proc_pid_state_at};
use std::collections::HashMap;

#[cfg(unix)]
#[test]
fn kill_pid_tree_once_prefers_process_group_on_posix() {
    use std::os::unix::process::CommandExt;
    use std::time::{Duration, Instant};

    let mut child = unsafe {
        std::process::Command::new("sleep")
            .arg("60")
            .pre_exec(|| {
                // Put the child into its own process group so killpg targets it
                // without affecting the test runner.
                // Safety: called in the child process after fork; setpgid is
                // async-signal-safe.
                libc::setpgid(0, 0);
                Ok(())
            })
            .spawn()
            .expect("sleep should be available")
    };

    let pid = i64::from(child.id());
    let killed = kill_pid_tree_once(pid, false);
    assert!(killed, "kill_pid_tree_once should return true");

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && is_pid_alive(pid) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !is_pid_alive(pid),
        "child process tree should be terminated"
    );

    let _ = child.wait();
}

#[cfg(windows)]
#[test]
fn kill_pid_tree_once_uses_taskkill_on_windows() {
    // `timeout` is a built-in Windows command that blocks for N seconds.
    let mut child = std::process::Command::new("timeout")
        .args(["/t", "60"])
        .spawn()
        .expect("timeout should be available");

    let pid = i64::from(child.id());
    let killed = kill_pid_tree_once(pid, false);
    assert!(killed, "kill_pid_tree_once should return true");

    let start = Instant::now();
    while start.elapsed().as_secs() < 5 && is_pid_alive(pid) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !is_pid_alive(pid),
        "child process tree should be terminated"
    );

    let _ = child.wait();
}

#[test]
fn proc_pid_state_at_reads_zombie_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    let proc_root = tmp.path().join("proc");
    let stat_path = proc_root.join("123").join("stat");
    std::fs::create_dir_all(stat_path.parent().unwrap()).unwrap();
    // pid (comm) state ppid pgrp session ...
    std::fs::write(&stat_path, "123 (python) Z 1 2 3 4 5 ...\n").unwrap();

    assert_eq!(proc_pid_state_at(123, &proc_root), Some("Z".to_string()));
}

#[test]
fn proc_pid_state_at_reads_uninterruptible_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    let proc_root = tmp.path().join("proc");
    let stat_path = proc_root.join("456").join("stat");
    std::fs::create_dir_all(stat_path.parent().unwrap()).unwrap();
    std::fs::write(&stat_path, "456 (python) D 1 2 3 4 5 ...\n").unwrap();

    assert_eq!(proc_pid_state_at(456, &proc_root), Some("D".to_string()));
}

#[test]
fn collect_project_process_candidates_finds_ccbd_project_arg() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-control-plane-scan");
    std::fs::create_dir_all(project_root.join(".ccb")).unwrap();

    let proc_root = tmp.path().join("proc");
    std::fs::create_dir_all(proc_root.join("101")).unwrap();
    std::fs::create_dir_all(proc_root.join("102")).unwrap();

    let cmdlines: HashMap<u32, String> = [
        (
            101,
            format!(
                "/usr/bin/python /opt/ccb/lib/ccbd/main.py --project {}",
                project_root.display()
            ),
        ),
        (
            102,
            format!(
                "/usr/bin/python /opt/ccb/lib/ccbd/main.py --project {}",
                tmp.path().join("other").display()
            ),
        ),
    ]
    .into_iter()
    .collect();

    let candidates = ccb_runtime_pid_cleanup::collect_project_process_candidates(
        &project_root,
        &proc_root,
        |pid| cmdlines.get(&pid).cloned().unwrap_or_default(),
        Some(999),
    );

    assert_eq!(candidates.len(), 1);
    assert!(candidates.contains_key(&101));
    assert!(candidates[&101].contains(&project_root.join(".ccb").join("ccbd")));
}

#[test]
fn collect_project_authority_pid_candidates_reads_lifecycle() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-authority-lifecycle");
    let ccbd_dir = project_root.join(".ccb").join("ccbd");
    std::fs::create_dir_all(&ccbd_dir).unwrap();
    let lifecycle_path = ccbd_dir.join("lifecycle.json");
    std::fs::write(
        &lifecycle_path,
        serde_json::json!({"owner_pid": 321, "keeper_pid": 654}).to_string(),
    )
    .unwrap();

    let candidates =
        ccb_runtime_pid_cleanup::collect_project_authority_pid_candidates(&project_root);

    assert_eq!(candidates.len(), 2);
    assert!(candidates.contains_key(&321));
    assert!(candidates.contains_key(&654));
    assert_eq!(candidates[&321], vec![lifecycle_path.clone()]);
    assert_eq!(candidates[&654], vec![lifecycle_path]);
}
