use ccb_provider_sessions::files::{
    check_session_writable, find_project_session_file, project_config_dir, safe_write_session,
    ProviderClientSpec,
};
use ccb_provider_sessions::resolution::{resolve_work_dir, resolve_work_dir_with_registry};
use ccb_provider_sessions::watch::{is_watch_file, SessionFileWatcher};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use tempfile::TempDir;

fn is_root() -> bool {
    unsafe { libc::getuid() == 0 }
}

#[test]
fn test_project_config_dir() {
    let dir = TempDir::new().unwrap();
    let expected = dir.path().canonicalize().unwrap().join(".ccb");
    assert_eq!(project_config_dir(dir.path()), expected);
}

#[test]
fn test_check_session_writable_new_file() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("session.json");
    let check = check_session_writable(&file);
    assert!(check.writable);
    assert!(check.reason.is_none());
    assert!(check.fix.is_none());
}

#[test]
fn test_check_session_writable_missing_parent() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("missing/dir/session.json");
    let check = check_session_writable(&file);
    assert!(!check.writable);
    assert!(check
        .reason
        .as_ref()
        .unwrap()
        .contains("Directory not found"));
    assert!(check.fix.as_ref().unwrap().starts_with("mkdir -p"));
}

#[test]
fn test_check_session_writable_symlink() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("real.json");
    fs::write(&target, "").unwrap();
    let link = dir.path().join("session.json");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &link).unwrap();
    #[cfg(not(unix))]
    std::os::windows::fs::symlink_file(&target, &link).unwrap();

    let check = check_session_writable(&link);
    assert!(!check.writable);
    assert!(check.reason.as_ref().unwrap().contains("symlink"));
}

#[test]
fn test_check_session_writable_directory() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("session.json");
    fs::create_dir(&file).unwrap();
    let check = check_session_writable(&file);
    assert!(!check.writable);
    assert!(check.reason.as_ref().unwrap().contains("directory"));
}

#[test]
#[allow(clippy::permissions_set_readonly_false)]
fn test_check_session_writable_readonly_parent() {
    if is_root() {
        return;
    }
    let dir = TempDir::new().unwrap();
    let ro = dir.path().join("readonly");
    fs::create_dir(&ro).unwrap();
    let mut perms = fs::metadata(&ro).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&ro, perms).unwrap();

    let file = ro.join("session.json");
    let check = check_session_writable(&file);

    // Restore permissions so TempDir can clean up.
    let mut perms = fs::metadata(&ro).unwrap().permissions();
    perms.set_readonly(false);
    fs::set_permissions(&ro, perms).unwrap();

    assert!(!check.writable);
    assert!(check.reason.as_ref().unwrap().contains("not writable"));
}

#[test]
fn test_safe_write_session_round_trip() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("session.json");
    safe_write_session(&file, "hello world").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "hello world");
}

#[test]
fn test_safe_write_session_fails_on_symlink() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("real.json");
    fs::write(&target, "").unwrap();
    let link = dir.path().join("session.json");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &link).unwrap();
    #[cfg(not(unix))]
    std::os::windows::fs::symlink_file(&target, &link).unwrap();

    let result = safe_write_session(&link, "data");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Cannot write"));
}

#[test]
fn test_find_project_session_file_via_anchor() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("project");
    let ccb = project.join(".ccb");
    fs::create_dir_all(&ccb).unwrap();
    let session = ccb.join(".claude-session");
    fs::write(&session, "").unwrap();

    let found = find_project_session_file(&project, ".claude-session");
    assert_eq!(found, Some(session));
}

#[test]
fn test_find_project_session_file_via_workspace_binding() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target");
    let target_ccb = target.join(".ccb");
    fs::create_dir_all(&target_ccb).unwrap();
    let session = target_ccb.join(".codex-session");
    fs::write(&session, "").unwrap();

    let workspace = dir.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let binding = workspace.join(".ccb-workspace.json");
    fs::write(
        &binding,
        serde_json::json!({"target_project": target.to_str().unwrap()}).to_string(),
    )
    .unwrap();

    let found = find_project_session_file(&workspace, ".codex-session");
    assert_eq!(found, Some(session));
}

#[test]
fn test_resolve_work_dir_with_explicit_session_file() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("project");
    let ccb = project.join(".ccb");
    fs::create_dir_all(&ccb).unwrap();
    let session = ccb.join(".claude-session");
    fs::write(&session, "").unwrap();

    let spec = ProviderClientSpec::new("claude", ".claude-session");
    let (work, session_opt) = resolve_work_dir(
        &spec,
        Some(session.to_str().unwrap()),
        None,
        Some(dir.path()),
    )
    .unwrap();
    assert_eq!(work, project.canonicalize().unwrap());
    assert_eq!(session_opt, Some(session.canonicalize().unwrap()));
}

#[test]
fn test_resolve_work_dir_without_session_file() {
    let dir = TempDir::new().unwrap();
    let cwd = dir.path().canonicalize().unwrap();
    let spec = ProviderClientSpec::new("claude", ".claude-session");
    let (work, session_opt) = resolve_work_dir(&spec, None, None, Some(&cwd)).unwrap();
    assert_eq!(work, cwd);
    assert!(session_opt.is_none());
}

