//! Mirrors Python `test/test_v2_kill_service.py` orchestration subset.

use ccbr_cli::context::{CliContext, CliContextBuilder};
use ccbr_cli::models::{ParsedCommand, ParsedKillCommand};
use ccbr_cli::services::daemon_runtime::models::KillSummary;
use ccbr_cli::services::kill::kill_project_with;
use ccbr_cli::services::kill::WorktreeGuardSummary;
use ccbr_cli::services::kill_runtime::agent_cleanup::KillPreparation;
use ccbr_cli::services::kill_runtime::reporting::record_kill_report;
use ccbr_cli::services::tmux_project_cleanup_runtime::models::ProjectTmuxCleanupSummary;
use std::collections::HashMap;

fn make_context(tmp: &tempfile::TempDir) -> CliContext {
    let project_root = tmp.path();
    std::fs::create_dir_all(project_root.join(".ccbr")).unwrap();
    std::fs::write(project_root.join(".ccbr/ccbr.config"), "demo:codex\n").unwrap();
    CliContextBuilder::new(ParsedCommand::Kill(ParsedKillCommand {
        project: None,
        force: false,
        kind: "kill".into(),
    }))
    .cwd(project_root.to_path_buf())
    .build()
    .unwrap()
}

fn sample_preparation() -> KillPreparation {
    KillPreparation {
        configured_agent_names: vec!["demo".into()],
        extra_agent_names: Vec::new(),
        tmux_sockets: vec![Some("/tmp/ccb.sock".into())],
        pid_candidates: HashMap::new(),
        control_plane_pids: Vec::new(),
    }
}

fn sample_summary() -> KillSummary {
    KillSummary {
        project_id: "proj".into(),
        state: "unmounted".into(),
        socket_path: "/tmp/sock".into(),
        forced: false,
        cleanup_summaries: Vec::new(),
        worktree_warnings: Vec::new(),
    }
}

#[test]
fn test_kill_project_orchestrates_all_stages() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    let project_id = context.project.project_id.clone();
    let command = ParsedKillCommand {
        project: None,
        force: false,
        kind: "kill".into(),
    };

    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let calls_clone = calls.clone();

    let result = kill_project_with(
        &context,
        &command,
        |_ctx, reason| {
            calls.lock().unwrap().push(format!("maintenance:{reason}"));
        },
        |_project_root| {
            calls.lock().unwrap().push("authority".into());
            HashMap::new()
        },
        |_ctx, force| {
            calls.lock().unwrap().push(format!("remote:{force}"));
            Ok(None)
        },
        |_paths, force, control_plane| {
            calls
                .lock()
                .unwrap()
                .push(format!("prepare:{force}:{:?}", control_plane.is_some()));
            Ok(sample_preparation())
        },
        |_paths, pid, force| {
            calls.lock().unwrap().push(format!("destroy:{pid}:{force}"));
            Ok(())
        },
        |_ctx, remote, force, _preparation| {
            calls
                .lock()
                .unwrap()
                .push(format!("resolve:{force}:{}", remote.is_some()));
            Ok(sample_summary())
        },
        |_paths, pid, force, _preparation, _remote, _summary| {
            calls
                .lock()
                .unwrap()
                .push(format!("finalize:{pid}:{force}"));
            Ok(sample_summary())
        },
        |_project_root, _workspaces_dir| {
            calls.lock().unwrap().push("prune".into());
        },
        |_project_root| WorktreeGuardSummary {
            warnings: Vec::new(),
        },
    )
    .unwrap();

    assert_eq!(
        *calls_clone.lock().unwrap(),
        vec![
            format!("maintenance:kill"),
            "authority".into(),
            "remote:false".into(),
            "prepare:false:true".into(),
            format!("destroy:{project_id}:false"),
            "resolve:false:false".into(),
            format!("finalize:{project_id}:false"),
        ]
    );
    assert_eq!(result.state, "unmounted");
}

