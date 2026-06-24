//! Mirrors Python `test/test_cleanup_service.py`.

use ccb_cli::context::{CliContext, CliContextBuilder};
use ccb_cli::models::{ParsedCommand, ParsedDoctorCommand};
use ccb_cli::services::cleanup::{
    cleanup_project_storage_with, CleanupSummary, DaemonInspection, DaemonInspector,
};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn stopped() -> impl DaemonInspector {
    struct Stopped;
    impl DaemonInspector for Stopped {
        fn inspect_daemon(&self, _context: &CliContext) -> anyhow::Result<DaemonInspection> {
            Ok(DaemonInspection::default())
        }
    }
    Stopped
}

fn active() -> impl DaemonInspector {
    struct Active;
    impl DaemonInspector for Active {
        fn inspect_daemon(&self, _context: &CliContext) -> anyhow::Result<DaemonInspection> {
            Ok(DaemonInspection {
                pid_alive: true,
                socket_connectable: true,
                phase: "running".into(),
                desired_state: "running".into(),
                mounted: true,
            })
        }
    }
    Active
}

fn make_context(project_root: &std::path::Path) -> CliContext {
    let ccb = project_root.join(".ccb");
    fs::create_dir_all(&ccb).unwrap();
    fs::write(ccb.join("ccb.config"), "demo:codex\n").unwrap();
    CliContextBuilder::new(ParsedCommand::Doctor(ParsedDoctorCommand {
        project: None,
        bundle: false,
        output_path: None,
        storage: false,
        json_output: false,
        kind: "doctor".into(),
    }))
    .cwd(project_root.to_path_buf())
    .build()
    .unwrap()
}

fn claude_home(context: &CliContext, agent: &str) -> PathBuf {
    PathBuf::from(
        context
            .paths
            .agent_provider_state_dir(agent, "claude")
            .as_str(),
    )
    .join("home")
}

fn gemini_home(context: &CliContext, agent: &str) -> PathBuf {
    PathBuf::from(
        context
            .paths
            .agent_provider_state_dir(agent, "gemini")
            .as_str(),
    )
    .join("home")
}

fn action_paths(summary: &CleanupSummary) -> HashSet<&str> {
    summary.actions.iter().map(|a| a.path.as_str()).collect()
}

#[test]
fn test_cleanup_prunes_old_claude_versions_and_gemini_caches() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(tmp.path());

    let home = claude_home(&context, "demo");
    let versions = home.join(".local/share/claude/versions");
    fs::create_dir_all(&versions).unwrap();
    for v in &["0.9.0", "0.9.1", "0.9.2"] {
        fs::create_dir(versions.join(v)).unwrap();
    }
    let bin_dir = home.join(".local/bin");
    fs::create_dir_all(&bin_dir).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(versions.join("0.9.2"), bin_dir.join("claude")).unwrap();
    #[cfg(not(unix))]
    {
        let _ = bin_dir.join("claude");
    }

    let gem_home = gemini_home(&context, "demo");
    fs::create_dir_all(gem_home.join(".npm/_cacache")).unwrap();
    fs::create_dir_all(gem_home.join(".cache/node-gyp")).unwrap();
    fs::create_dir_all(gem_home.join(".gemini/tmp")).unwrap();
    fs::write(gem_home.join(".gemini/tmp/session.json"), "{}").unwrap();

    let summary =
        cleanup_project_storage_with(&context, &serde_json::Value::Null, &stopped()).unwrap();
    assert_eq!(summary.status, "ok");
    assert_eq!(summary.deleted_count, 3, "actions: {:?}", summary.actions);
    let paths = action_paths(&summary);
    assert!(paths.contains(versions.join("0.9.0").to_str().unwrap()));
    assert!(!paths.contains(versions.join("0.9.1").to_str().unwrap()));
    assert!(!paths.contains(versions.join("0.9.2").to_str().unwrap()));
    assert!(paths.contains(gem_home.join(".npm/_cacache").to_str().unwrap()));
    assert!(paths.contains(gem_home.join(".cache/node-gyp").to_str().unwrap()));
    assert!(gem_home.join(".gemini/tmp/session.json").exists());
}

#[test]
fn test_cleanup_refuses_when_pending_jobs_exist() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(tmp.path());

    let jobs_path = PathBuf::from(context.paths.agent_jobs_path("demo").as_str());
    fs::create_dir_all(jobs_path.parent().unwrap()).unwrap();
    let mut file = fs::File::create(&jobs_path).unwrap();
    writeln!(file, r#"{{"job_id":"job_1","status":"accepted"}}"#).unwrap();

    let err =
        cleanup_project_storage_with(&context, &serde_json::Value::Null, &stopped()).unwrap_err();
    assert!(err.to_string().contains("pending ask jobs exist"));
}

