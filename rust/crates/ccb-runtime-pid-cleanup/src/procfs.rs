//! /proc filesystem helpers for PID cleanup.
//!
//! Mirrors Python `runtime_pid_cleanup.procfs`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::utils::coerce_pid;

/// Read a `.pid` file and coerce its contents to a PID.
///
/// Mirrors Python `runtime_pid_cleanup.procfs.read_pid_file`.
pub fn read_pid_file(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    coerce_pid(text)
}

/// Read a `/proc/{pid}/{entry}` symlink and return its target.
///
/// Mirrors Python `runtime_pid_cleanup.procfs.read_proc_path`.
pub fn read_proc_path(pid: u32, entry: &str) -> Option<PathBuf> {
    let target = fs::read_link(format!("/proc/{pid}/{entry}")).ok()?;
    Some(target)
}

/// Read the command line for a PID, replacing NUL bytes with spaces.
///
/// Mirrors Python `runtime_pid_cleanup.procfs.read_proc_cmdline`.
pub fn read_proc_cmdline(pid: u32) -> String {
    let raw = match fs::read(format!("/proc/{pid}/cmdline")) {
        Ok(data) => data,
        Err(_) => return String::new(),
    };
    raw.split(|&b| b == 0)
        .map(|slice| String::from_utf8_lossy(slice).to_string())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Remove files ending with the `.pid` suffix.
///
/// Mirrors Python `runtime_pid_cleanup.procfs.remove_pid_files`.
pub fn remove_pid_files(paths: &[PathBuf]) {
    for path in paths {
        if path.extension().and_then(|e| e.to_str()) != Some("pid") {
            continue;
        }
        if let Err(e) = fs::remove_file(path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                // Ignore other errors, matching Python's broad except Exception.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn read_pid_file_reads_valid_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.pid");
        fs::write(&path, "12345\n").unwrap();
        assert_eq!(read_pid_file(&path), Some(12345));
    }

    #[test]
    fn read_pid_file_returns_none_for_invalid_content() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.pid");
        fs::write(&path, "not-a-pid").unwrap();
        assert_eq!(read_pid_file(&path), None);
    }

    #[test]
    fn read_pid_file_returns_none_for_missing_file() {
        let path = PathBuf::from("/nonexistent/path/file.pid");
        assert_eq!(read_pid_file(&path), None);
    }

    #[test]
    fn read_proc_cmdline_for_current_process() {
        let current = std::process::id();
        let cmdline = read_proc_cmdline(current);
        // Command line should be non-empty on Linux.
        assert!(!cmdline.is_empty());
    }

    #[test]
    fn read_proc_path_for_current_process_cwd() {
        let current = std::process::id();
        let cwd = read_proc_path(current, "cwd");
        assert!(cwd.is_some());
    }

    #[test]
    fn remove_pid_files_deletes_only_pid_files() {
        let tmp = tempfile::tempdir().unwrap();
        let pid_path = tmp.path().join("test.pid");
        let other_path = tmp.path().join("test.json");
        fs::write(&pid_path, "42").unwrap();
        fs::write(&other_path, "{}").unwrap();

        remove_pid_files(&[pid_path.clone(), other_path.clone()]);

        assert!(!pid_path.exists());
        assert!(other_path.exists());
    }
}
