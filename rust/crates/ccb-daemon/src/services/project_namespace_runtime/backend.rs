//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/backend.py`.
//! 1:1 file alignment stub.

use std::thread;
use std::time::{Duration, Instant};

use ccb_terminal::placeholders::{pane_placeholder_argv, pane_placeholder_cmd};
use ccb_terminal::readiness::{
    is_tmux_absent_server_text, is_tmux_missing_session_text, is_tmux_transient_server_error_text,
    tmux_command_failure_message, tmux_failure_detail,
};
use ccb_terminal::{TerminalBackend as _, TmuxBackend, TmuxOutput};

use crate::DaemonError;
use crate::Result;

// Re-export tmux error types so the public API matches Python's `__all__`.
pub use ccb_terminal::readiness::{TmuxCommandError, TmuxTransientServerUnavailable};

/// Environment keys propagated into the tmux server session.
const TMUX_ENVIRONMENT_KEYS: &[&str] = &[
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "WSL_DISTRO_NAME",
    "WSL_INTEROP",
    "SSH_AUTH_SOCK",
    "SSH_CONNECTION",
];

/// Default detached session size used when the caller does not supply a terminal size.
const DEFAULT_SESSION_WIDTH: i32 = 160;
const DEFAULT_SESSION_HEIGHT: i32 = 48;

/// Minimum sane dimensions for a materializable multi-pane session.
const MIN_SESSION_WIDTH: i32 = 40;
const MIN_SESSION_HEIGHT: i32 = 15;

/// Default timeout/poll values matching Python `terminal_runtime.tmux_readiness`.
const TMUX_OBJECT_READY_TIMEOUT_S: f64 = 3.0;
const TMUX_OBJECT_READY_POLL_INTERVAL_S: f64 = 0.05;

/// Clipboard pipe command bound to copy-mode-vi keys. Mirrors the Python helper.
const CLIPBOARD_PIPE_COMMAND: &str = "sh -lc 'tmp=$(mktemp \"${TMPDIR:-/tmp}/ccb-clipboard.XXXXXX\") || exit 0; cat >\"$tmp\"; if command -v wl-copy >/dev/null 2>&1 && [ -n \"${WAYLAND_DISPLAY:-}\" ]; then (wl-copy <\"$tmp\"; rm -f \"$tmp\") >/dev/null 2>&1 & elif command -v xclip >/dev/null 2>&1 && [ -n \"${DISPLAY:-}\" ]; then (xclip -selection clipboard <\"$tmp\"; rm -f \"$tmp\") >/dev/null 2>&1 & elif command -v xsel >/dev/null 2>&1 && [ -n \"${DISPLAY:-}\" ]; then (xsel --clipboard --input <\"$tmp\"; rm -f \"$tmp\") >/dev/null 2>&1 & elif command -v pbcopy >/dev/null 2>&1; then pbcopy <\"$tmp\"; rm -f \"$tmp\"; elif command -v powershell.exe >/dev/null 2>&1; then powershell.exe -NoProfile -Command \"[Console]::InputEncoding=[System.Text.UTF8Encoding]::new(); Set-Clipboard -Value ([Console]::In.ReadToEnd())\" <\"$tmp\"; rm -f \"$tmp\"; elif command -v pwsh >/dev/null 2>&1; then pwsh -NoLogo -NoProfile -Command \"[Console]::InputEncoding=[System.Text.UTF8Encoding]::new(); Set-Clipboard -Value ([Console]::In.ReadToEnd())\" <\"$tmp\"; rm -f \"$tmp\"; else rm -f \"$tmp\"; fi'";

/// Description of a tmux window returned by `list_windows`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxWindowRecord {
    pub window_id: Option<String>,
    pub window_name: String,
    pub active: bool,
}

impl TmuxWindowRecord {
    pub fn new(window_id: Option<String>, window_name: String, active: bool) -> Self {
        Self {
            window_id,
            window_name,
            active,
        }
    }
}

/// Factory that produces a tmux backend bound to a socket path.
///
/// Kept as a unit struct for backwards compatibility with callers that construct
/// `BackendFactory {}` directly. The actual socket is supplied to `build_backend`.
#[derive(Debug, Clone, Default)]
pub struct BackendFactory {}