#[test]
fn test_cleanup_refuses_when_jobs_jsonl_is_malformed() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(tmp.path());

    let jobs_path = PathBuf::from(context.paths.agent_jobs_path("demo").as_str());
    fs::create_dir_all(jobs_path.parent().unwrap()).unwrap();
    fs::write(&jobs_path, "{this is not json").unwrap();

    let err =
        cleanup_project_storage_with(&context, &serde_json::Value::Null, &stopped()).unwrap_err();
    assert!(err.to_string().contains("pending ask jobs exist"));
}

#[test]
fn test_cleanup_refuses_when_ccbd_is_active() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(tmp.path());

    let err =
        cleanup_project_storage_with(&context, &serde_json::Value::Null, &active()).unwrap_err();
    assert!(err.to_string().contains("requires stopped ccbd"));
}

#[cfg(unix)]
#[test]
fn test_cleanup_reports_symlinked_claude_versions_dir() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(tmp.path());

    let home = claude_home(&context, "demo");
    let versions = home.join(".local/share/claude/versions");
    let real = tmp.path().join("real-versions");
    fs::create_dir_all(&real).unwrap();
    fs::create_dir_all(versions.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&real, &versions).unwrap();

    let summary =
        cleanup_project_storage_with(&context, &serde_json::Value::Null, &stopped()).unwrap();
    assert_eq!(summary.deleted_count, 0);
    assert_eq!(summary.skipped_count, 1);
    assert_eq!(summary.skipped[0].reason, "versions_dir_is_symlink");
}

#[cfg(unix)]
#[test]
fn test_cleanup_prunes_shared_claude_versions_referenced_by_symlinked_agent_home() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(tmp.path());

    let shared_versions = PathBuf::from(
        context
            .paths
            .shared_cache_dir()
            .join("claude")
            .join("versions")
            .as_str(),
    );
    fs::create_dir_all(&shared_versions).unwrap();
    for v in &["0.9.0", "0.9.1", "0.9.2"] {
        fs::create_dir(shared_versions.join(v)).unwrap();
    }

    let home = claude_home(&context, "demo");
    let local_versions = home.join(".local/share/claude/versions");
    fs::create_dir_all(local_versions.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&shared_versions, &local_versions).unwrap();

    let bin_dir = home.join(".local/bin");
    fs::create_dir_all(&bin_dir).unwrap();
    std::os::unix::fs::symlink(shared_versions.join("0.9.2"), bin_dir.join("claude")).unwrap();

    let summary =
        cleanup_project_storage_with(&context, &serde_json::Value::Null, &stopped()).unwrap();
    assert_eq!(summary.status, "ok");
    assert_eq!(summary.deleted_count, 1, "actions: {:?}", summary.actions);
    assert_eq!(summary.skipped_count, 1, "skipped: {:?}", summary.skipped);
    assert!(summary
        .skipped
        .iter()
        .any(|s| s.reason == "versions_dir_is_symlink"));
    assert!(action_paths(&summary).contains(shared_versions.join("0.9.0").to_str().unwrap()));
    assert!(shared_versions.join("0.9.1").exists());
    assert!(shared_versions.join("0.9.2").exists());
    assert!(summary
        .actions
        .iter()
        .any(|a| a.reason == "old_shared_claude_version_cache"));
}

#[cfg(unix)]
#[test]
fn test_cleanup_prunes_external_claude_versions_referenced_by_agent_home() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(tmp.path());

    let external_versions = PathBuf::from(
        context
            .paths
            .provider_external_cache_dir("claude")
            .unwrap()
            .join("versions")
            .as_str(),
    );
    fs::create_dir_all(&external_versions).unwrap();
    for v in &["0.9.0", "0.9.1", "0.9.2"] {
        fs::create_dir(external_versions.join(v)).unwrap();
    }

    let home = claude_home(&context, "demo");
    let local_versions = home.join(".local/share/claude/versions");
    fs::create_dir_all(local_versions.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&external_versions, &local_versions).unwrap();

    let bin_dir = home.join(".local/bin");
    fs::create_dir_all(&bin_dir).unwrap();
    std::os::unix::fs::symlink(external_versions.join("0.9.2"), bin_dir.join("claude")).unwrap();

    let summary =
        cleanup_project_storage_with(&context, &serde_json::Value::Null, &stopped()).unwrap();
    assert_eq!(summary.deleted_count, 1, "actions: {:?}", summary.actions);
    assert!(action_paths(&summary).contains(external_versions.join("0.9.0").to_str().unwrap()));
    assert!(summary
        .actions
        .iter()
        .any(|a| a.reason == "old_shared_claude_version_cache"));
}

