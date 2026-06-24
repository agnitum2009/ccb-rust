//! Mirrors Python `lib/cli/services/start_foreground.py`.

use crate::context::CliContext;
use crate::services::daemon_runtime::policy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

const ATTACH_ESTABLISH_TIMEOUT_S: f64 = 1.5;
const ATTACH_ESTABLISH_POLL_INTERVAL_S: f64 = 0.05;
const ATTACH_TARGET_READY_POLL_INTERVAL_S: f64 = 0.05;
const MIN_ATTACH_RPC_TIMEOUT_S: f64 = 0.1;

/// Public env-overridable constants for tests.
pub static mut ATTACH_TARGET_READY_TIMEOUT_S_OVERRIDE: Option<f64> = None;
pub static mut FOREGROUND_ATTACH_RPC_TIMEOUT_S_OVERRIDE: Option<f64> = None;
pub static mut ATTACH_TARGET_READY_POLL_INTERVAL_S_OVERRIDE: Option<f64> = None;

fn attach_target_ready_timeout_s() -> f64 {
    unsafe {
        ATTACH_TARGET_READY_TIMEOUT_S_OVERRIDE
            .unwrap_or_else(policy::foreground_attach_target_ready_timeout_s)
    }
}

fn foreground_attach_rpc_timeout_s() -> f64 {
    unsafe {
        FOREGROUND_ATTACH_RPC_TIMEOUT_S_OVERRIDE
            .unwrap_or_else(policy::foreground_attach_rpc_timeout_s)
    }
}