/// Operational handle for a tmux backend.
#[derive(Debug, Clone)]
pub struct Backend {
    pub socket_path: String,
    pub session_name: String,
    tmux: TmuxBackend,
}

impl Backend {
    /// Run a raw tmux command through the backend.
    ///
    /// Mirrors Python `TmuxBackend._tmux_run(args, check=False, capture=True)`.
    pub fn _tmux_run(
        &self,
        args: &[&str],
        check: bool,
        capture: bool,
    ) -> BackendResult<TmuxOutput> {
        self.tmux
            .tmux_run(args, check, capture, None, None)
            .map_err(BackendError::Io)
    }

    /// Return the configured socket path if non-empty.
    pub fn socket_path(&self) -> Option<&str> {
        let path = self.socket_path.trim();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    }

    /// Build the base tmux command vector (program + socket args).
    pub fn tmux_base(&self) -> Vec<String> {
        self.tmux.tmux_base()
    }

    /// Check whether a session or pane-like target exists.
    pub fn is_alive(&self, session_name: &str) -> BackendResult<bool> {
        let output = self._tmux_run(&["has-session", "-t", session_name], false, true)?;
        Ok(output.success())
    }

    /// Split `target` and return the new pane id.
    ///
    /// Mirrors Python `TmuxBackend.split_pane(parent_pane_id, direction, percent, cmd, cwd)`.
    pub fn split_pane(
        &self,
        target: &str,
        direction: &str,
        percent: i32,
        cmd: &str,
        cwd: &str,
    ) -> BackendResult<String> {
        let clamped_percent = percent.clamp(1, 99) as u32;
        match self
            .tmux
            .create_pane(cmd, cwd, direction, clamped_percent, Some(target))
        {
            Ok(pane_id) if pane_id.starts_with('%') => Ok(pane_id),
            Ok(_) | Err(_) => self.split_pane_manual(target, direction, clamped_percent, cmd, cwd),
        }
    }

    fn split_pane_manual(
        &self,
        target: &str,
        direction: &str,
        percent: u32,
        cmd: &str,
        cwd: &str,
    ) -> BackendResult<String> {
        let mut args = vec!["split-window".to_string(), "-t".to_string(), target.to_string()];
        match direction.to_lowercase().as_str() {
            "left" => args.push("-hb".to_string()),
            "right" => args.push("-h".to_string()),
            "up" => args.push("-vb".to_string()),
            "down" => args.push("-v".to_string()),
            _ => {}
        }
        args.extend([
            "-p".to_string(),
            percent.to_string(),
            "-c".to_string(),
            cwd.to_string(),
        ]);
        if !cmd.is_empty() {
            args.push(cmd.to_string());
        }
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = self._tmux_run(&args_ref, true, true)?;
        if !output.success() {
            return Err(BackendError::Command(format!(
                "tmux split-window failed: {}",
                output.stderr.trim()
            )));
        }
        let pane_id = output.stdout.trim();
        if pane_id.starts_with('%') {
            Ok(pane_id.to_string())
        } else {
            Err(BackendError::Command(format!(
                "tmux split-window did not return pane_id: {pane_id:?}"
            )))
        }
    }
}

/// Build a backend bound to `socket_path`.
///
/// Mirrors Python `build_backend(backend_factory, socket_path=socket_path)`. The factory
/// is accepted for API compatibility but is currently a unit struct; the socket path is
/// the authoritative binding parameter.
pub fn build_backend(_factory: &BackendFactory, socket_path: &str) -> Result<Backend> {
    let socket_path = socket_path.to_string();
    let tmux = TmuxBackend::new(None, Some(socket_path.clone()));
    Ok(Backend {
        socket_path,
        session_name: String::new(),
        tmux,
    })
}

/// Start the tmux server if it is not already running.
pub fn prepare_server(backend: &Backend, timeout_s: Option<f64>) -> Result<()> {
    tmux_run_ready(
        backend,
        &["start-server"],
        "failed to prepare tmux server",
        timeout_s,
    )
    .map(|_| ())
    .map_err(DaemonError::from)
}

