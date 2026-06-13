use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::env;
use crate::panes::TmuxRunner;
use crate::readiness::{is_tmux_transient_server_error_text, TmuxTransientServerUnavailable};

/// Normalize a start directory; empty or `.` becomes empty string.
pub fn normalize_start_dir(cwd: Option<&str>) -> String {
    let start_dir = cwd.unwrap_or("").trim();
    if start_dir.is_empty() || start_dir == "." {
        String::new()
    } else {
        start_dir.to_string()
    }
}

/// Append stderr redirection to a command body.
pub fn append_stderr_redirection(
    cmd_body: &str,
    stderr_log_path: Option<&str>,
) -> (String, Option<String>) {
    let Some(path) = stderr_log_path else {
        return (cmd_body.to_string(), None);
    };
    let log_path = std::fs::canonicalize(path).unwrap_or_else(|_| Path::new(path).to_path_buf());
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let quoted = shell_quote(&log_path.to_string_lossy());
    (
        format!("{cmd_body} 2>> {quoted}"),
        Some(log_path.to_string_lossy().to_string()),
    )
}

/// Resolve shell preference order.
pub fn resolve_shell(
    env_shell: Option<&str>,
    tmux_default_shell: Option<&str>,
    process_shell: Option<&str>,
    fallback_shell: &str,
) -> String {
    if let Some(shell) = env_shell {
        let shell = shell.trim();
        if !shell.is_empty() {
            return shell.to_string();
        }
    }
    if let Some(shell) = tmux_default_shell {
        let shell = shell.trim();
        if !shell.is_empty() {
            return shell.to_string();
        }
    }
    if let Some(shell) = process_shell {
        let shell = shell.trim();
        if !shell.is_empty() {
            return shell.to_string();
        }
    }
    fallback_shell.to_string()
}

/// Resolve shell flags from explicit raw flags or shell name defaults.
pub fn resolve_shell_flags(shell: &str, flags_raw: Option<&str>) -> Vec<String> {
    if let Some(raw) = flags_raw {
        let raw = raw.trim();
        if !raw.is_empty() {
            return shlex_split(raw);
        }
    }
    let shell_name = Path::new(shell)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    match shell_name.as_str() {
        "bash" | "zsh" | "ksh" | "fish" => vec!["-l".to_string(), "-c".to_string()],
        "sh" | "dash" => vec!["-c".to_string()],
        _ => vec!["-c".to_string()],
    }
}