#[cfg(unix)]
#[test]
fn test_cleanup_removes_legacy_shared_claude_versions_after_external_migration() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(tmp.path());

    let legacy_shared = PathBuf::from(
        context
            .paths
            .shared_cache_dir()
            .join("claude")
            .join("versions")
            .as_str(),
    );
    fs::create_dir_all(&legacy_shared).unwrap();
    fs::create_dir(legacy_shared.join("0.9.2")).unwrap();

    let external_versions = PathBuf::from(
        context
            .paths
            .provider_external_cache_dir("claude")
            .unwrap()
            .join("versions")
            .as_str(),
    );
    fs::create_dir_all(&external_versions).unwrap();
    fs::create_dir(external_versions.join("0.9.2")).unwrap();

    let home = claude_home(&context, "demo");
    let local_versions = home.join(".local/share/claude/versions");
    fs::create_dir_all(local_versions.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&external_versions, &local_versions).unwrap();

    let bin_dir = home.join(".local/bin");
    fs::create_dir_all(&bin_dir).unwrap();
    std::os::unix::fs::symlink(external_versions.join("0.9.2"), bin_dir.join("claude")).unwrap();

    let summary =
        cleanup_project_storage_with(&context, &serde_json::Value::Null, &stopped()).unwrap();
    assert_eq!(summary.deleted_count, 1, "actions: {:?}", summary.actions);
    assert!(action_paths(&summary).contains(legacy_shared.join("0.9.2").to_str().unwrap()));
    assert!(summary
        .actions
        .iter()
        .any(|a| a.reason == "unreferenced_shared_claude_version_cache"));
}

#[test]
fn test_cleanup_removes_claude_rebuildable_caches() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(tmp.path());

    let home = claude_home(&context, "demo");
    let caches = [
        home.join(".cache/claude"),
        home.join(".npm/_logs"),
        home.join(".claude/cache"),
        home.join(".claude/telemetry"),
        home.join(".claude/paste-cache"),
        home.join(".claude/plugins/marketplaces"),
    ];
    for c in &caches {
        fs::create_dir_all(c).unwrap();
    }
    fs::create_dir_all(home.join(".claude/projects")).unwrap();
    fs::write(home.join(".claude/projects/session.jsonl"), "{}\n").unwrap();

    let summary =
        cleanup_project_storage_with(&context, &serde_json::Value::Null, &stopped()).unwrap();
    assert_eq!(summary.deleted_count, 6, "actions: {:?}", summary.actions);
    assert!(home.join(".claude/projects/session.jsonl").exists());
}

#[cfg(unix)]
#[test]
fn test_cleanup_skips_gemini_cache_behind_out_of_bounds_symlink() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(tmp.path());

    let home = gemini_home(&context, "demo");
    fs::create_dir_all(&home).unwrap();
    let real_npm = tmp.path().join("outside-npm");
    fs::create_dir_all(real_npm.join("_cacache")).unwrap();
    std::os::unix::fs::symlink(&real_npm, home.join(".npm")).unwrap();

    let summary =
        cleanup_project_storage_with(&context, &serde_json::Value::Null, &stopped()).unwrap();
    assert_eq!(summary.deleted_count, 0);
    assert!(summary
        .skipped
        .iter()
        .any(|s| s.reason == "path_out_of_bounds"));
}

#[test]
fn test_cleanup_removes_gemini_shared_and_external_rebuildable_caches() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(tmp.path());

    let shared = PathBuf::from(context.paths.shared_cache_dir().join("gemini").as_str());
    fs::create_dir_all(shared.join("npm/_cacache")).unwrap();
    fs::create_dir_all(shared.join("xdg/node-gyp")).unwrap();

    let external = PathBuf::from(
        context
            .paths
            .provider_external_cache_dir("gemini")
            .unwrap()
            .as_str(),
    );
    fs::create_dir_all(external.join("npm/_cacache")).unwrap();
    fs::create_dir_all(external.join("xdg/node-gyp")).unwrap();

    let summary =
        cleanup_project_storage_with(&context, &serde_json::Value::Null, &stopped()).unwrap();
    assert_eq!(summary.deleted_count, 4, "actions: {:?}", summary.actions);
}

#[test]
fn test_cleanup_trims_pane_crash_logs_by_runtime_count() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(tmp.path());

    let runtime_dir = PathBuf::from(
        context
            .paths
            .agent_provider_runtime_dir("demo", "claude")
            .as_str(),
    );
    fs::create_dir_all(&runtime_dir).unwrap();
    for i in 0..55 {
        fs::write(
            runtime_dir.join(format!("pane-crash-{:03}.log", i)),
            "crash\n",
        )
        .unwrap();
    }

    let summary =
        cleanup_project_storage_with(&context, &serde_json::Value::Null, &stopped()).unwrap();
    assert_eq!(summary.deleted_count, 5, "actions: {:?}", summary.actions);
    assert!(runtime_dir.join("pane-crash-054.log").exists());
    assert!(!runtime_dir.join("pane-crash-000.log").exists());
}
