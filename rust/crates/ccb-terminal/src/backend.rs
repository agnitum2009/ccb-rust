use std::process::{Command, Output, Stdio};
use std::time::Duration;

use thiserror::Error;

use crate::tmux;

#[derive(Error, Debug)]
pub enum TerminalError {
    #[error("tmux error: {0}")]
    Tmux(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("pane not found: {0}")]
    PaneNotFound(String),
    #[error("command failed: {0}")]
    CommandFailed(String),
}

pub type Result<T> = std::result::Result<T, TerminalError>;

/// Abstract terminal backend trait. Maps to Python `TerminalBackend` ABC.
pub trait TerminalBackend: Send + Sync {
    fn send_text(&self, pane_id: &str, text: &str) -> Result<()>;
    fn is_alive(&self, pane_id: &str) -> Result<bool>;
    fn kill_pane(&self, pane_id: &str) -> Result<()>;
    fn activate(&self, pane_id: &str) -> Result<()>;
    fn create_pane(
        &self,
        cmd: &str,
        cwd: &str,
        direction: &str,
        percent: u32,
        parent_pane: Option<&str>,
    ) -> Result<String>;
}

/// Result of a tmux subprocess invocation.
#[derive(Debug, Clone)]
pub struct TmuxOutput {
    pub stdout: String,
    pub stderr: String,
    pub status: std::process::ExitStatus,
}

impl TmuxOutput {
    pub fn success(&self) -> bool {
        self.status.success()
    }

    pub fn returncode(&self) -> Option<i32> {
        self.status.code()
    }
}

/// Tmux backend implementation.
#[derive(Debug, Clone)]
pub struct TmuxBackend {
    socket_name: Option<String>,
    socket_path: Option<String>,
}

impl TmuxBackend {
    pub fn new(socket_name: Option<String>, socket_path: Option<String>) -> Self {
        let socket_path = socket_path
            .or_else(|| std::env::var("CCB_TMUX_SOCKET_PATH").ok())
            .filter(|s| !s.trim().is_empty())
            .map(|s| expanduser(&s));
        let socket_name = socket_name
            .or_else(|| std::env::var("CCB_TMUX_SOCKET").ok())
            .filter(|s| !s.trim().is_empty());
        Self {
            socket_name,
            socket_path,
        }
    }

    pub fn socket_name(&self) -> Option<&str> {
        self.socket_name.as_deref()
    }

    pub fn socket_path(&self) -> Option<&str> {
        self.socket_path.as_deref()
    }

    /// Build the base tmux command with socket/config arguments.
    pub fn tmux_base(&self) -> Vec<String> {
        tmux::tmux_base(self.socket_name.as_deref(), self.socket_path.as_deref())
    }