/// Apply CCB's default server/window/key policy to the tmux server.
///
/// Individual option failures are swallowed so that a conservative tmux build does not
/// abort namespace creation.
pub fn ensure_server_policy(backend: &Backend, timeout_s: Option<f64>) -> Result<()> {
    tmux_run_ready(
        backend,
        &["set-option", "-g", "destroy-unattached", "off"],
        "failed to persist tmux destroy-unattached policy",
        timeout_s,
    )
    .map(|_| ())
    .map_err(DaemonError::from)?;

    apply_optional_server_policy(backend, "mouse", "on", timeout_s)?;
    apply_optional_server_policy(backend, "history-limit", "50000", timeout_s)?;
    apply_optional_server_policy(backend, "set-clipboard", "on", timeout_s)?;
    apply_optional_server_policy(backend, "focus-events", "on", timeout_s)?;
    apply_optional_server_policy(backend, "escape-time", "10", timeout_s)?;
    apply_tmux_environment_policy(backend, timeout_s)?;
    apply_optional_window_policy(backend, "mode-keys", "vi", timeout_s)?;

    apply_optional_tmux_policy(
        backend,
        &["bind-key", "-T", "copy-mode-vi", "v", "send-keys", "-X", "begin-selection"],
        "tmux copy-mode-vi begin-selection binding",
        timeout_s,
    )?;
    apply_optional_tmux_policy(
        backend,
        &[
            "bind-key",
            "-T",
            "copy-mode-vi",
            "C-v",
            "send-keys",
            "-X",
            "rectangle-toggle",
        ],
        "tmux copy-mode-vi rectangle-toggle binding",
        timeout_s,
    )?;

    for key in ["y", "Enter", "MouseDragEnd1Pane"] {
        apply_optional_tmux_policy(
            backend,
            &[
                "bind-key",
                "-T",
                "copy-mode-vi",
                key,
                "send-keys",
                "-X",
                "copy-pipe-and-cancel",
                CLIPBOARD_PIPE_COMMAND,
            ],
            &format!("tmux copy-mode-vi clipboard binding {key}"),
            timeout_s,
        )?;
    }

    for (key, direction) in [("h", "-L"), ("j", "-D"), ("k", "-U"), ("l", "-R")] {
        apply_optional_tmux_policy(
            backend,
            &["bind-key", key, "select-pane", direction],
            &format!("tmux vi pane focus binding {key}"),
            timeout_s,
        )?;
    }

    for (key, direction) in [("H", "-L"), ("J", "-D"), ("K", "-U"), ("L", "-R")] {
        apply_optional_tmux_policy(
            backend,
            &["bind-key", "-r", key, "resize-pane", direction, "5"],
            &format!("tmux vi pane resize binding {key}"),
            timeout_s,
        )?;
    }

    Ok(())
}

fn apply_tmux_environment_policy(backend: &Backend, timeout_s: Option<f64>) -> Result<()> {
    let update_environment = TMUX_ENVIRONMENT_KEYS.join(" ");
    apply_optional_tmux_policy(
        backend,
        &[
            "set-option",
            "-g",
            "update-environment",
            &update_environment,
        ],
        "tmux update-environment policy",
        timeout_s,
    )?;

    for key in TMUX_ENVIRONMENT_KEYS {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                apply_optional_tmux_policy(
                    backend,
                    &["set-environment", "-g", key, &value],
                    &format!("tmux environment {key}"),
                    timeout_s,
                )?;
            }
        }
    }
    Ok(())
}

fn apply_optional_server_policy(
    backend: &Backend,
    option: &str,
    value: &str,
    timeout_s: Option<f64>,
) -> Result<()> {
    apply_optional_tmux_policy(
        backend,
        &["set-option", "-g", option, value],
        &format!("tmux {option} policy"),
        timeout_s,
    )
}

fn apply_optional_window_policy(
    backend: &Backend,
    option: &str,
    value: &str,
    timeout_s: Option<f64>,
) -> Result<()> {
    apply_optional_tmux_policy(
        backend,
        &["set-window-option", "-g", option, value],
        &format!("tmux {option} window policy"),
        timeout_s,
    )
}

