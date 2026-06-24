//! Mirrors Python `lib/ccbd/start_runtime/layout.py`.

use std::path::Path;

/// Build the bootstrap command used for the CCB command pane.
///
/// Mirrors Python `cmd_bootstrap_command()`.
pub fn cmd_bootstrap_command() -> String {
    let shell = resolved_cmd_shell();
    let flags = cmd_shell_login_flags(&shell);
    let mut argv = vec!["exec".to_string(), shell];
    argv.extend(flags);
    argv.into_iter()
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn resolved_cmd_shell() -> String {
    let mut seen = std::collections::HashSet::new();
    for candidate in [
        std::env::var("CCBR_CMD_SHELL").unwrap_or_default(),
        std::env::var("SHELL").unwrap_or_default(),
        passwd_login_shell(),
        ccbr_terminal::env::default_shell().0,
        "bash".to_string(),
        "sh".to_string(),
    ] {
        let normalized = candidate.trim();
        if normalized.is_empty() || seen.contains(normalized) {
            continue;
        }
        seen.insert(normalized.to_string());
        if let Some(resolved) = resolve_shell_candidate(normalized) {
            return resolved;
        }
    }
    "sh".to_string()
}

fn resolve_shell_candidate(candidate: &str) -> Option<String> {
    if candidate.contains('/') {
        let path = Path::new(candidate);
        if path.exists() {
            return Some(candidate.to_string());
        }
        return None;
    }
    find_in_path(candidate)
}

fn find_in_path(name: &str) -> Option<String> {
    std::env::var("PATH").ok().and_then(|path| {
        path.split(':')
            .map(|dir| Path::new(dir).join(name))
            .find(|p| p.exists())
            .map(|p| p.to_string_lossy().to_string())
    })
}

fn passwd_login_shell() -> String {
    // Mirroring Python: if the `pwd` module is unavailable, return empty.
    // On non-Unix targets we do not have `libc::getpwuid`, so return empty.
    #[cfg(unix)]
    {
        // Safety: `getpwuid` returns a pointer to static/thread-local data.
        let entry = unsafe { libc::getpwuid(libc::getuid()) };
        if entry.is_null() {
            return String::new();
        }
        // Safety: `pw_shell` is a valid C string when `entry` is non-null.
        let shell = unsafe {
            std::ffi::CStr::from_ptr((*entry).pw_shell)
                .to_string_lossy()
                .trim()
                .to_string()
        };
        shell
    }
    #[cfg(not(unix))]
    {
        String::new()
    }
}

fn cmd_shell_login_flags(shell: &str) -> Vec<String> {
    let shell_name = Path::new(shell)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ["bash", "dash", "fish", "ksh", "sh", "zsh"].contains(&shell_name.as_str()) {
        vec!["-l".to_string()]
    } else {
        Vec::new()
    }
}

fn shell_quote(part: String) -> String {
    if part.is_empty()
        || part
            .chars()
            .any(|c| c.is_whitespace() || c == '\'' || c == '"' || c == '\\')
    {
        let escaped = part.replace('\'', "'\"'\"'");
        format!("'{}'", escaped)
    } else {
        part
    }
}

#[cfg(test)]
mod tests {
    use super::cmd_bootstrap_command;
    use std::io::Write;
    use std::path::PathBuf;

    fn with_shell(name: &str, test: impl FnOnce(&PathBuf)) {
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
}
