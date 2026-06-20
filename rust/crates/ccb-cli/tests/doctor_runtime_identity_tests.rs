//! Mirrors Python `test/test_doctor_runtime_identity.py`.

use ccb_cli::ops_views_doctor::render_doctor;
use ccb_cli::services::doctor_runtime::system::{
    runtime_identity_summary_with, user_name,
};
use serde_json::Value;
use std::path::Path;

#[test]
fn test_runtime_identity_summary_reports_root_project_owner_warning() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().join("project");
    let ccb_dir = project_root.join(".ccb");
    let install_dir = tmp.path().join("install");
    std::fs::create_dir_all(&ccb_dir).unwrap();
    std::fs::create_dir_all(&install_dir).unwrap();

    let fake_path_owner = |path: &Path| -> Option<Value> {
        if path == project_root || path == ccb_dir {
            Some(serde_json::json!({"uid": 1000, "name": "demo"}))
        } else if path == install_dir {
            Some(serde_json::json!({"uid": 0, "name": "root"}))
        } else {
            None
        }
    };

    let installation = serde_json::json!({
        "path": install_dir,
        "root_install": true,
        "install_user_id": "0",
        "install_user_name": "root",
        "sudo_user": "demo",
    });

    // The public summary API uses real filesystem ownership. For this parity
    // test we inject the same ownership data the Python test stubs out.
    let summary = runtime_identity_summary_with(
        &project_root,
        Some(&ccb_dir),
        Some(&installation),
        0,
        &user_name(0),
        fake_path_owner,
    );

    assert_eq!(summary["user_id"], 0);
    assert_eq!(summary["user_name"], "root");
    assert_eq!(summary["root_runtime"], true);
    assert_eq!(summary["install_root_owned"], true);
    assert_eq!(summary["project_owner"], "1000:demo");
    assert_eq!(summary["ccb_dir_owner"], "1000:demo");
    assert_eq!(summary["install_owner"], "0:root");
    assert_eq!(summary["sudo_user"], "demo");

    let warnings = summary["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0],
        "Running CCB as root in a non-root-owned project can create root-owned .ccb files."
    );
}

#[test]
fn test_render_doctor_includes_root_runtime_identity_lines() {
    let payload = serde_json::json!({
        "project": "/tmp/repo",
        "project_id": "proj-1",
        "installation": {
            "path": "/tmp/install",
            "install_mode": "release",
            "source_kind": "release",
            "version": "7.2.1",
            "channel": "stable",
            "build_time": "2026-06-03T00:00:00Z",
            "platform": "linux",
            "arch": "x86_64",
        },
        "runtime": {
            "user_id": 0,
            "user_name": "root",
            "home": "/root",
            "root_runtime": true,
            "install_root_owned": true,
            "install_user_id": 0,
            "install_user_name": "root",
            "sudo_user": "demo",
            "project_owner": "1000:demo",
            "ccb_dir_owner": "1000:demo",
            "install_owner": "0:root",
            "warnings": [
                "Running CCB as root in a non-root-owned project can create root-owned .ccb files.",
            ],
        },
        "requirements": {
            "python_executable": "/usr/bin/python3",
            "python_version": "3.12.0",
            "tmux_available": true,
            "tmux_path": "/usr/bin/tmux",
            "provider_commands": [],
        },
        "ccbd": {
            "state": "unmounted",
            "health": "unknown",
            "generation": 0,
            "last_heartbeat_at": null,
            "pid_alive": false,
            "socket_connectable": false,
            "heartbeat_fresh": false,
            "takeover_allowed": true,
            "reason": "not_started",
            "active_execution_count": 0,
            "recoverable_execution_count": 0,
            "nonrecoverable_execution_count": 0,
            "pending_items_count": 0,
            "terminal_pending_count": 0,
            "recoverable_execution_providers": [],
            "nonrecoverable_execution_providers": [],
            "diagnostic_errors": [],
        },
        "agents": [],
    });

    let lines = render_doctor(&payload);

    assert!(lines.contains(&"user_id: 0".to_string()));
    assert!(lines.contains(&"user_name: root".to_string()));
    assert!(lines.contains(&"home: /root".to_string()));
    assert!(lines.contains(&"root_runtime: true".to_string()));
    assert!(lines.contains(&"install_root_owned: true".to_string()));
    assert!(lines.contains(&"sudo_user: demo".to_string()));
    assert!(lines.contains(&"project_owner: 1000:demo".to_string()));
    assert!(lines.contains(&"ccb_dir_owner: 1000:demo".to_string()));
    assert!(lines.contains(
        &"runtime_warning: Running CCB as root in a non-root-owned project can create root-owned .ccb files.".to_string()
    ));
}