fn apply_optional_tmux_policy(
    backend: &Backend,
    args: &[&str],
    description: &str,
    timeout_s: Option<f64>,
) -> Result<()> {
    let _ = tmux_run_ready(
        backend,
        args,
        &format!("failed to persist {description}"),
        timeout_s,
    );
    Ok(())
}

/// Create a detached tmux session.
#[allow(clippy::too_many_arguments)]
pub fn create_session(
    backend: &Backend,
    session_name: &str,
    project_root: &str,
    window_name: Option<&str>,
    terminal_size: Option<(i32, i32)>,
    timeout_s: Option<f64>,
) -> Result<()> {
    let (width, height) = resolved_session_size(terminal_size);
    let mut args: Vec<String> = vec![
        "new-session".to_string(),
        "-d".to_string(),
        "-x".to_string(),
        width.to_string(),
        "-y".to_string(),
        height.to_string(),
        "-s".to_string(),
        session_name.to_string(),
    ];
    if let Some(name) = window_name.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        args.extend(["-n".to_string(), name.to_string()]);
    }
    args.push("-c".to_string());
    args.push(project_root.to_string());
    args.extend(pane_placeholder_argv());

    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    tmux_run_ready(
        backend,
        &args_ref,
        &format!("failed to create tmux session {session_name:?}"),
        timeout_s,
    )
    .map(|_| ())
    .map_err(DaemonError::from)
}

fn resolved_session_size(terminal_size: Option<(i32, i32)>) -> (i32, i32) {
    let default = (DEFAULT_SESSION_WIDTH, DEFAULT_SESSION_HEIGHT);
    let (width, height) = match terminal_size {
        Some((w, h)) => (w, h),
        None => return default,
    };
    if width < MIN_SESSION_WIDTH || height < MIN_SESSION_HEIGHT {
        return default;
    }
    (width, height)
}

/// Build a tmux target string from session and optional window names.
pub fn session_window_target(session_name: &str, window_name: Option<&str>) -> Result<String> {
    let session_text = session_name.trim();
    if session_text.is_empty() {
        return Err(DaemonError::Config("session_name cannot be empty".to_string()));
    }
    let window_text = window_name.map(|s| s.trim()).unwrap_or("");
    if window_text.is_empty() {
        Ok(session_text.to_string())
    } else {
        Ok(format!("{session_text}:{window_text}"))
    }
}

/// List windows in `session_name`.
pub fn list_windows(
    backend: &Backend,
    session_name: &str,
    timeout_s: Option<f64>,
) -> Result<Vec<TmuxWindowRecord>> {
    let output = tmux_run_ready(
        backend,
        &[
            "list-windows",
            "-t",
            session_name,
            "-F",
            "#{window_id}\t#{window_name}\t#{window_active}",
        ],
        &format!("failed to list tmux windows for session {session_name:?}"),
        timeout_s,
    )
    .map_err(DaemonError::from)?;
    Ok(parse_list_windows_output(&output.stdout))
}

fn parse_list_windows_output(stdout: &str) -> Vec<TmuxWindowRecord> {
    let mut windows = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 3 {
            continue;
        }
        let window_id = parts[0].trim();
        let window_id = if window_id.is_empty() {
            None
        } else {
            Some(window_id.to_string())
        };
        let window_name = parts[1].trim();
        if window_name.is_empty() {
            continue;
        }
        let active = matches!(parts[2].trim(), "1" | "true" | "True");
        windows.push(TmuxWindowRecord::new(window_id, window_name.to_string(), active));
    }
    windows
}

/// Find a window by name, returning `None` if absent.
pub fn find_window(
    backend: &Backend,
    session_name: &str,
    window_name: &str,
    timeout_s: Option<f64>,
) -> Result<Option<TmuxWindowRecord>> {
    let target_name = window_name.trim();
    if target_name.is_empty() {
        return Ok(None);
    }
    let windows = list_windows(backend, session_name, timeout_s)?;
    Ok(windows.into_iter().find(|r| r.window_name == target_name))
}

