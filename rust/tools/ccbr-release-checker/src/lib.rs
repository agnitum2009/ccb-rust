use camino::{Utf8Path, Utf8PathBuf};
use std::io::Write;
use std::process::{Command, Output, Stdio};
use wait_timeout::ChildExt;

pub mod assets;
pub mod github;
pub mod local;
pub mod markdown;
pub mod workflows;

pub const EXPECTED_ASSETS: &[&str] = &[
    "ccbr-linux-x86_64.tar.gz",
    "ccbr-macos-universal.tar.gz",
    "SHA256SUMS",
];
pub const CHECKSUMMED_ASSETS: &[&str] =
    &["ccbr-linux-x86_64.tar.gz", "ccbr-macos-universal.tar.gz"];
pub const REQUIRED_TAG_WORKFLOWS: &[&str] = &["Release Artifacts"];
pub const RELEASE_RUN_LIMIT: usize = 50;
pub const BRANCH_VALIDATION_WORKFLOWS: &[&str] = &[
    "Tests",
    "CCBD Real Platform Smoke",
    "Cross-Platform Compatibility Test",
];
pub const DEV_STRICT_PHASES: &[&str] = &["dev", "published"];
pub const DEV_ALWAYS_REQUIRED_WORKFLOWS: &[&str] = &["Tests", "CCBD Real Platform Smoke"];
pub const DEV_DEFAULT_BRANCH_WORKFLOWS: &[&str] = &["Cross-Platform Compatibility Test"];
pub const DEV_RELEASE_TRIGGER_PATHS: &[&str] = &["VERSION", "ccb"];
pub const DEV_HOMEPAGE_PATHS: &[&str] = &["README.md", "README_zh.md"];

pub const DEFAULT_REPO: &str = "SeemSeam/claude_codex_bridge";

#[derive(Debug, Default)]
pub struct Report {
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
}

impl Report {
    pub fn fail(&mut self, message: impl Into<String>, fix: Option<&str>) {
        let message = message.into();
        if let Some(fix) = fix {
            self.issues
                .push(format!("FAIL: {message}\n      fix: {fix}"));
        } else {
            self.issues.push(format!("FAIL: {message}"));
        }
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(format!("WARN: {}", message.into()));
    }

    pub fn has_issues(&self) -> bool {
        !self.issues.is_empty()
    }
}

fn fake_exit_status(code: u32) -> std::process::ExitStatus {
    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c").arg(format!("exit {}", code));
    cmd.output()
        .map(|o| o.status)
        .unwrap_or_else(|_| std::process::ExitStatus::default())
}

pub fn eprintln(message: impl AsRef<str>) {
    let _ = std::io::stderr().write_all(message.as_ref().as_bytes());
    let _ = std::io::stderr().write_all(b"\n");
}

pub fn run(cmd: &[&str], cwd: &Utf8Path) -> Output {
    run_with_timeout(cmd, cwd, 60)
}

/// Run a command with a timeout.
pub fn run_with_timeout(cmd: &[&str], cwd: &Utf8Path, timeout_seconds: u64) -> Output {
    let mut command = Command::new(cmd[0]);
    command.args(&cmd[1..]).current_dir(cwd);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match command.spawn() {
        Ok(mut child) => {
            match child.wait_timeout(std::time::Duration::from_secs(timeout_seconds)) {
                Ok(Some(status)) => {
                    let mut output = match child.wait_with_output() {
                        Ok(out) => out,
                        Err(_) => {
                            return Output {
                                status: fake_exit_status(1),
                                stdout: Vec::new(),
                                stderr: b"failed to capture output".to_vec(),
                            }
                        }
                    };
                    output.status = status;
                    output
                }
                Ok(None) => {
                    // Timed out
                    let _ = child.kill();
                    Output {
                        status: fake_exit_status(124),
                        stdout: Vec::new(),
                        stderr: format!("command timed out after {timeout_seconds}s").into_bytes(),
                    }
                }
                Err(_) => Output {
                    status: fake_exit_status(1),
                    stdout: Vec::new(),
                    stderr: b"failed to wait for command".to_vec(),
                },
            }
        }
        Err(_) => Output {
            status: fake_exit_status(127),
            stdout: Vec::new(),
            stderr: b"command not found".to_vec(),
        },
    }
}

pub fn repo_root(start: &Utf8Path) -> Utf8PathBuf {
    let output = run_with_timeout(&["git", "rev-parse", "--show-toplevel"], start, 60);
    if output.status.success() {
        return String::from_utf8_lossy(&output.stdout).trim().into();
    }
    start
        .canonicalize_utf8()
        .unwrap_or_else(|_| start.to_path_buf())
}

pub fn read(path: &Utf8Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub fn read_bytes(path: &Utf8Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

pub fn infer_repo(root: &Utf8Path) -> String {
    let output = run_with_timeout(&["git", "remote", "get-url", "origin"], root, 60);
    if !output.status.success() {
        return DEFAULT_REPO.to_string();
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let re = regex::Regex::new(r"github.com[:/]([^/]+)/([^/.]+)(?:\.git)?$").unwrap();
    re.captures(&url)
        .map(|caps| format!("{}/{}", &caps[1], &caps[2]))
        .unwrap_or_else(|| DEFAULT_REPO.to_string())
}

pub fn git_output(root: &Utf8Path, args: &[&str]) -> Option<String> {
    let mut cmd = vec!["git"];
    cmd.extend_from_slice(args);
    let output = run_with_timeout(&cmd, root, 60);
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

pub fn normalize_version(version: &str) -> String {
    let version = version.trim();
    if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    }
}

pub fn bare_version(version: &str) -> String {
    version.strip_prefix('v').unwrap_or(version).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_version() {
        assert_eq!(normalize_version("1.2.3"), "v1.2.3");
        assert_eq!(normalize_version("v1.2.3"), "v1.2.3");
        assert_eq!(normalize_version("  1.2.3  "), "v1.2.3");
    }

    #[test]
    fn test_bare_version() {
        assert_eq!(bare_version("v1.2.3"), "1.2.3");
        assert_eq!(bare_version("1.2.3"), "1.2.3");
    }
}
