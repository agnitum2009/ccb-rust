use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;

use crate::panes::{TmuxRunOutput, TmuxRunner};
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

pub use crate::backend_selection::TerminalBackendSelection;

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

type TmuxRunFn = Arc<
    dyn Fn(
            Vec<String>,
            bool,
            bool,
            Option<Vec<u8>>,
            Option<Duration>,
            Vec<(String, String)>,
        ) -> std::io::Result<TmuxOutput>
        + Send
        + Sync,
>;

/// Tmux backend implementation.
#[derive(Clone)]
pub struct TmuxBackend {
    socket_name: Option<String>,
    socket_path: Option<String>,
    runner: Option<TmuxRunFn>,
}

impl std::fmt::Debug for TmuxBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TmuxBackend")
            .field("socket_name", &self.socket_name)
            .field("socket_path", &self.socket_path)
            .field("has_runner", &self.runner.is_some())
            .finish()
    }
}

impl TmuxBackend {
    pub fn new(socket_name: Option<String>, socket_path: Option<String>) -> Self {
        let socket_path = socket_path
            .or_else(|| std::env::var("CCBR_TMUX_SOCKET_PATH").ok())
            .filter(|s| !s.trim().is_empty())
            .map(|s| expanduser(&s));
        let socket_name = socket_name
            .or_else(|| std::env::var("CCBR_TMUX_SOCKET").ok())
            .filter(|s| !s.trim().is_empty());
        Self {
            socket_name,
            socket_path,
            runner: None,
        }
    }