/// Create a new window in `session_name`.
pub fn create_window(
    backend: &Backend,
    session_name: &str,
    window_name: &str,
    project_root: &str,
    select: bool,
    timeout_s: Option<f64>,
) -> Result<TmuxWindowRecord> {
    let args: Vec<String> = vec![
        "new-window".to_string(),
        "-d".to_string(),
        "-t".to_string(),
        session_name.to_string(),
        "-n".to_string(),
        window_name.to_string(),
        "-c".to_string(),
        project_root.to_string(),
    ]
    .into_iter()
    .chain(pane_placeholder_argv().into_iter())
    .collect();
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    tmux_run_ready(
        backend,
        &args_ref,
        &format!(
            "failed to create tmux window {window_name:?} for session {session_name:?}"
        ),
        timeout_s,
    )
    .map_err(DaemonError::from)?;

    let record = wait_for_window(backend, session_name, window_name, timeout_s)?;
    let record = record.ok_or_else(|| {
        DaemonError::Config(format!(
            "failed to resolve tmux window {window_name:?} for session {session_name:?}"
        ))
    })?;
    if select {
        select_window(
            backend,
            &session_window_target(session_name, record.window_id.as_deref())?,
        )?;
    }
    Ok(record)
}

/// Return an existing window or create it.
pub fn ensure_window(
    backend: &Backend,
    session_name: &str,
    window_name: &str,
    project_root: &str,
    select: bool,
    timeout_s: Option<f64>,
) -> Result<TmuxWindowRecord> {
    if let Some(record) = find_window(backend, session_name, window_name, timeout_s)? {
        if select {
            select_window(
                backend,
                &session_window_target(session_name, record.window_id.as_deref())?,
            )?;
        }
        return Ok(record);
    }
    create_window(backend, session_name, window_name, project_root, select, timeout_s)
}

/// Rename a window target.
pub fn rename_window(
    backend: &Backend,
    target: &str,
    new_name: &str,
    timeout_s: Option<f64>,
) -> Result<()> {
    tmux_run_ready(
        backend,
        &["rename-window", "-t", target, new_name],
        &format!("failed to rename tmux window target {target:?} to {new_name:?}"),
        timeout_s,
    )
    .map(|_| ())
    .map_err(DaemonError::from)?;

    let session_name = target.split(':').next().map(|s| s.trim()).unwrap_or("");
    if !session_name.is_empty()
        && wait_for_window(backend, session_name, new_name, timeout_s)?.is_none()
    {
        return Err(DaemonError::Config(format!(
            "failed to observe renamed tmux window {new_name:?} for session {session_name:?}"
        )));
    }
    Ok(())
}

/// Kill a window target.
pub fn kill_window(backend: &Backend, target: &str, timeout_s: Option<f64>) -> Result<()> {
    tmux_run_ready(
        backend,
        &["kill-window", "-t", target],
        &format!("failed to kill tmux window target {target:?}"),
        timeout_s,
    )
    .map(|_| ())
    .map_err(DaemonError::from)
}

/// Return `true` if `session_name` exists.
pub fn session_alive(
    backend: &Backend,
    session_name: &str,
    timeout_s: Option<f64>,
) -> Result<bool> {
    wait_until_ready(
        || session_alive_once(backend, session_name),
        &format!("failed to inspect tmux session {session_name:?}"),
        timeout_s,
    )
    .map_err(DaemonError::from)
}

/// Return the root pane id of `session_name`.
pub fn session_root_pane(
    backend: &Backend,
    session_name: &str,
    timeout_s: Option<f64>,
) -> Result<String> {
    window_root_pane(backend, session_name, timeout_s)
}

/// Return the root pane id of `target_window`.
pub fn window_root_pane(backend: &Backend, target_window: &str, timeout_s: Option<f64>) -> Result<String> {
    let pane_id = wait_for_root_pane(backend, target_window, timeout_s)?;
    if !pane_id.starts_with('%') {
        return Err(DaemonError::Config(format!(
            "failed to resolve root pane for tmux target {target_window:?}"
        )));
    }
    Ok(pane_id)
}