    /// Run a tmux command with the given arguments.
    pub fn tmux_run(
        &self,
        args: &[&str],
        check: bool,
        capture: bool,
        input_bytes: Option<&[u8]>,
        timeout: Option<Duration>,
    ) -> std::io::Result<TmuxOutput> {
        let base = self.tmux_base();
        let mut cmd = Command::new(&base[0]);
        for arg in &base[1..] {
            cmd.arg(arg);
        }
        for arg in args {
            cmd.arg(arg);
        }
        cmd.env_clear();
        for (key, value) in isolated_tmux_env() {
            cmd.env(key, value);
        }
        if input_bytes.is_some() {
            cmd.stdin(Stdio::piped());
        }
        if capture {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        }

        let output = if let Some(t) = timeout {
            run_with_timeout(cmd, input_bytes, t)?
        } else {
            run_command(cmd, input_bytes)?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if check && !output.status.success() {
            return Err(std::io::Error::other(format!(
                "tmux command failed ({}): {}",
                output.status.code().unwrap_or(-1),
                stderr.trim()
            )));
        }

        Ok(TmuxOutput {
            stdout,
            stderr,
            status: output.status,
        })
    }

    pub fn tmux_run_capture(&self, args: &[&str]) -> Result<String> {
        let output = self.tmux_run(args, false, true, None, None)?;
        Ok(output.stdout)
    }

    /// Strip ANSI escape sequences from text.
    pub fn strip_ansi(text: &str) -> String {
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let re = RE.get_or_init(|| regex::Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").unwrap());
        re.replace_all(text, "").to_string()
    }

    pub fn looks_like_pane_id(&self, value: &str) -> bool {
        tmux::looks_like_pane_id(value)
    }

    pub fn looks_like_tmux_target(&self, value: &str) -> bool {
        tmux::looks_like_tmux_target(value)
    }

    pub fn require_pane_id(&self, pane_id: &str, action: &str) -> Result<String> {
        let pane_id = pane_id.trim();
        if self.looks_like_pane_id(pane_id) {
            Ok(pane_id.to_string())
        } else {
            Err(TerminalError::CommandFailed(format!(
                "{action} requires tmux pane id, got {pane_id:?}"
            )))
        }
    }

    pub fn env_tmux_pane(&self) -> String {
        std::env::var("TMUX_PANE").unwrap_or_default()
    }
}

impl TerminalBackend for TmuxBackend {
    fn send_text(&self, pane_id: &str, text: &str) -> Result<()> {
        let pane_id = self.require_pane_id(pane_id, "send_text")?;
        self.tmux_run(
            &["send-keys", "-t", &pane_id, "-l", text],
            true,
            false,
            None,
            None,
        )?;
        Ok(())
    }

    fn is_alive(&self, pane_id: &str) -> Result<bool> {
        if pane_id.trim().is_empty() {
            return Ok(false);
        }
        if self.looks_like_tmux_target(pane_id) {
            let output =
                self.tmux_run_capture(&["display-message", "-p", "-t", pane_id, "#{pane_dead}"])?;
            Ok(tmux::pane_is_alive(&output))
        } else {
            let output = self.tmux_run(&["has-session", "-t", pane_id], false, true, None, None)?;
            Ok(output.success())
        }
    }

    fn kill_pane(&self, pane_id: &str) -> Result<()> {
        if pane_id.trim().is_empty() {
            return Ok(());
        }
        if self.looks_like_tmux_target(pane_id) {
            self.tmux_run(&["kill-pane", "-t", pane_id], false, false, None, None)?;
        } else {
            self.tmux_run(&["kill-session", "-t", pane_id], false, false, None, None)?;
        }
        Ok(())
    }

    fn activate(&self, pane_id: &str) -> Result<()> {
        if pane_id.trim().is_empty() {
            return Ok(());
        }
        if self.looks_like_tmux_target(pane_id) {
            let pane_id = self.require_pane_id(pane_id, "activate")?;
            self.tmux_run(&["select-pane", "-t", &pane_id], false, false, None, None)?;
            if tmux::should_attach_selected_pane(&std::env::var("TMUX").unwrap_or_default()) {
                if let Ok(session) = self.tmux_run_capture(&[
                    "display-message",
                    "-p",
                    "-t",
                    &pane_id,
                    "#{session_name}",
                ]) {
                    let session = tmux::parse_session_name(&session);
                    if !session.is_empty() {
                        self.tmux_run(&["attach", "-t", &session], false, false, None, None)?;
                    }
                }
            }
        } else {
            self.tmux_run(&["attach", "-t", pane_id], false, false, None, None)?;
        }
        Ok(())
    }

    fn create_pane(
        &self,
        cmd: &str,
        cwd: &str,
        direction: &str,
        percent: u32,
        parent_pane: Option<&str>,
    ) -> Result<String> {
        let (flag, _) = tmux::normalize_split_direction(direction);
        let percent = percent.clamp(1, 99);
        let mut args = vec![
            "split-window".to_string(),
            flag.to_string(),
            "-p".to_string(),
            percent.to_string(),
            "-c".to_string(),
            cwd.to_string(),
        ];
        if let Some(parent) = parent_pane {
            args.push("-t".to_string());
            args.push(parent.to_string());
        }
        args.push(cmd.to_string());

        let output = self.tmux_run(
            &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            false,
            true,
            None,
            None,
        )?;
        Ok(output.stdout.trim().to_string())
    }
}

fn run_command(mut cmd: Command, input_bytes: Option<&[u8]>) -> std::io::Result<Output> {
    let mut child = cmd.spawn()?;
    if let Some(bytes) = input_bytes {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(bytes)?;
        }
    }
    child.wait_with_output()
}

fn run_with_timeout(
    mut cmd: Command,
    input_bytes: Option<&[u8]>,
    timeout: Duration,
) -> std::io::Result<Output> {
    let mut child = cmd.spawn()?;
    if let Some(bytes) = input_bytes {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(bytes)?;
        }
    }
    let id = child.id();
    std::thread::spawn(move || {
        std::thread::sleep(timeout);
        unsafe {
            libc::kill(id as i32, libc::SIGTERM);
        }
    });
    child.wait_with_output()
}

