pub mod files;
pub mod resolution;
pub mod watch;

// Re-exports match Python `provider_sessions.__init__.__all__` exactly.
pub use files::{
    check_session_writable, find_project_session_file, print_session_error,
    print_session_error_stderr, project_config_dir, resolve_project_config_dir, safe_write_session,
    CCB_PROJECT_CONFIG_DIRNAME,
};
pub use resolution::{resolve_work_dir, resolve_work_dir_with_registry};
pub use watch::{SessionFileWatcher, HAS_WATCHDOG};

#[cfg(test)]
mod tests {
pub mod discovery;
pub mod pathing;
pub mod writable;
pub mod writing;
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn crate_root_reexports_are_reachable() {
        let _ = CCB_PROJECT_CONFIG_DIRNAME;
        let _ = HAS_WATCHDOG;

        print_session_error("stdout message", false);
        print_session_error_stderr("stderr message");

        let _ = check_session_writable(Path::new("/tmp/nonexistent-session.json"));
        let _ = find_project_session_file(Path::new("/tmp"), ".claude-session");

        let _ = project_config_dir(Path::new("/tmp"));
        let _ = resolve_project_config_dir(Path::new("/tmp"));

        let dir = TempDir::new().unwrap();
        let session = dir.path().join("session.json");
        let _ = safe_write_session(&session, "hello");

        let _watcher = SessionFileWatcher::new(
            dir.path().to_path_buf(),
            |_path: std::path::PathBuf| {},
            false,
        );
        let _watcher2 = SessionFileWatcher::new_with_predicate(
            dir.path().to_path_buf(),
            |_path: std::path::PathBuf| {},
            false,
            Some(|_path: &Path| true),
        );

        let spec = files::ProviderClientSpec::new("claude", ".claude-session");
        let _ = resolve_work_dir(&spec, None, None, None);
        let _ = resolve_work_dir_with_registry(
            &spec,
            "claude",
            None,
            None,
            None,
            "CCB_REGISTRY_ONLY_REEXPORT_TEST",
        );
    }
}