    /// Replace the real subprocess runner with a custom function.
    ///
    /// Intended for tests that need to assert on the commands and environment
    /// that would be passed to tmux without spawning a real tmux server.
    pub fn with_runner<F>(mut self, runner: F) -> Self
    where
        F: Fn(
                Vec<String>,
                bool,
                bool,
                Option<Vec<u8>>,
                Option<Duration>,
                Vec<(String, String)>,
            ) -> std::io::Result<TmuxOutput>
            + Send
            + Sync
            + 'static,
    {
        self.runner = Some(Arc::new(runner));
        self
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
        let full: Vec<String> = base
            .into_iter()
            .chain(args.iter().map(|s| s.to_string()))
            .collect();
        let env: Vec<(String, String)> = crate::env::isolated_tmux_env().into_iter().collect();

        if let Some(runner) = &self.runner {
            return runner(
                full,
                check,
                capture,
                input_bytes.map(|b| b.to_vec()),
                timeout,
                env,
            );
        }

        let mut cmd = Command::new(&full[0]);
        for arg in &full[1..] {
            cmd.arg(arg);
        }
        cmd.env_clear();
        for (key, value) in env {
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

    pub fn get_current_pane_id(&self) -> Result<String> {
        self.pane_service()
            .get_current_pane_id(&self.env_tmux_pane())
            .map_err(|e| TerminalError::CommandFailed(e.to_string()))
    }

    pub fn pane_exists(&self, pane_id: &str) -> bool {
        self.pane_service().pane_exists(pane_id)
    }

    pub fn find_pane_by_title_marker(&self, marker: &str) -> Option<String> {
        self.pane_service().find_pane_by_title_marker(marker)
    }

    pub fn describe_pane(
        &self,
        pane_id: &str,
        user_options: &[&str],
    ) -> Option<std::collections::HashMap<String, String>> {
        let opts: Vec<String> = user_options.iter().map(|s| s.to_string()).collect();
        self.pane_service().describe_pane(pane_id, &opts)
    }

    pub fn is_pane_alive(&self, pane_id: &str) -> bool {
        self.pane_service().is_pane_alive(pane_id)
    }

    pub fn is_tmux_pane_alive(&self, pane_id: &str) -> Result<bool> {
        let _ = self.require_pane_id(pane_id, "is_tmux_pane_alive")?;
        Ok(self.is_pane_alive(pane_id))
    }

    pub fn send_text_to_pane(&self, pane_id: &str, text: &str) -> Result<()> {
        let pane_id = self.require_pane_id(pane_id, "send_text_to_pane")?;
        self.send_text(&pane_id, text)
    }

    pub fn kill_tmux_pane(&self, pane_id: &str) -> Result<()> {
        let pane_id = self.require_pane_id(pane_id, "kill_tmux_pane")?;
        self.kill_pane(&pane_id)
    }

    pub fn activate_tmux_pane(&self, pane_id: &str) -> Result<()> {
        let pane_id = self.require_pane_id(pane_id, "activate_tmux_pane")?;
        self.activate(&pane_id)
    }

    pub fn split_pane(
        &self,
        parent_pane_id: &str,
        direction: &str,
        percent: u32,
        cmd: Option<&str>,
        cwd: Option<&str>,
    ) -> Result<String> {
        self.pane_service()
            .split_pane(parent_pane_id, direction, percent, cmd, cwd)
            .map_err(|e| TerminalError::CommandFailed(e.to_string()))
    }

    /// Ensure the pane is not in copy mode, cancelling it if necessary.
    ///
    /// Mirrors Python `TmuxBackend._ensure_not_in_copy_mode`.
    pub fn ensure_not_in_copy_mode(&self, pane_id: &str) {
        let Ok(output) = self.tmux_run(
            &["display-message", "-p", "-t", pane_id, "#{pane_in_mode}"],
            false,
            true,
            None,
            None,
        ) else {
            return;
        };
        if tmux::copy_mode_is_active(&output.stdout)
            && self
                .tmux_run(
                    &["send-keys", "-t", pane_id, "-X", "cancel"],
                    false,
                    false,
                    None,
                    None,
                )
                .is_err()
        {
            // Best-effort cancellation.
        }
    }

    fn env_float(&self, name: &str, default: f64) -> f64 {
        crate::env::env_float(name, default)
    }

    fn pane_service(&self) -> crate::panes::TmuxPaneService {
        let backend = self.clone();
        crate::panes::TmuxPaneService::new(
            move |args: &[&str], check: bool, capture: bool| -> anyhow::Result<TmuxRunOutput> {
                let output = backend
                    .tmux_run(args, check, capture, None, None)
                    .map_err(|e| anyhow::anyhow!(e))?;
                if check && !output.success() {
                    return Err(anyhow::anyhow!(
                        "tmux command failed ({}): {}",
                        output.status.code().unwrap_or(-1),
                        output.stderr
                    ));
                }
                Ok(TmuxRunOutput {
                    stdout: output.stdout,
                    stderr: output.stderr,
                    returncode: output.status.code().unwrap_or(-1),
                })
            },
        )
    }

    /// Create a detached session and return its root pane id.
    fn create_detached_root_pane(&self, cwd: &str) -> Result<String> {
        let session_name = tmux::default_detached_session_name(
            cwd,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        );
        let mut args = vec![
            "new-session".to_string(),
            "-d".to_string(),
            "-s".to_string(),
            session_name.clone(),
            "-c".to_string(),
            cwd.to_string(),
        ];
        args.extend(tmux::pane_placeholder_argv());
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.tmux_run(&args_ref, true, false, None, None)?;
        let output = self.tmux_run(
            &["list-panes", "-t", &session_name, "-F", "#{pane_id}"],
            true,
            true,
            None,
            None,
        )?;
        let pane_id = output
            .stdout
            .lines()
            .map(|l| l.trim())
            .find(|l| !l.is_empty())
            .unwrap_or("")
            .to_string();
        if pane_id.starts_with('%') {
            Ok(pane_id)
        } else {
            Err(TerminalError::CommandFailed(
                "tmux failed to resolve root pane_id for detached session".to_string(),
            ))
        }
    }

    fn respawn_service(&self) -> crate::respawn::TmuxRespawnService {
        let backend = self.clone();
        crate::respawn::TmuxRespawnService::new(
            Box::new(backend),
            |_pane_id| {},
            std::env::vars().collect(),
        )
    }
}

impl TmuxRunner for TmuxBackend {
    fn run(&self, args: &[&str], check: bool, capture: bool) -> anyhow::Result<TmuxRunOutput> {
        let output = self
            .tmux_run(args, check, capture, None, None)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(TmuxRunOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            returncode: output.status.code().unwrap_or(-1),
        })
    }

    fn run_with_input(
        &self,
        args: &[&str],
        check: bool,
        capture: bool,
        input_bytes: Option<&[u8]>,
    ) -> anyhow::Result<TmuxRunOutput> {
        let output = self
            .tmux_run(args, check, capture, input_bytes, None)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(TmuxRunOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            returncode: output.status.code().unwrap_or(-1),
        })
    }
}