fn expanduser(path: &str) -> String {
    if path.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            return path.replacen('~', &home, 1);
        }
    }
    path.to_string()
}

/// Build an isolated tmux environment.
pub fn isolated_tmux_env() -> Vec<(String, String)> {
    let base = tmux_compatible_env();
    let remove = [
        "TMUX",
        "TMUX_PANE",
        "CCB_TMUX_SOCKET",
        "CCB_TMUX_SOCKET_PATH",
    ];
    base.into_iter()
        .filter(|(k, _)| !remove.contains(&k.as_str()))
        .collect()
}

fn tmux_compatible_env() -> Vec<(String, String)> {
    let mut compatible: Vec<(String, String)> = std::env::vars().collect();
    let term = std::env::var("TERM")
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    if term == "xterm-ghostty" {
        if let Some(idx) = compatible.iter().position(|(k, _)| k == "TERM") {
            compatible[idx].1 = "xterm-256color".to_string();
        } else {
            compatible.push(("TERM".to_string(), "xterm-256color".to_string()));
        }
    }
    compatible
}

/// Backend selection that mimics Python `TerminalBackendSelection`.
#[derive(Debug, Default)]
pub struct TerminalBackendSelection {
    cached: Option<TmuxBackend>,
}

impl TerminalBackendSelection {
    pub fn new() -> Self {
        Self { cached: None }
    }

    pub fn get_backend(&mut self) -> &TmuxBackend {
        if self.cached.is_none() {
            self.cached = Some(TmuxBackend::new(None, None));
        }
        self.cached.as_ref().unwrap()
    }

    pub fn get_backend_for_session(&self, session: &crate::registry::UserSession) -> TmuxBackend {
        let socket_name = session.tmux_socket_name.clone();
        let socket_path = session.tmux_socket_path.clone();
        TmuxBackend::new(socket_name, socket_path)
    }

    pub fn get_pane_id_from_session(
        &self,
        session: &crate::registry::UserSession,
    ) -> Option<String> {
        session
            .pane_id
            .clone()
            .or_else(|| session.tmux_session.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        let text = "\x1b[31mhello\x1b[0m world";
        assert_eq!(TmuxBackend::strip_ansi(text), "hello world");
    }

    #[test]
    fn test_expanduser() {
        std::env::set_var("HOME", "/home/test");
        assert_eq!(expanduser("~/foo"), "/home/test/foo");
        assert_eq!(expanduser("/abs/path"), "/abs/path");
    }

    #[test]
    fn test_tmux_base_no_socket() {
        std::env::remove_var("CCB_TMUX_SOCKET");
        std::env::remove_var("CCB_TMUX_SOCKET_PATH");
        let backend = TmuxBackend::new(None, None);
        let base = backend.tmux_base();
        assert_eq!(base[0], "tmux");
    }

    #[test]
    fn test_tmux_base_with_socket_name() {
        let backend = TmuxBackend::new(Some("mysock".into()), None);
        let base = backend.tmux_base();
        assert!(base.contains(&"-L".to_string()));
        assert!(base.contains(&"mysock".to_string()));
    }

    #[test]
    fn test_backend_selection_caches() {
        let mut selection = TerminalBackendSelection::new();
        let first = selection.get_backend() as *const TmuxBackend;
        let second = selection.get_backend() as *const TmuxBackend;
        assert_eq!(first, second);
    }
}