/// Build a full shell command string.
pub fn build_shell_command(shell: &str, flags: &[String], cmd_body: &str) -> String {
    let mut argv = vec![shell.to_string()];
    argv.extend(flags.iter().cloned());
    argv.push(cmd_body.to_string());
    argv.iter()
        .map(|s| shell_quote(s))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build tmux respawn-pane argument list.
pub fn build_respawn_tmux_args(pane_id: &str, start_dir: &str, full_command: &str) -> Vec<String> {
    let mut args = vec![
        "respawn-pane".to_string(),
        "-k".to_string(),
        "-t".to_string(),
        pane_id.to_string(),
    ];
    if !start_dir.is_empty() {
        args.push("-c".to_string());
        args.push(start_dir.to_string());
    }
    args.push(full_command.to_string());
    args
}

/// Service to respawn tmux panes with retry logic.
pub struct TmuxRespawnService {
    tmux_run: Box<dyn TmuxRunner>,
    ensure_pane_log: Box<dyn Fn(&str) + Send + Sync>,
    env: HashMap<String, String>,
}

impl TmuxRespawnService {
    pub fn new<F>(
        tmux_run: Box<dyn TmuxRunner>,
        ensure_pane_log: F,
        env: HashMap<String, String>,
    ) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        Self {
            tmux_run,
            ensure_pane_log: Box::new(ensure_pane_log),
            env,
        }
    }

    pub fn respawn_pane(
        &self,
        pane_id: &str,
        cmd: &str,
        cwd: Option<&str>,
        stderr_log_path: Option<&str>,
        remain_on_exit: bool,
    ) -> anyhow::Result<()> {
        let pane_text = pane_id.trim();
        if pane_text.is_empty() {
            return Err(anyhow::anyhow!("pane_id is required"));
        }
        let cmd_body = cmd.trim();
        if cmd_body.is_empty() {
            return Err(anyhow::anyhow!("cmd is required"));
        }

        (self.ensure_pane_log)(pane_id);
        let start_dir = normalize_start_dir(cwd);
        let (cmd_body, _) = append_stderr_redirection(cmd_body, stderr_log_path);
        let full = self.resolved_shell_command(&cmd_body);

        if remain_on_exit {
            self.set_remain_on_exit(pane_id);
        }
        let tmux_args = build_respawn_tmux_args(pane_id, &start_dir, &full);
        self.run_respawn_command(&tmux_args)?;
        if remain_on_exit {
            self.set_remain_on_exit(pane_id);
        }
        Ok(())
    }

    fn resolved_shell_command(&self, cmd_body: &str) -> String {
        let shell = resolve_shell(
            self.env.get("CCB_TMUX_SHELL").map(|s| s.as_str()),
            self.tmux_default_shell().as_deref(),
            self.env.get("SHELL").map(|s| s.as_str()),
            &default_shell().0,
        );
        let flags = resolve_shell_flags(
            &shell,
            self.env.get("CCB_TMUX_SHELL_FLAGS").map(|s| s.as_str()),
        );
        build_shell_command(&shell, &flags, cmd_body)
    }

    fn tmux_default_shell(&self) -> Option<String> {
        let output = self
            .tmux_run
            .run(&["show-option", "-gqv", "default-shell"], false, true)
            .ok()?;
        if output.returncode != 0 {
            return None;
        }
        let shell = output.stdout.trim();
        if shell.is_empty() {
            None
        } else {
            Some(shell.to_string())
        }
    }

    fn set_remain_on_exit(&self, pane_id: &str) {
        let _ = self.tmux_run.run(
            &["set-option", "-p", "-t", pane_id, "remain-on-exit", "on"],
            false,
            true,
        );
    }

    fn run_respawn_command(&self, tmux_args: &[String]) -> anyhow::Result<()> {
        let timeout = tmux_object_ready_timeout_s(None);
        let deadline = Instant::now() + Duration::from_secs_f64(timeout);
        let mut last_error: Option<anyhow::Error>;
        loop {
            match self.run_respawn_once(tmux_args) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if !is_tmux_transient_server_error_text(&e.to_string()) {
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }
            if Instant::now() >= deadline {
                if let Some(e) = last_error {
                    if is_tmux_transient_server_error_text(&e.to_string()) {
                        return Err(TmuxTransientServerUnavailable::new(&e.to_string()).into());
                    }
                    return Err(e);
                }
                return Err(anyhow::anyhow!("respawn pane failed"));
            }
            std::thread::sleep(Duration::from_secs_f64(tmux_object_ready_poll_interval_s()));
        }
    }

    fn run_respawn_once(&self, tmux_args: &[String]) -> anyhow::Result<()> {
        let args_ref: Vec<&str> = tmux_args.iter().map(|s| s.as_str()).collect();
        let output = self.tmux_run.run(&args_ref, false, true)?;
        if output.returncode == 0 {
            return Ok(());
        }
        let detail = tmux_failure_detail(&output, tmux_args);
        if is_tmux_transient_server_error_text(&detail) {
            return Err(TmuxTransientServerUnavailable::new(&detail).into());
        }
        Err(anyhow::anyhow!("respawn pane failed: {detail}"))
    }
}

fn tmux_failure_detail(output: &crate::panes::TmuxRunOutput, tmux_args: &[String]) -> String {
    let stderr = output.stderr.trim();
    let stdout = output.stdout.trim();
    if !stderr.is_empty() || !stdout.is_empty() {
        return if stderr.is_empty() { stdout } else { stderr }.to_string();
    }
    format!("tmux command failed: {}", tmux_args.join(" "))
}

pub fn tmux_object_ready_timeout_s(timeout_s: Option<f64>) -> f64 {
    if let Some(t) = timeout_s {
        return t.max(0.0);
    }
    env::env_float("CCB_TMUX_OBJECT_READY_TIMEOUT_S", 3.0)
}

