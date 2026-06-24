//! Mirrors Python `test/test_ccbd_start_runtime_layout.py`.

use ccbr_daemon::start_runtime_layout::cmd_bootstrap_command;
use std::io::Write;

fn with_shell(name: &str, test: impl FnOnce(&std::path::PathBuf)) {
    let tmp = tempfile::tempdir().unwrap();
    let shell_path = tmp.path().join(name);
    {
        let mut f = std::fs::File::create(&shell_path).unwrap();
        f.write_all(b"#!/bin/sh\n").unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shell_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let old_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", tmp.path().display(), old_path));
    std::env::set_var("SHELL", name);
    test(&shell_path);
    std::env::set_var("PATH", old_path);
    std::env::remove_var("SHELL");
}

#[test]
fn test_cmd_bootstrap_command_uses_user_zsh_directly() {
    with_shell("zsh", |shell_path| {
        assert_eq!(
            cmd_bootstrap_command(),
            format!("exec {} -l", shell_path.display())
        );
    });
}

#[test]
fn test_cmd_bootstrap_command_is_shell_language_agnostic_for_fish() {
    with_shell("fish", |shell_path| {
        assert_eq!(
            cmd_bootstrap_command(),
            format!("exec {} -l", shell_path.display())
        );
    });
}