fn attach_target_ready_poll_interval_s() -> f64 {
    unsafe {
        ATTACH_TARGET_READY_POLL_INTERVAL_S_OVERRIDE.unwrap_or(ATTACH_TARGET_READY_POLL_INTERVAL_S)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForegroundAttachSummary {
    pub project_id: String,
    pub tmux_socket_path: String,
    pub tmux_session_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForegroundAttachError(pub String);

impl fmt::Display for ForegroundAttachError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ForegroundAttachError {}

/// Minimal client interface needed for foreground attach.
pub trait AttachClient: Send + Sync {
    fn ping(&self, target: &str) -> Result<serde_json::Value, String>;
    fn with_timeout(&self, timeout_s: f64) -> Box<dyn AttachClient>;
}

impl AttachClient for crate::ccbd::CcbdClient {
    fn ping(&self, target: &str) -> Result<serde_json::Value, String> {
        crate::ccbd::CcbdClient::ping(self, target).map_err(|e| e.to_string())
    }
    fn with_timeout(&self, timeout_s: f64) -> Box<dyn AttachClient> {
        Box::new(crate::ccbd::CcbdClient::with_timeout(self, timeout_s))
    }
}

/// Minimal child-process interface for foreground attach.
pub trait AttachChild: Send + Sync {
    fn pid(&self) -> u32;
    fn poll(&mut self) -> Option<i32>;
    fn wait(&mut self) -> Result<i32, String>;
}

impl AttachChild for std::process::Child {
    fn pid(&self) -> u32 {
        self.id()
    }

    fn poll(&mut self) -> Option<i32> {
        match self.try_wait() {
            Ok(Some(status)) => status.code(),
            _ => None,
        }
    }

    fn wait(&mut self) -> Result<i32, String> {
        self.wait()
            .map_err(|e| e.to_string())
            .map(|s| s.code().unwrap_or(0))
    }
}

/// Subprocess operations used during foreground attach.
pub trait AttachRuntime: Send + Sync {
    fn which_tmux(&self) -> Option<String>;
    fn tmux_base(&self, socket_path: &str) -> Vec<String>;
    fn run(&self, args: &[String], env: &HashMap<String, String>) -> Result<RunOutcome, String>;
    fn popen(
        &self,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Box<dyn AttachChild>, String>;
    fn sleep(&self, seconds: f64);
    fn monotonic(&self) -> f64;
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunOutcome {
    pub returncode: i32,
    pub stdout: String,
}

/// Default runtime that invokes real subprocesses.
pub struct DefaultAttachRuntime;

impl DefaultAttachRuntime {
    fn find_tmux() -> Option<String> {
        std::env::var("PATH")
            .ok()
            .and_then(|path| {
                path.split(':')
                    .map(|dir| std::path::Path::new(dir).join("tmux"))
                    .find(|p| p.exists())
            })
            .map(|p| p.to_string_lossy().to_string())
    }
}

impl AttachRuntime for DefaultAttachRuntime {
    fn which_tmux(&self) -> Option<String> {
        Self::find_tmux()
    }

    fn tmux_base(&self, socket_path: &str) -> Vec<String> {
        vec![
            "tmux".to_string(),
            "-S".to_string(),
            socket_path.to_string(),
        ]
    }

    fn run(&self, args: &[String], env: &HashMap<String, String>) -> Result<RunOutcome, String> {
        let output = Command::new(&args[0])
            .args(&args[1..])
            .envs(env)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .map_err(|e| e.to_string())?;
        Ok(RunOutcome {
            returncode: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        })
    }

    fn popen(
        &self,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Box<dyn AttachChild>, String> {
        let child = Command::new(&args[0])
            .args(&args[1..])
            .envs(env)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(Box::new(child))
    }

    fn sleep(&self, seconds: f64) {
        thread::sleep(Duration::from_secs_f64(seconds));
    }

    fn monotonic(&self) -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
    }
}

/// Attach the current terminal to the project's namespace tmux session.
///
/// Mirrors Python `attach_started_project_namespace(context)`.
pub fn attach_started_project_namespace(
    context: &CliContext,
) -> Result<ForegroundAttachSummary, ForegroundAttachError> {
    let runtime = DefaultAttachRuntime;
    let client = build_foreground_attach_client(context, &runtime)?;
    attach_started_project_namespace_with(context, &runtime, client)
}

pub fn attach_started_project_namespace_with<C>(
    context: &CliContext,
    runtime: &dyn AttachRuntime,
    client: C,
) -> Result<ForegroundAttachSummary, ForegroundAttachError>
where
    C: AttachClient + 'static,
{
    if runtime.which_tmux().is_none() {
        return Err(ForegroundAttachError(
            "tmux is required for interactive `ccb`".into(),
        ));
    }

    let env = attach_env();
    let payload = wait_for_attach_target(&client, &env, runtime)?;

    let tmux_socket_path = payload
        .get("namespace_tmux_socket_path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let tmux_session_name = payload
        .get("namespace_tmux_session_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let summary = ForegroundAttachSummary {
        project_id: context.project.project_id.clone(),
        tmux_socket_path: tmux_socket_path.clone(),
        tmux_session_name: tmux_session_name.clone(),
    };

    let mut attach = runtime
        .popen(
            &tmux_cmd(
                &tmux_socket_path,
                &["attach-session", "-t", &tmux_session_name],
            ),
            &env,
        )
        .map_err(|e| ForegroundAttachError(format!("failed to spawn tmux attach: {e}")))?;

    let attached = wait_for_attach_established(
        attach.as_mut(),
        &tmux_socket_path,
        &tmux_session_name,
        &env,
        runtime,
    );

    if attached {
        best_effort_refresh_attached_client(
            &tmux_socket_path,
            &tmux_session_name,
            attach.pid(),
            &env,
            runtime,
        );
    }

    let returncode = attach.wait().unwrap_or(1);

    if attached {
        if !tmux_has_session(&tmux_socket_path, &tmux_session_name, &env, runtime) {
            best_effort_stop_backend_after_namespace_exit(context, runtime);
        }
        return Ok(summary);
    }

    if returncode != 0 && !tmux_has_session(&tmux_socket_path, &tmux_session_name, &env, runtime) {
        return Err(ForegroundAttachError(
            "project namespace session exited before foreground attach completed".into(),
        ));
    }

    Err(ForegroundAttachError(
        "failed to attach project namespace after successful `ccb` start".into(),
    ))
}

fn build_foreground_attach_client(
    context: &CliContext,
    _runtime: &dyn AttachRuntime,
) -> Result<crate::ccbd::CcbdClient, ForegroundAttachError> {
    Ok(
        crate::ccbd::CcbdClient::new(context.paths.ccbd_socket_path())
            .with_timeout(foreground_attach_rpc_timeout_s()),
    )
}

fn attach_env() -> HashMap<String, String> {
    let mut env = ccb_terminal::env::isolated_tmux_env();
    env.remove("TMUX");
    env.remove("TMUX_PANE");
    env
}

fn tmux_cmd(socket_path: &str, args: &[&str]) -> Vec<String> {
    let mut cmd = vec![
        "tmux".to_string(),
        "-S".to_string(),
        socket_path.to_string(),
    ];
    cmd.extend(args.iter().map(|s| s.to_string()));
    cmd
}

fn wait_for_attach_target<C: AttachClient>(
    client: &C,
    env: &HashMap<String, String>,
    runtime: &dyn AttachRuntime,
) -> Result<serde_json::Value, ForegroundAttachError> {
    let deadline = runtime.monotonic() + attach_target_ready_timeout_s();
    let mut attempts = 0;
    let mut ping_successes = 0;
    let mut last_error = attach_target_unavailable_error(attempts, attach_target_ready_timeout_s());

    loop {
        let remaining_s = deadline - runtime.monotonic();
        if remaining_s < MIN_ATTACH_RPC_TIMEOUT_S {
            return Err(ForegroundAttachError(last_error));
        }
        let attempt_timeout_s = foreground_attach_rpc_timeout_s().min(remaining_s);
        attempts += 1;
        match client.with_timeout(attempt_timeout_s).ping("ccbd") {
            Ok(payload) => {
                ping_successes += 1;
                let (ready, error) = attach_target_ready(&payload, env, runtime)?;
                if ready {
                    return Ok(payload);
                }
                last_error = attach_namespace_timeout_error(
                    &error,
                    attempts,
                    ping_successes,
                    attach_target_ready_timeout_s(),
                );
            }
            Err(err) => {
                last_error = attach_ping_timeout_error(
                    &err,
                    attempts,
                    attach_target_ready_timeout_s(),
                    attempt_timeout_s,
                );
            }
        }

        if runtime.monotonic() >= deadline {
            return Err(ForegroundAttachError(last_error));
        }
        let sleep_s =
            attach_target_ready_poll_interval_s().min((deadline - runtime.monotonic()).max(0.0));
        runtime.sleep(sleep_s);
    }
}

fn attach_target_ready(
    payload: &serde_json::Value,
    env: &HashMap<String, String>,
    runtime: &dyn AttachRuntime,
) -> Result<(bool, String), ForegroundAttachError> {
    let tmux_socket_path = payload
        .get("namespace_tmux_socket_path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let tmux_session_name = payload
        .get("namespace_tmux_session_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let workspace_window_name = payload
        .get("namespace_workspace_window_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let ui_attachable = payload
        .get("namespace_ui_attachable")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if tmux_socket_path.is_empty() || tmux_session_name.is_empty() || !ui_attachable {
        return Ok((
            false,
            "project namespace is not attachable after successful `ccb` start".into(),
        ));
    }
    if !tmux_has_session(&tmux_socket_path, &tmux_session_name, env, runtime) {
        return Ok((
            false,
            "project namespace session is missing after successful `ccb` start".into(),
        ));
    }
    if !workspace_window_name.is_empty()
        && !tmux_select_window(
            &tmux_socket_path,
            &format!("{tmux_session_name}:{workspace_window_name}"),
            env,
            runtime,
        )
    {
        return Ok((
            false,
            "project namespace workspace window is missing after successful `ccb` start".into(),
        ));
    }
    Ok((true, String::new()))
}

fn attach_target_unavailable_error(attempts: i32, timeout_s: f64) -> String {
    format!(
        "foreground attach timed out: project namespace did not become attachable within {:.1}s after successful `ccb` start (attempts={})",
        timeout_s, attempts
    )
}

fn attach_ping_timeout_error(
    err: &str,
    attempts: i32,
    timeout_s: f64,
    rpc_timeout_s: f64,
) -> String {
    let detail = err.trim();
    let detail = if detail.is_empty() {
        "CcbdClientError"
    } else {
        detail
    };
    format!(
        "foreground attach timed out: ccbd did not respond to ping within {:.1}s after successful `ccb` start (rpc_timeout={:.1}s, attempts={}, last_error={})",
        timeout_s, rpc_timeout_s, attempts, detail
    )
}

fn attach_namespace_timeout_error(
    error: &str,
    attempts: i32,
    ping_successes: i32,
    timeout_s: f64,
) -> String {
    let detail = error.trim();
    let detail = if detail.is_empty() {
        "project namespace is not attachable"
    } else {
        detail
    };
    format!(
        "foreground attach timed out: ccbd is responsive but project namespace was not attachable within {:.1}s after successful `ccb` start (attempts={}, ping_successes={}, last_error={})",
        timeout_s, attempts, ping_successes, detail
    )
}

fn wait_for_attach_established(
    attach: &mut dyn AttachChild,
    tmux_socket_path: &str,
    tmux_session_name: &str,
    env: &HashMap<String, String>,
    runtime: &dyn AttachRuntime,
) -> bool {
    let deadline = runtime.monotonic() + ATTACH_ESTABLISH_TIMEOUT_S;
    loop {
        if tmux_client_pid_attached(
            tmux_socket_path,
            tmux_session_name,
            attach.pid(),
            env,
            runtime,
        ) {
            return true;
        }
        if attach.poll().is_some() {
            return false;
        }
        if runtime.monotonic() >= deadline {
            return true;
        }
        runtime.sleep(ATTACH_ESTABLISH_POLL_INTERVAL_S);
    }
}

fn tmux_client_pid_attached(
    tmux_socket_path: &str,
    tmux_session_name: &str,
    client_pid: u32,
    env: &HashMap<String, String>,
    runtime: &dyn AttachRuntime,
) -> bool {
    tmux_list_client_pids(tmux_socket_path, tmux_session_name, env, runtime)
        .contains(&(client_pid as i64))
}

fn tmux_list_client_pids(
    tmux_socket_path: &str,
    tmux_session_name: &str,
    env: &HashMap<String, String>,
    runtime: &dyn AttachRuntime,
) -> Vec<i64> {
    let result = runtime.run(
        &tmux_cmd(
            tmux_socket_path,
            &[
                "list-clients",
                "-t",
                tmux_session_name,
                "-F",
                "#{client_pid}",
            ],
        ),
        env,
    );
    match result {
        Ok(outcome) if outcome.returncode == 0 => outcome
            .stdout
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .filter_map(|line| line.parse::<i64>().ok())
            .collect(),
        _ => Vec::new(),
    }
}

fn tmux_client_tty(
    tmux_socket_path: &str,
    tmux_session_name: &str,
    client_pid: u32,
    env: &HashMap<String, String>,
    runtime: &dyn AttachRuntime,
) -> Option<String> {
    let result = runtime.run(
        &tmux_cmd(
            tmux_socket_path,
            &[
                "list-clients",
                "-t",
                tmux_session_name,
                "-F",
                "#{client_pid}\t#{client_tty}",
            ],
        ),
        env,
    );
    match result {
        Ok(outcome) if outcome.returncode == 0 => outcome
            .stdout
            .lines()
            .filter_map(|line| {
                let (pid_text, tty_text) = line.split_once('\t')?;
                let pid = pid_text.trim().parse::<i64>().ok()?;
                if pid != client_pid as i64 {
                    return None;
                }
                let tty = tty_text.trim();
                if tty.is_empty() {
                    None
                } else {
                    Some(tty.to_string())
                }
            })
            .next(),
        _ => None,
    }
}

fn best_effort_refresh_attached_client(
    tmux_socket_path: &str,
    tmux_session_name: &str,
    client_pid: u32,
    env: &HashMap<String, String>,
    runtime: &dyn AttachRuntime,
) {
    let Some(client_tty) = tmux_client_tty(
        tmux_socket_path,
        tmux_session_name,
        client_pid,
        env,
        runtime,
    ) else {
        return;
    };
    let _ = runtime.run(
        &tmux_cmd(tmux_socket_path, &["refresh-client", "-t", &client_tty]),
        env,
    );
}

fn best_effort_stop_backend_after_namespace_exit(
    context: &CliContext,
    _runtime: &dyn AttachRuntime,
) {
    crate::services::daemon::record_shutdown_intent(context, "foreground_session_exit");
    let _ = crate::ccbd::CcbdClient::new(context.paths.ccbd_socket_path())
        .with_timeout(foreground_attach_rpc_timeout_s())
        .request("stop-all", &serde_json::json!({"force": false}));
}

fn tmux_has_session(
    tmux_socket_path: &str,
    tmux_session_name: &str,
    env: &HashMap<String, String>,
    runtime: &dyn AttachRuntime,
) -> bool {
    matches!(
        runtime.run(
            &tmux_cmd(tmux_socket_path, &["has-session", "-t", tmux_session_name]),
            env,
        ),
        Ok(outcome) if outcome.returncode == 0
    )
}

fn tmux_select_window(
    tmux_socket_path: &str,
    target: &str,
    env: &HashMap<String, String>,
    runtime: &dyn AttachRuntime,
) -> bool {
    matches!(
        runtime.run(
            &tmux_cmd(tmux_socket_path, &["select-window", "-t", target]),
            env,
        ),
        Ok(outcome) if outcome.returncode == 0
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    struct FakeChild {
        pid: u32,
        returncode: Option<i32>,
        waited: AtomicBool,
    }

    impl AttachChild for FakeChild {
        fn pid(&self) -> u32 {
            self.pid
        }
        fn poll(&mut self) -> Option<i32> {
            self.returncode
        }
        fn wait(&mut self) -> Result<i32, String> {
            self.waited.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(self.returncode.unwrap_or(0))
        }
    }

    #[derive(Default)]
    struct FakeRuntime {
        monotonic_value: Mutex<f64>,
        run_responses: Mutex<HashMap<String, RunOutcome>>,
        popen_response: Mutex<Option<Box<dyn AttachChild>>>,
    }

    impl FakeRuntime {
        fn advance(&self, seconds: f64) {
            *self.monotonic_value.lock().unwrap() += seconds;
        }
    }

    impl AttachRuntime for FakeRuntime {
        fn which_tmux(&self) -> Option<String> {
            Some("/usr/bin/tmux".into())
        }

        fn tmux_base(&self, socket_path: &str) -> Vec<String> {
            vec!["tmux".into(), "-S".into(), socket_path.into()]
        }

        fn run(
            &self,
            args: &[String],
            _env: &HashMap<String, String>,
        ) -> Result<RunOutcome, String> {
            let key = args.join(" ");
            Ok(self
                .run_responses
                .lock()
                .unwrap()
                .remove(&key)
                .unwrap_or(RunOutcome {
                    returncode: 0,
                    stdout: String::new(),
                }))
        }

        fn popen(
            &self,
            _args: &[String],
            _env: &HashMap<String, String>,
        ) -> Result<Box<dyn AttachChild>, String> {
            self.popen_response
                .lock()
                .unwrap()
                .take()
                .ok_or_else(|| "no popen response".into())
        }

        fn sleep(&self, seconds: f64) {
            self.advance(seconds);
        }

        fn monotonic(&self) -> f64 {
            *self.monotonic_value.lock().unwrap()
        }
    }

    struct FakeClient {
        responses: Arc<Mutex<Vec<Result<serde_json::Value, String>>>>,
    }

    impl Clone for FakeClient {
        fn clone(&self) -> Self {
            Self {
                responses: Arc::clone(&self.responses),
            }
        }
    }

    impl AttachClient for FakeClient {
        fn ping(&self, target: &str) -> Result<serde_json::Value, String> {
            assert_eq!(target, "ccbd");
            self.responses.lock().unwrap().remove(0)
        }
        fn with_timeout(&self, _timeout_s: f64) -> Box<dyn AttachClient> {
            Box::new(self.clone())
        }
    }

    fn make_context(tmp: &tempfile::TempDir) -> CliContext {
        let project_root = tmp.path().to_path_buf();
        let ccb_dir = project_root.join(".ccb");
        std::fs::create_dir_all(&ccb_dir).unwrap();
        std::fs::write(ccb_dir.join("ccb.config"), "demo:codex\n").unwrap();
        let command =
            crate::models::ParsedCommand::Start(crate::models_start::ParsedStartCommand {
                project: None,
                agent_names: Vec::new(),
                restore: true,
                auto_permission: true,
                reset_context: false,
                kind: "start".into(),
            });
        crate::context::CliContextBuilder::new(command)
            .cwd(project_root.clone())
            .build()
            .unwrap()
    }

    #[test]
    fn attach_env_normalizes_ghostty_term_for_tmux() {
        std::env::set_var("TERM", "xterm-ghostty");
        std::env::set_var("TMUX", "/tmp/tmux-1000/default,123,0");
        std::env::set_var("TMUX_PANE", "%77");
        let env = attach_env();
        assert_eq!(env.get("TERM"), Some(&"xterm-256color".to_string()));
        assert!(!env.contains_key("TMUX"));
        assert!(!env.contains_key("TMUX_PANE"));
        std::env::remove_var("TERM");
        std::env::remove_var("TMUX");
        std::env::remove_var("TMUX_PANE");
    }

    #[test]
    fn attach_started_project_namespace_happy_path() {
        let tmp = tempfile::tempdir().unwrap();
        let context = make_context(&tmp);
        let runtime = FakeRuntime::default();
        runtime.advance(0.0);

        let socket_path = context.paths.ccbd_tmux_socket_path().to_string();
        let session_name = context.paths.ccbd_tmux_session_name();
        let workspace_window = context.paths.ccbd_tmux_workspace_window_name();

        runtime.run_responses.lock().unwrap().insert(
            format!("tmux -S {socket_path} has-session -t {session_name}"),
            RunOutcome {
                returncode: 0,
                stdout: String::new(),
            },
        );
        runtime.run_responses.lock().unwrap().insert(
            format!("tmux -S {socket_path} select-window -t {session_name}:{workspace_window}"),
            RunOutcome {
                returncode: 0,
                stdout: String::new(),
            },
        );
        runtime.run_responses.lock().unwrap().insert(
            format!("tmux -S {socket_path} list-clients -t {session_name} -F #{{client_pid}}"),
            RunOutcome {
                returncode: 0,
                stdout: "4242\n".into(),
            },
        );
        runtime.run_responses.lock().unwrap().insert(
            format!("tmux -S {socket_path} list-clients -t {session_name} -F #{{client_pid}}\\t#{{client_tty}}"),
            RunOutcome { returncode: 0, stdout: "4242\t/dev/pts/55\n".into() },
        );
        runtime.run_responses.lock().unwrap().insert(
            format!("tmux -S {socket_path} refresh-client -t /dev/pts/55"),
            RunOutcome {
                returncode: 0,
                stdout: String::new(),
            },
        );
        runtime.run_responses.lock().unwrap().insert(
            format!("tmux -S {socket_path} has-session -t {session_name}"),
            RunOutcome {
                returncode: 0,
                stdout: String::new(),
            },
        );

        *runtime.popen_response.lock().unwrap() = Some(Box::new(FakeChild {
            pid: 4242,
            returncode: Some(0),
            waited: AtomicBool::new(false),
        }));

        let client = FakeClient {
            responses: Arc::new(Mutex::new(vec![Ok(json!({
                "namespace_tmux_socket_path": socket_path,
                "namespace_tmux_session_name": session_name,
                "namespace_workspace_window_name": workspace_window,
                "namespace_ui_attachable": true,
            }))])),
        };

        let summary = attach_started_project_namespace_with(&context, &runtime, client).unwrap();
        assert_eq!(summary.project_id, context.project.project_id);
        assert_eq!(summary.tmux_socket_path, socket_path);
        assert_eq!(summary.tmux_session_name, session_name);
    }

    #[test]
    fn attach_started_project_namespace_requires_attachable() {
        unsafe {
            ATTACH_TARGET_READY_TIMEOUT_S_OVERRIDE = Some(0.1);
        }
        let tmp = tempfile::tempdir().unwrap();
        let context = make_context(&tmp);
        let runtime = FakeRuntime::default();
        runtime.advance(0.0);

        let client = FakeClient {
            responses: Arc::new(Mutex::new(vec![
                Ok(json!({
                    "namespace_tmux_socket_path": "",
                    "namespace_tmux_session_name": "",
                    "namespace_workspace_window_name": "",
                    "namespace_ui_attachable": false,
                })),
                Ok(json!({
                    "namespace_tmux_socket_path": "",
                    "namespace_tmux_session_name": "",
                    "namespace_workspace_window_name": "",
                    "namespace_ui_attachable": false,
                })),
            ])),
        };

        let err = attach_started_project_namespace_with(&context, &runtime, client).unwrap_err();
        assert!(err.0.contains("not attachable"));
        unsafe {
            ATTACH_TARGET_READY_TIMEOUT_S_OVERRIDE = None;
        }
    }
}