#[test]
fn test_kill_project_prunes_worktrees_only_when_forced() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    let command = ParsedKillCommand {
        project: None,
        force: true,
        kind: "kill".into(),
    };

    let pruned = std::sync::Arc::new(std::sync::Mutex::new(false));
    let pruned_clone = pruned.clone();

    kill_project_with(
        &context,
        &command,
        |_ctx, _reason| {},
        |_project_root| HashMap::new(),
        |_ctx, _force| Ok(None),
        |_paths, _force, _control_plane| Ok(sample_preparation()),
        |_paths, _project_id, _force| Ok(()),
        |_ctx, _remote, _force, _preparation| Ok(sample_summary()),
        |_paths, _project_id, _force, _preparation, _remote, _summary| Ok(sample_summary()),
        |_project_root, _workspaces_dir| {
            *pruned.lock().unwrap() = true;
        },
        |_project_root| WorktreeGuardSummary {
            warnings: Vec::new(),
        },
    )
    .unwrap();

    assert!(*pruned_clone.lock().unwrap());
}

#[test]
fn test_kill_project_attaches_worktree_warnings() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    let command = ParsedKillCommand {
        project: None,
        force: false,
        kind: "kill".into(),
    };

    let result = kill_project_with(
        &context,
        &command,
        |_ctx, _reason| {},
        |_project_root| HashMap::new(),
        |_ctx, _force| Ok(None),
        |_paths, _force, _control_plane| Ok(sample_preparation()),
        |_paths, _project_id, _force| Ok(()),
        |_ctx, _remote, _force, _preparation| Ok(sample_summary()),
        |_paths, _project_id, _force, _preparation, _remote, _summary| Ok(sample_summary()),
        |_project_root, _workspaces_dir| {},
        |_project_root| WorktreeGuardSummary {
            warnings: vec!["stale worktree".into()],
        },
    )
    .unwrap();

    assert_eq!(result.worktree_warnings.len(), 1);
    assert_eq!(result.worktree_warnings[0], "stale worktree");
}

#[test]
fn test_kill_project_uses_remote_summary_when_present() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    let command = ParsedKillCommand {
        project: None,
        force: false,
        kind: "kill".into(),
    };

    let remote = KillSummary {
        project_id: "remote-proj".into(),
        state: "unmounted".into(),
        socket_path: "/tmp/remote".into(),
        forced: false,
        cleanup_summaries: Vec::new(),
        worktree_warnings: Vec::new(),
    };
    let remote2 = remote.clone();

    let result = kill_project_with(
        &context,
        &command,
        |_ctx, _reason| {},
        |_project_root| HashMap::new(),
        |_ctx, _force| Ok(Some(remote)),
        |_paths, _force, _control_plane| Ok(sample_preparation()),
        |_paths, _project_id, _force| Ok(()),
        |_ctx, _remote, _force, _preparation| Ok(sample_summary()),
        |_paths, _project_id, _force, _preparation, remote, _summary| {
            Ok(KillSummary {
                project_id: remote.map(|r| r.project_id.clone()).unwrap_or_default(),
                ..sample_summary()
            })
        },
        |_project_root, _workspaces_dir| {},
        |_project_root| WorktreeGuardSummary {
            warnings: Vec::new(),
        },
    )
    .unwrap();

    assert_eq!(result.project_id, remote2.project_id);
}

#[test]
fn test_record_kill_report_persists_report_json() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path();
    std::fs::create_dir_all(project_root.join(".ccbr")).unwrap();
    std::fs::write(project_root.join(".ccbr/ccbr.config"), "demo:codex\n").unwrap();

    let paths = ccbr_storage::paths::PathLayout::new(
        camino::Utf8Path::from_path(project_root)
            .unwrap()
            .to_path_buf(),
    );
    let summary = ProjectTmuxCleanupSummary {
        socket_name: Some("/tmp/ccb.sock".into()),
        owned_panes: vec!["%1".into()],
        active_panes: vec!["%2".into()],
        orphaned_panes: vec![],
        killed_panes: vec!["%3".into()],
    };

    record_kill_report(
        &paths,
        "kill",
        true,
        &[summary],
        &["demo".into()],
        &["extra".into()],
    )
    .unwrap();

    let report_path = paths.ccbd_dir().join("kill-report.json");
    assert!(report_path.exists());
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(report_path.as_std_path()).unwrap()).unwrap();
    assert_eq!(report["record_type"], "kill_report");
    assert_eq!(report["trigger"], "kill");
    assert_eq!(report["forced"], true);
    assert_eq!(report["stopped_agents"], serde_json::json![["demo"]]);
    assert_eq!(report["extra_agents"], serde_json::json![["extra"]]);
    assert_eq!(report["cleanup_summary"].as_array().unwrap().len(), 1);
}