/// Split `target` and return the new pane id.
pub fn split_pane(
    backend: &Backend,
    target: &str,
    direction: &str,
    percent: i32,
    project_root: &str,
    timeout_s: Option<f64>,
) -> Result<String> {
    let pane_id = backend.split_pane(target, direction, percent, &pane_placeholder_cmd(), project_root)?;
    if pane_id.starts_with('%') {
        return Ok(pane_id);
    }
    let resolved = wait_for_root_pane(backend, target, timeout_s)?;
    if resolved.starts_with('%') {
        return Ok(resolved);
    }
    Err(DaemonError::Config(format!(
        "failed to split tmux pane from target {target:?}"
    )))
}

/// Kill the tmux server and clean up its socket file.
pub fn kill_server(backend: &Backend) -> bool {
    let _ = backend._tmux_run(&["kill-server"], false, true);
    let socket_path = backend.socket_path.trim();
    if socket_path.is_empty() {
        return true;
    }
    let path = std::path::Path::new(socket_path);
    if !path.exists() {
        return true;
    }
    for _ in 0..30 {
        if !path.exists() {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    if let Err(e) = std::fs::remove_file(path) {
        tracing::debug!("failed to remove tmux socket {socket_path}: {e}");
    }
    true
}

/// Poll until `window_name` exists in `session_name`.
pub fn wait_for_window(
    backend: &Backend,
    session_name: &str,
    window_name: &str,
    timeout_s: Option<f64>,
) -> Result<Option<TmuxWindowRecord>> {
    wait_until(
        || match find_window(backend, session_name, window_name, None) {
            Ok(record) => Ok(record),
            Err(DaemonError::Config(msg)) if is_tmux_transient_server_error_text(&msg) => {
                Err(BackendError::Transient(msg))
            }
            Err(e) => Err(BackendError::Command(e.to_string())),
        },
        Some(&format!(
            "failed to observe tmux window {window_name:?} for session {session_name:?}"
        )),
        timeout_s,
    )
    .map_err(DaemonError::from)
}

/// Select a window target.
pub fn select_window(backend: &Backend, target: &str) -> Result<()> {
    wait_until_ready(
        || {
            tmux_run_ready(
                backend,
                &["select-window", "-t", target],
                &format!("failed to select tmux window target {target:?}"),
                Some(0.0),
            )
        },
        &format!("failed to select tmux window target {target:?}"),
        Some(0.0),
    )
    .map(|_| ())
    .map_err(DaemonError::from)
}

/// Poll until the root pane of `target_window` can be resolved.
pub fn wait_for_root_pane(
    backend: &Backend,
    target_window: &str,
    timeout_s: Option<f64>,
) -> Result<String> {
    let pane_id = wait_until(
        || root_pane_once(backend, target_window),
        Some(&format!(
            "failed to resolve root pane for tmux target {target_window:?}"
        )),
        timeout_s,
    )
    .map_err(DaemonError::from)?;
    pane_id.ok_or_else(|| {
        DaemonError::Config(format!(
            "failed to resolve root pane for tmux target {target_window:?}"
        ))
    })
}

fn root_pane_once(backend: &Backend, target_window: &str) -> BackendResult<Option<String>> {
    let output = match tmux_run_once(backend, &["list-panes", "-t", target_window, "-F", "#{pane_id}"]) {
        Some(out) => out,
        None => return Ok(None),
    };
    let pane_id = output.stdout.lines().map(|l| l.trim()).find(|l| !l.is_empty());
    Ok(pane_id.map(|s| s.to_string()))
}

fn tmux_run_ready(
    backend: &Backend,
    args: &[&str],
    failure_message: &str,
    timeout_s: Option<f64>,
) -> BackendResult<TmuxOutput> {
    wait_until_ready(
        || tmux_run_checked(backend, args),
        failure_message,
        timeout_s,
    )
}

fn tmux_run_once(backend: &Backend, args: &[&str]) -> Option<TmuxOutput> {
    match tmux_run_checked(backend, args) {
        Ok(output) => Some(output),
        Err(BackendError::Transient(_)) => None,
        Err(_) => None,
    }
}

fn tmux_run_checked(backend: &Backend, args: &[&str]) -> BackendResult<TmuxOutput> {
    let output = backend._tmux_run(args, false, true)?;
    if output.success() {
        return Ok(output);
    }
    let detail = tmux_failure_detail(&output.stderr, &output.stdout, &[]);
    let socket_path = backend.socket_path();
    let command = backend.tmux_base();
    let args_owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    if is_tmux_transient_server_error_text(&detail) {
        return Err(BackendError::Transient(tmux_command_failure_message(
            "tmux server unavailable",
            Some(&args_owned),
            Some(&detail),
            socket_path,
            Some(&command),
        )));
    }
    Err(BackendError::Command(tmux_command_failure_message(
        "tmux command failed",
        Some(&args_owned),
        Some(&detail),
        socket_path,
        Some(&command),
    )))
}

fn wait_until<T>(
    probe: impl Fn() -> BackendResult<Option<T>>,
    failure_message: Option<&str>,
    timeout_s: Option<f64>,
) -> BackendResult<Option<T>> {
    let deadline = Instant::now() + Duration::from_secs_f64(tmux_object_ready_timeout_s(timeout_s));
    let mut last_transient: Option<BackendError> = None;
    loop {
        match probe() {
            Ok(Some(value)) => return Ok(Some(value)),
            Ok(None) => {}
            Err(err @ BackendError::Transient(_)) => last_transient = Some(err),
            Err(err) => return Err(err),
        }
        if Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_secs_f64(tmux_object_ready_poll_interval_s()));
    }
    if let Some(err) = last_transient {
        if let Some(msg) = failure_message {
            return Err(BackendError::Transient(tmux_command_failure_message(
                msg,
                None,
                Some(&err.to_string()),
                None,
                None,
            )));
        }
    }
    Ok(None)
}

fn wait_until_ready<T>(
    action: impl Fn() -> BackendResult<T>,
    failure_message: &str,
    timeout_s: Option<f64>,
) -> BackendResult<T> {
    let deadline = Instant::now() + Duration::from_secs_f64(tmux_object_ready_timeout_s(timeout_s));
    let last_error = loop {
        match action() {
            Ok(value) => return Ok(value),
            Err(err) => {
                if Instant::now() >= deadline {
                    break Some(err);
                }
            }
        }
        thread::sleep(Duration::from_secs_f64(tmux_object_ready_poll_interval_s()));
    };
    if let Some(err) = last_error {
        let detail = err.to_string();
        let msg = tmux_command_failure_message(failure_message, None, Some(&detail), None, None);
        return match err {
            BackendError::Transient(_) => Err(BackendError::Transient(msg)),
            _ => Err(BackendError::Command(msg)),
        };
    }
    Err(BackendError::Command(failure_message.to_string()))
}

fn session_alive_once(backend: &Backend, session_name: &str) -> BackendResult<bool> {
    let output = backend._tmux_run(&["has-session", "-t", session_name], false, true)?;
    if output.success() {
        return Ok(true);
    }
    let detail = tmux_failure_detail(&output.stderr, &output.stdout, &[]);
    if is_tmux_absent_server_text(&detail) {
        return Ok(false);
    }
    if is_tmux_transient_server_error_text(&detail) {
        return Err(BackendError::Transient(detail));
    }
    if detail.is_empty() || is_tmux_missing_session_text(&detail) {
        return Ok(false);
    }
    Err(BackendError::Command(detail))
}

fn tmux_object_ready_timeout_s(timeout_s: Option<f64>) -> f64 {
    let value = timeout_s.unwrap_or_else(|| {
        std::env::var("CCB_TMUX_OBJECT_READY_TIMEOUT_S")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(TMUX_OBJECT_READY_TIMEOUT_S)
    });
    value.max(0.0)
}

fn tmux_object_ready_poll_interval_s() -> f64 {
    let value = std::env::var("CCB_TMUX_OBJECT_READY_POLL_INTERVAL_S")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(TMUX_OBJECT_READY_POLL_INTERVAL_S);
    value.max(0.0)
}

/// Internal result type used while classifying tmux failures.
pub type BackendResult<T> = std::result::Result<T, BackendError>;

/// Internal error type that distinguishes transient server unavailability.
#[derive(Debug)]
pub enum BackendError {
    Transient(String),
    Command(String),
    Io(std::io::Error),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::Transient(msg) => write!(f, "{msg}"),
            BackendError::Command(msg) => write!(f, "{msg}"),
            BackendError::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for BackendError {}

impl From<BackendError> for DaemonError {
    fn from(err: BackendError) -> Self {
        match err {
            BackendError::Io(io_err) => DaemonError::Io(io_err),
            BackendError::Transient(msg) | BackendError::Command(msg) => {
                DaemonError::Config(msg)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolved_session_size_defaults() {
        assert_eq!(
            resolved_session_size(None),
            (DEFAULT_SESSION_WIDTH, DEFAULT_SESSION_HEIGHT)
        );
    }

    #[test]
    fn test_resolved_session_size_valid() {
        assert_eq!(resolved_session_size(Some((100, 60))), (100, 60));
    }

    #[test]
    fn test_resolved_session_size_too_small() {
        assert_eq!(
            resolved_session_size(Some((10, 10))),
            (DEFAULT_SESSION_WIDTH, DEFAULT_SESSION_HEIGHT)
        );
        assert_eq!(
            resolved_session_size(Some((39, 60))),
            (DEFAULT_SESSION_WIDTH, DEFAULT_SESSION_HEIGHT)
        );
        assert_eq!(
            resolved_session_size(Some((100, 14))),
            (DEFAULT_SESSION_WIDTH, DEFAULT_SESSION_HEIGHT)
        );
    }

    #[test]
    fn test_session_window_target() {
        assert_eq!(session_window_target("sess", None).unwrap(), "sess");
        assert_eq!(
            session_window_target("sess", Some("win")).unwrap(),
            "sess:win"
        );
        assert_eq!(session_window_target("sess", Some("")).unwrap(), "sess");
        assert!(session_window_target("", None).is_err());
    }

    #[test]
    fn test_parse_list_windows_output() {
        let stdout = "@0\tcontrol\t1\n@1\tworkspace\t0\n@2\t\t0\nmalformed\n";
        let windows = parse_list_windows_output(stdout);
        assert_eq!(windows.len(), 2);
        assert_eq!(
            windows[0],
            TmuxWindowRecord::new(Some("@0".to_string()), "control".to_string(), true)
        );
        assert_eq!(
            windows[1],
            TmuxWindowRecord::new(Some("@1".to_string()), "workspace".to_string(), false)
        );
    }

    #[test]
    fn test_tmux_object_ready_timeout_s() {
        assert_eq!(tmux_object_ready_timeout_s(Some(1.5)), 1.5);
        assert_eq!(tmux_object_ready_timeout_s(Some(-1.0)), 0.0);
        assert!(tmux_object_ready_timeout_s(None) >= TMUX_OBJECT_READY_TIMEOUT_S);
    }

    #[test]
    fn test_tmux_object_ready_poll_interval_s() {
        assert!(tmux_object_ready_poll_interval_s() >= 0.0);
    }

    #[test]
    fn test_build_backend() {
        let factory = BackendFactory::default();
        let backend = build_backend(&factory, "/tmp/ccb-test.sock").unwrap();
        assert_eq!(backend.socket_path, "/tmp/ccb-test.sock");
        assert!(backend.session_name.is_empty());
        assert!(backend.socket_path().is_some());
    }

    #[test]
    fn test_backend_error_into_daemon_error() {
        let err = BackendError::Command("boom".to_string());
        let daemon_err: DaemonError = err.into();
        assert!(matches!(daemon_err, DaemonError::Config(_)));

        let err = BackendError::Transient("retry".to_string());
        let daemon_err: DaemonError = err.into();
        assert!(matches!(daemon_err, DaemonError::Config(_)));
    }

    #[test]
    #[ignore = "requires a running tmux server"]
    fn test_real_session_lifecycle() {
        let factory = BackendFactory::default();
        let backend = build_backend(&factory, "/tmp/ccb-test-real.sock").unwrap();
        prepare_server(&backend, Some(5.0)).unwrap();
        assert!(session_alive(&backend, "test-session", Some(5.0)).unwrap());
    }
}