pub fn tmux_object_ready_poll_interval_s() -> f64 {
    env::env_float("CCB_TMUX_OBJECT_READY_POLL_INTERVAL_S", 0.05)
}

/// Return default shell and flag for current platform.
pub fn default_shell() -> (String, String) {
    #[cfg(target_os = "windows")]
    {
        for shell in ["pwsh", "powershell"] {
            if which(shell) {
                return (shell.to_string(), "-Command".to_string());
            }
        }
        return ("powershell".to_string(), "-Command".to_string());
    }
    #[cfg(not(target_os = "windows"))]
    {
        ("bash".to_string(), "-c".to_string())
    }
}

#[allow(dead_code)]
fn which(name: &str) -> bool {
    std::env::var("PATH")
        .ok()
        .map(|path| {
            path.split(':')
                .any(|dir| Path::new(dir).join(name).exists())
        })
        .unwrap_or(false)
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "_%-=+.,/:@".contains(c))
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

fn shlex_split(s: &str) -> Vec<String> {
    s.split_whitespace().map(|p| p.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panes::{TmuxRunOutput, TmuxRunner};

    fn ok(stdout: &str) -> TmuxRunOutput {
        TmuxRunOutput {
            stdout: stdout.to_string(),
            stderr: String::new(),
            returncode: 0,
        }
    }

    fn err(stderr: &str) -> TmuxRunOutput {
        TmuxRunOutput {
            stdout: String::new(),
            stderr: stderr.to_string(),
            returncode: 1,
        }
    }

    #[test]
    fn test_normalize_start_dir() {
        assert_eq!(normalize_start_dir(None), "");
        assert_eq!(normalize_start_dir(Some(".")), "");
        assert_eq!(normalize_start_dir(Some("/tmp/demo")), "/tmp/demo");
    }

    #[test]
    fn test_resolve_shell_prefers_explicit_then_tmux_then_process_then_fallback() {
        assert_eq!(
            resolve_shell(Some("/bin/zsh"), Some("/bin/bash"), Some("/bin/sh"), "bash"),
            "/bin/zsh"
        );
        assert_eq!(
            resolve_shell(None, Some("/bin/bash"), Some("/bin/sh"), "bash"),
            "/bin/bash"
        );
        assert_eq!(
            resolve_shell(None, None, Some("/bin/sh"), "bash"),
            "/bin/sh"
        );
        assert_eq!(resolve_shell(None, None, None, "bash"), "bash");
    }

    #[test]
    fn test_resolve_shell_flags_defaults() {
        assert_eq!(resolve_shell_flags("/bin/bash", None), vec!["-l", "-c"]);
        assert_eq!(resolve_shell_flags("/bin/zsh", None), vec!["-l", "-c"]);
        assert_eq!(resolve_shell_flags("/bin/dash", None), vec!["-c"]);
        assert_eq!(resolve_shell_flags("/bin/custom", None), vec!["-c"]);
        assert_eq!(resolve_shell_flags("/bin/bash", Some("-c")), vec!["-c"]);
    }

    #[test]
    fn test_build_shell_command_quotes_arguments() {
        let command = build_shell_command(
            "/bin/bash",
            &["-l".to_string(), "-c".to_string()],
            "echo hi > /tmp/a b",
        );
        assert!(command.starts_with("/bin/bash"));
        assert!(command.contains("'echo hi > /tmp/a b'"));
    }

    #[test]
    fn test_build_respawn_tmux_args() {
        assert_eq!(
            build_respawn_tmux_args("%9", "/tmp/demo", "bash -c 'echo hi'"),
            vec![
                "respawn-pane",
                "-k",
                "-t",
                "%9",
                "-c",
                "/tmp/demo",
                "bash -c 'echo hi'",
            ]
        );
    }

    #[test]
    fn test_tmux_respawn_service_builds_respawn_and_remain_calls() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<String>>::new()));
        let calls_clone = calls.clone();
        let runner: Box<dyn TmuxRunner> = Box::new(
            move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
                calls_clone
                    .lock()
                    .unwrap()
                    .push(args.iter().map(|s| s.to_string()).collect());
                if args == ["show-option", "-gqv", "default-shell"] {
                    return Ok(ok("/bin/bash\n"));
                }
                Ok(ok(""))
            },
        );
        let service = TmuxRespawnService::new(
            runner,
            |_pane_id| {},
            HashMap::from_iter([("SHELL".to_string(), "/bin/bash".to_string())]),
        );

        service
            .respawn_pane(
                "%9",
                "echo hi",
                Some("/tmp/demo"),
                Some("/tmp/err.log"),
                true,
            )
            .unwrap();

        let calls = calls.lock().unwrap();
        assert_eq!(calls[0], vec!["show-option", "-gqv", "default-shell"]);
        assert!(calls.iter().any(|c| {
            c.len() >= 6
                && c[0] == "set-option"
                && c[1] == "-p"
                && c[2] == "-t"
                && c[3] == "%9"
                && c[4] == "remain-on-exit"
                && c[5] == "on"
        }));
        assert!(calls.iter().any(|c| {
            c.len() >= 5 && c[0] == "respawn-pane" && c[1] == "-k" && c[2] == "-t" && c[3] == "%9"
        }));
    }

    #[test]
    fn test_tmux_respawn_service_requires_pane_and_cmd() {
        let runner: Box<dyn TmuxRunner> =
            Box::new(|_args: &[&str], _check: bool, _capture: bool| Ok(ok("")));
        let service = TmuxRespawnService::new(runner, |_pane_id| {}, HashMap::new());
        assert!(service
            .respawn_pane("", "echo hi", None, None, true)
            .is_err());
        assert!(service.respawn_pane("%1", "  ", None, None, true).is_err());
    }

    #[test]
    fn test_tmux_respawn_service_retries_transient_tmux_failures() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<String>>::new()));
        let calls_clone = calls.clone();
        let respawn_attempts = std::sync::Arc::new(std::sync::Mutex::new(0));
        let attempts_clone = respawn_attempts.clone();
        let runner: Box<dyn TmuxRunner> = Box::new(
            move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
                calls_clone
                    .lock()
                    .unwrap()
                    .push(args.iter().map(|s| s.to_string()).collect());
                if args == ["show-option", "-gqv", "default-shell"] {
                    return Ok(ok("/bin/bash\n"));
                }
                if !args.is_empty() && args[0] == "respawn-pane" {
                    let mut attempts = attempts_clone.lock().unwrap();
                    *attempts += 1;
                    if *attempts == 1 {
                        return Ok(err("no server running on /tmp/ccb-runtime/test.sock\n"));
                    }
                }
                Ok(ok(""))
            },
        );
        let service = TmuxRespawnService::new(
            runner,
            |_pane_id| {},
            HashMap::from_iter([("SHELL".to_string(), "/bin/bash".to_string())]),
        );

        service
            .respawn_pane("%9", "echo hi", None, None, false)
            .unwrap();

        assert_eq!(*respawn_attempts.lock().unwrap(), 2);
        let calls = calls.lock().unwrap();
        assert!(calls.iter().any(|c| {
            c.len() >= 5 && c[0] == "respawn-pane" && c[1] == "-k" && c[2] == "-t" && c[3] == "%9"
        }));
    }

    #[test]
    fn test_tmux_respawn_service_does_not_retry_non_transient_failure() {
        let respawn_attempts = std::sync::Arc::new(std::sync::Mutex::new(0));
        let attempts_clone = respawn_attempts.clone();
        let runner: Box<dyn TmuxRunner> = Box::new(
            move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
                if args == ["show-option", "-gqv", "default-shell"] {
                    return Ok(ok("/bin/bash\n"));
                }
                if !args.is_empty() && args[0] == "respawn-pane" {
                    let mut attempts = attempts_clone.lock().unwrap();
                    *attempts += 1;
                    return Ok(err("pane not found\n"));
                }
                Ok(ok(""))
            },
        );
        let service = TmuxRespawnService::new(
            runner,
            |_pane_id| {},
            HashMap::from_iter([("SHELL".to_string(), "/bin/bash".to_string())]),
        );

        let result = service.respawn_pane("%9", "echo hi", None, None, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("pane not found"));
        assert_eq!(*respawn_attempts.lock().unwrap(), 1);
    }
}