impl TerminalBackend for TmuxBackend {
    fn send_text(&self, pane_id: &str, text: &str) -> Result<()> {
        // Delegate to the Python-mirroring buffer-based sender. It accepts both tmux
        // pane targets and session names and uses `load-buffer`/`paste-buffer` for
        // multi-line or large payloads.
        let backend_for_copy_mode = self.clone();
        let backend_for_env = self.clone();
        let sender = crate::input::TmuxTextSender::new(
            Box::new(self.clone()),
            move |pid| {
                backend_for_copy_mode.ensure_not_in_copy_mode(pid);
            },
            move |name, default| backend_for_env.env_float(name, default),
        );
        sender
            .send_text(pane_id, text)
            .map_err(|e| TerminalError::CommandFailed(e.to_string()))
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
        let cmd = cmd.trim();
        let cwd = cwd.trim();
        let cwd = if cwd.is_empty() { "." } else { cwd };

        // Resolve parent pane: explicit parent, current tmux pane, or none (detached session).
        let base = parent_pane
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string())
            .or_else(|| {
                self.pane_service()
                    .get_current_pane_id(&self.env_tmux_pane())
                    .ok()
            })
            .unwrap_or_default();

        let pane_id = if !base.is_empty() {
            self.pane_service()
                .split_pane(&base, direction, percent, None, Some(cwd))
                .map_err(|e| TerminalError::CommandFailed(e.to_string()))?
        } else {
            self.create_detached_root_pane(cwd)?
        };

        if !cmd.is_empty() {
            self.respawn_service()
                .respawn_pane(&pane_id, cmd, Some(cwd), None, false)
                .map_err(|e| TerminalError::CommandFailed(e.to_string()))?;
        }

        Ok(pane_id)
    }
}

impl crate::layouts::TmuxLayoutBackend for TmuxBackend {
    fn get_current_pane_id(&self) -> anyhow::Result<String> {
        self.pane_service()
            .get_current_pane_id(&self.env_tmux_pane())
    }

    fn is_alive(&self, pane_id: &str) -> bool {
        self.pane_service().is_pane_alive(pane_id)
    }

    fn create_pane(
        &self,
        cmd: &str,
        cwd: &str,
        direction: &str,
        percent: u32,
        parent_pane: Option<&str>,
    ) -> anyhow::Result<String> {
        TerminalBackend::create_pane(self, cmd, cwd, direction, percent, parent_pane)
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn split_pane(
        &self,
        parent_pane_id: &str,
        direction: &str,
        percent: u32,
    ) -> anyhow::Result<String> {
        self.pane_service()
            .split_pane(parent_pane_id, direction, percent, None, None)
    }

    fn set_pane_title(&self, pane_id: &str, title: &str) {
        self.pane_service().set_pane_title(pane_id, title);
    }

    fn set_pane_user_option(&self, pane_id: &str, name: &str, value: &str) {
        self.pane_service()
            .set_pane_user_option(pane_id, name, value);
    }

    fn set_pane_style(
        &self,
        pane_id: &str,
        border_style: Option<&str>,
        active_border_style: Option<&str>,
    ) {
        self.pane_service()
            .set_pane_style(pane_id, border_style, active_border_style);
    }

    fn tmux_run(&self, args: &[&str], check: bool, capture: bool) -> anyhow::Result<String> {
        let output = self
            .tmux_run(args, check, capture, None, None)
            .map_err(|e| anyhow::anyhow!(e))?;
        if check && !output.success() {
            return Err(anyhow::anyhow!(
                "tmux command failed ({}): {}",
                output.status.code().unwrap_or(-1),
                output.stderr
            ));
        }
        Ok(output.stdout)
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

/// Build an isolated tmux environment as a vector of pairs.
///
/// This preserves the original return type for backwards compatibility;
/// the canonical implementation lives in [`crate::env::isolated_tmux_env`].
pub fn isolated_tmux_env() -> Vec<(String, String)> {
    crate::env::isolated_tmux_env().into_iter().collect()
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
        std::env::remove_var("CCBR_TMUX_SOCKET");
        std::env::remove_var("CCBR_TMUX_SOCKET_PATH");
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
}