#[test]
fn test_resolve_work_dir_with_registry_finds_project_file() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("project");
    let ccb = project.join(".ccb");
    fs::create_dir_all(&ccb).unwrap();
    let session = ccb.join(".droid-session");
    fs::write(&session, "").unwrap();

    let spec = ProviderClientSpec::new("droid", ".droid-session");
    let (work, session_opt) = resolve_work_dir_with_registry(
        &spec,
        "droid",
        None,
        None,
        Some(&project),
        "CCB_REGISTRY_ONLY_TEST",
    )
    .unwrap();
    assert_eq!(work, project.canonicalize().unwrap());
    assert_eq!(session_opt, Some(session.canonicalize().unwrap()));
}

#[test]
fn test_resolve_work_dir_with_registry_only_env() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    let env_key = "CCB_REGISTRY_ONLY_INTEGRATION";
    std::env::set_var(env_key, "1");

    let spec = ProviderClientSpec::new("agy", ".agy-session");
    let result = resolve_work_dir_with_registry(&spec, "agy", None, None, Some(&project), env_key);
    std::env::remove_var(env_key);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("is no longer supported"));
}

fn watcher_for(dir: &TempDir, recursive: bool) -> (SessionFileWatcher, Receiver<PathBuf>) {
    let (tx, rx) = channel();
    let watcher = SessionFileWatcher::new(
        dir.path().to_path_buf(),
        move |path| {
            let _ = tx.send(path);
        },
        recursive,
    );
    (watcher, rx)
}

#[test]
fn test_watcher_detects_new_log_file() {
    let dir = TempDir::new().unwrap();
    let (mut watcher, rx) = watcher_for(&dir, false);
    watcher.start();
    std::thread::sleep(Duration::from_millis(150));

    let file = dir.path().join("agent.jsonl");
    fs::write(&file, "line\n").unwrap();

    let event = rx.recv_timeout(Duration::from_millis(1000)).unwrap();
    assert_eq!(event, file);
    watcher.stop();
}

#[test]
fn test_watcher_detects_index_file() {
    let dir = TempDir::new().unwrap();
    let (mut watcher, rx) = watcher_for(&dir, false);
    watcher.start();
    std::thread::sleep(Duration::from_millis(150));

    let file = dir.path().join("sessions-index.json");
    fs::write(&file, "{}").unwrap();

    let event = rx.recv_timeout(Duration::from_millis(1000)).unwrap();
    assert_eq!(event, file);
    watcher.stop();
}

#[test]
fn test_watcher_ignores_hidden_files() {
    let dir = TempDir::new().unwrap();
    let (mut watcher, rx) = watcher_for(&dir, false);
    watcher.start();
    std::thread::sleep(Duration::from_millis(150));

    let file = dir.path().join(".hidden.jsonl");
    fs::write(&file, "line\n").unwrap();

    let result = rx.recv_timeout(Duration::from_millis(300));
    assert!(result.is_err());
    watcher.stop();
}

#[test]
fn test_watcher_recursive() {
    let dir = TempDir::new().unwrap();
    let nested = dir.path().join("nested");
    fs::create_dir(&nested).unwrap();
    let (mut watcher, rx) = watcher_for(&dir, true);
    watcher.start();
    std::thread::sleep(Duration::from_millis(150));

    let file = nested.join("deep.jsonl");
    fs::write(&file, "line\n").unwrap();

    let event = rx.recv_timeout(Duration::from_millis(1000)).unwrap();
    assert_eq!(event, file);
    watcher.stop();
}

#[test]
fn test_is_watch_file_predicate() {
    assert!(is_watch_file(Path::new("agent.jsonl")));
    assert!(is_watch_file(Path::new("sessions-index.json")));
    assert!(!is_watch_file(Path::new(".hidden.jsonl")));
    assert!(!is_watch_file(Path::new("agent.json")));
}

#[test]
fn test_crate_root_reexports_reachable() {
    use std::path::Path;

    let _: &str = ccb_provider_sessions::CCB_PROJECT_CONFIG_DIRNAME;
    let _: bool = ccb_provider_sessions::HAS_WATCHDOG;

    let _: fn(&str, bool) = ccb_provider_sessions::print_session_error;
    let _: fn(&str) = ccb_provider_sessions::print_session_error_stderr;

    let dir = TempDir::new().unwrap();
    let session = dir.path().join("session.json");

    let _ = ccb_provider_sessions::check_session_writable(&session);
    let _ = ccb_provider_sessions::find_project_session_file(dir.path(), ".claude-session");
    let _ = ccb_provider_sessions::project_config_dir(dir.path());
    let _ = ccb_provider_sessions::resolve_project_config_dir(dir.path());
    let _ = ccb_provider_sessions::safe_write_session(&session, "hello");

    let spec = ccb_provider_sessions::files::ProviderClientSpec::new("claude", ".claude-session");
    let _ = ccb_provider_sessions::resolve_work_dir(&spec, None, None, None);
    let _ = ccb_provider_sessions::resolve_work_dir_with_registry(
        &spec,
        "claude",
        None,
        None,
        None,
        "CCB_REGISTRY_ONLY_REEXPORT_TEST",
    );

    let _watcher = ccb_provider_sessions::SessionFileWatcher::new(
        dir.path().to_path_buf(),
        |_path: PathBuf| {},
        false,
    );
    let _watcher = ccb_provider_sessions::SessionFileWatcher::with_predicate(
        dir.path().to_path_buf(),
        |_path: PathBuf| {},
        false,
        Some(|_path: &Path| true),
    );
    let _watcher = ccb_provider_sessions::SessionFileWatcher::new_with_predicate(
        dir.path().to_path_buf(),
        |_path: PathBuf| {},
        false,
        Some(|_path: &Path| true),
    );
}
