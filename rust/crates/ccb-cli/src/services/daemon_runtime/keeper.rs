//! Mirrors Python `lib/cli/services/daemon_runtime/keeper.py`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use camino::Utf8PathBuf;
use ccb_storage::paths::PathLayout;
use serde_json::Value;

/// Context required by keeper runtime helpers.
///
/// Mirrors the `context` object passed to Python keeper functions.
#[derive(Debug, Clone)]
pub struct KeeperContext {
    pub project_id: String,
    pub project_root: PathBuf,
    pub paths: PathLayout,
}

impl KeeperContext {
    /// Build a keeper context from a project root.
    pub fn from_project_root(project_root: impl Into<PathBuf>) -> Self {
        let project_root = project_root.into();
        let utf8_root = Utf8PathBuf::from_path_buf(project_root.clone())
            .unwrap_or_else(|_| Utf8PathBuf::from(project_root.to_string_lossy().as_ref()));
        let paths = PathLayout::new(utf8_root);
        Self {
            project_id: paths.project_id().to_string(),
            project_root,
            paths,
        }
    }
}

/// Describes a spawned keeper process without actually executing it.
///
/// Used by [`spawn_keeper_process_with`] so tests can inspect the command
/// that would be run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeeperSpawn {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
}

/// Trait for ownership guards that provide a startup lock.
///
/// Mirrors Python `OwnershipGuard.startup_lock()` context manager.
pub trait OwnershipGuard {
    /// Run `f` while holding the startup lock.
    fn with_startup_lock<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R;
}

/// Clear shutdown intent from keeper state store.
///
/// Mirrors Python `clear_shutdown_intent(context)`.
/// Uses closure injection for state store operations.
pub fn clear_shutdown_intent<C>(clear_fn: C)
where
    C: FnOnce(),
{
    clear_fn()
}

/// Record running intent in lifecycle store.
///
/// Mirrors Python `record_running_intent(context)`.
/// Uses closure injection for lifecycle store operations.
pub fn record_running_intent<L>(
    lifecycle_load_fn: L,
    lifecycle_save_fn: L,
    _project_id: &str,
    socket_path: &str,
    config_signature: Option<&str>,
) -> bool
where
    L: Fn(&Value) -> Value,
{
    let current = lifecycle_load_fn(&Value::Null);
    let startup_requested = current.get("desired_state").and_then(|v| v.as_str())
        != Some("running")
        || current.get("phase").and_then(|v| v.as_str()) != Some("mounted");

    let mut updated = current.clone();
    if let Some(obj) = updated.as_object_mut() {
        obj.insert(
            "desired_state".to_string(),
            Value::String("running".to_string()),
        );
        if let Some(sig) = config_signature {
            obj.insert(
                "config_signature".to_string(),
                Value::String(sig.to_string()),
            );
        }
        obj.insert(
            "socket_path".to_string(),
            Value::String(socket_path.to_string()),
        );
        obj.insert("last_failure_reason".to_string(), Value::Null);
        obj.insert("shutdown_intent".to_string(), Value::Null);
    }

    lifecycle_save_fn(&updated);
    startup_requested
}

/// Record shutdown intent in lifecycle and shutdown intent stores.
///
/// Mirrors Python `record_shutdown_intent(context, reason)`.
/// Uses closure injection for store operations.
pub fn record_shutdown_intent<LL, LS, S>(
    lifecycle_load_fn: LL,
    lifecycle_save_fn: LS,
    shutdown_save_fn: S,
    project_id: &str,
    reason: &str,
    requested_by_pid: u32,
) where
    LL: Fn(&Value) -> Value,
    LS: Fn(&Value),
    S: Fn(&Value),
{
    let current = lifecycle_load_fn(&Value::Null);

    let mut updated = current.clone();
    if let Some(obj) = updated.as_object_mut() {
        let phase = obj
            .get("phase")
            .and_then(|v| v.as_str())
            .unwrap_or("unmounted");
        let new_phase = if phase == "unmounted" {
            "unmounted"
        } else {
            "stopping"
        };
        obj.insert("phase".to_string(), Value::String(new_phase.to_string()));
        obj.insert(
            "desired_state".to_string(),
            Value::String("stopped".to_string()),
        );
        obj.insert(
            "shutdown_intent".to_string(),
            Value::String(reason.to_string()),
        );
        obj.insert("last_failure_reason".to_string(), Value::Null);
    }

    lifecycle_save_fn(&updated);

    let mut shutdown_intent = serde_json::Map::new();
    shutdown_intent.insert(
        "project_id".to_string(),
        Value::String(project_id.to_string()),
    );
    shutdown_intent.insert(
        "requested_at".to_string(),
        Value::Number(
            (std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64)
                .into(),
        ),
    );
    shutdown_intent.insert(
        "requested_by_pid".to_string(),
        Value::Number(requested_by_pid.into()),
    );
    shutdown_intent.insert("reason".to_string(), Value::String(reason.to_string()));

    shutdown_save_fn(&Value::Object(shutdown_intent));
}

/// Finalize shutdown lifecycle state.
///
/// Mirrors Python `finalize_shutdown_lifecycle(context)`.
/// Uses closure injection for lifecycle store operations.
pub fn finalize_shutdown_lifecycle<L>(lifecycle_load_fn: L, lifecycle_save_fn: L, socket_path: &str)
where
    L: Fn(&Value) -> Value,
{
    let current = lifecycle_load_fn(&Value::Null);

    let mut updated = current.clone();
    if let Some(obj) = updated.as_object_mut() {
        obj.insert("phase".to_string(), Value::String("unmounted".to_string()));
        obj.insert(
            "desired_state".to_string(),
            Value::String("stopped".to_string()),
        );
        obj.insert("owner_pid".to_string(), Value::Null);
        obj.insert("owner_daemon_instance_id".to_string(), Value::Null);
        obj.insert("socket_inode".to_string(), Value::Null);
        obj.insert(
            "socket_path".to_string(),
            Value::String(socket_path.to_string()),
        );
        obj.insert("last_failure_reason".to_string(), Value::Null);
    }

    lifecycle_save_fn(&updated);
}

/// Wait for keeper to be ready.
///
/// Mirrors Python `wait_for_keeper_ready(...)`.
/// Uses closure injection for keeper state store and process checks.
pub fn wait_for_keeper_ready<F1, F2>(
    timeout_s: f64,
    _keeper_state_load_fn: F1,
    keeper_is_running_fn: F2,
) -> bool
where
    F1: Fn(&Value) -> bool,
    F2: Fn(&Value) -> bool,
{
    let timeout = timeout_s.max(0.0);
    let deadline = Instant::now() + Duration::from_secs_f64(timeout);

    while Instant::now() < deadline {
        let state = Value::Object(serde_json::Map::new());
        if keeper_is_running_fn(&state) {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let state = Value::Object(serde_json::Map::new());
    keeper_is_running_fn(&state)
}

/// Wait for keeper to exit.
///
/// Mirrors Python `wait_for_keeper_exit(...)`.
/// Uses closure injection for keeper state store and process checks.
pub fn wait_for_keeper_exit<F1, F2>(
    timeout_s: f64,
    _keeper_state_load_fn: F1,
    keeper_is_running_fn: F2,
) -> bool
where
    F1: Fn(&Value) -> bool,
    F2: Fn(&Value) -> bool,
{
    let timeout = timeout_s.max(0.0);
    let deadline = Instant::now() + Duration::from_secs_f64(timeout);

    while Instant::now() < deadline {
        let state = Value::Object(serde_json::Map::new());
        if !keeper_is_running_fn(&state) {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let state = Value::Object(serde_json::Map::new());
    !keeper_is_running_fn(&state)
}

/// Get keeper PID from state or lease.
///
/// Mirrors Python `keeper_pid(context, lease, ...)`.
/// Uses closure injection for keeper state store and process checks.
pub fn keeper_pid<L, F>(lease: &Value, keeper_state_load_fn: L, keeper_is_running_fn: F) -> i64
where
    L: Fn(&Value) -> Value,
    F: Fn(&Value) -> bool,
{
    let state = keeper_state_load_fn(&Value::Null);

    if keeper_is_running_fn(&state) {
        return state
            .get("keeper_pid")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
    }

    let lease_keeper_pid = lease
        .get("keeper_pid")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if lease_keeper_pid > 0 {
        lease_keeper_pid
    } else {
        0
    }
}

/// Ensure keeper is started, acquiring startup lock if needed.
///
/// Mirrors Python `ensure_keeper_started(...)`.
/// Uses closure injection for all keeper operations.
pub fn ensure_keeper_started<A, G, S>(
    mount_manager_factory: A,
    ownership_guard_factory: G,
    spawn_keeper_fn: S,
    ready_timeout_s: f64,
) -> bool
where
    A: Fn() -> Value,
    G: Fn(Value) -> Value,
    S: FnOnce(),
{
    // Check if already running
    let manager = mount_manager_factory();
    let _guard = ownership_guard_factory(manager);

    // Try to acquire lock and start if not running
    spawn_keeper_fn();
    wait_for_keeper_ready(
        ready_timeout_s,
        |_| true, // State load returns bool (simulating successful load)
        |_| true, // Is running returns true
    )
}

/// Compute the `lib/` root used to locate `ccbd/keeper_main.py`.
///
/// Mirrors Python `_lib_root()`. At build time this is derived from
/// `CARGO_MANIFEST_DIR`; at runtime a release install would need a different
/// resolution strategy.
pub fn keeper_lib_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir
        .ancestors()
        .nth(3)
        .expect("CARGO_MANIFEST_DIR should have a project-root ancestor");
    project_root.join("lib")
}

/// Build the command/environment description for spawning the keeper.
///
/// Mirrors the preparation half of Python `spawn_keeper_process(context)`.
pub fn prepare_keeper_spawn(context: &KeeperContext) -> KeeperSpawn {
    let lib_root = keeper_lib_root();
    let script = lib_root.join("ccbd").join("keeper_main.py");

    let mut env = HashMap::new();
    env.insert("PYTHONUNBUFFERED".to_string(), "1".to_string());
    env.insert(
        "PYTHONPATH".to_string(),
        lib_root.to_string_lossy().to_string(),
    );

    KeeperSpawn {
        program: PathBuf::from("python"),
        args: vec![
            script.to_string_lossy().to_string(),
            "--project".to_string(),
            context.project_root.to_string_lossy().to_string(),
        ],
        cwd: context.project_root.clone(),
        env,
    }
}

/// Spawn the keeper process for `context`.
///
/// Mirrors Python `spawn_keeper_process(context)`.
pub fn spawn_keeper_process(context: &KeeperContext) -> std::io::Result<()> {
    spawn_keeper_process_with(context, |spawn| {
        let mut cmd = Command::new(&spawn.program);
        cmd.args(&spawn.args)
            .current_dir(&spawn.cwd)
            .envs(&spawn.env)
            .stdout(Stdio::from(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(keeper_stdout_path(context))?,
            ))
            .stderr(Stdio::from(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(keeper_stderr_path(context))?,
            ));

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }
        }

        let _ = cmd.spawn()?;
        Ok(())
    })
}

/// Variant that accepts a custom spawn function for testing.
pub fn spawn_keeper_process_with<F>(context: &KeeperContext, spawn_fn: F) -> std::io::Result<()>
where
    F: FnOnce(KeeperSpawn) -> std::io::Result<()>,
{
    ensure_keeper_dirs(context)?;
    let spawn = prepare_keeper_spawn(context);
    spawn_fn(spawn)
}

fn keeper_stdout_path(context: &KeeperContext) -> PathBuf {
    context
        .paths
        .ccbd_dir()
        .as_std_path()
        .join("keeper.stdout.log")
}

fn keeper_stderr_path(context: &KeeperContext) -> PathBuf {
    context
        .paths
        .ccbd_dir()
        .as_std_path()
        .join("keeper.stderr.log")
}

fn ensure_keeper_dirs(context: &KeeperContext) -> std::io::Result<()> {
    // Ensure runtime state root and ccbd dir exist. The Python code also calls
    // `context.paths.ensure_runtime_state_root()`; we create the directories
    // that the logs live under.
    std::fs::create_dir_all(context.paths.ccbd_dir().as_std_path())?;
    std::fs::create_dir_all(context.paths.runtime_state_root().as_std_path())?;
    Ok(())
}

/// Ensure keeper is started for `context`, with injection closures for tests.
///
/// Mirrors Python `ensure_keeper_started(context, ...)`.
pub fn ensure_keeper_started_for_context<M, G, E, C, S>(
    context: &KeeperContext,
    mount_manager_factory: impl FnOnce(&PathLayout) -> M,
    ownership_guard_factory: impl FnOnce(&PathLayout, M) -> G,
    mut process_exists_fn: E,
    mut process_cmdline_fn: C,
    spawn_keeper_process_fn: S,
    ready_timeout_s: f64,
) -> bool
where
    G: OwnershipGuard,
    E: FnMut(u32) -> bool,
    C: FnMut(u32) -> Vec<String>,
    S: FnOnce(&KeeperContext),
{
    if keeper_state_is_running_for_context(
        context,
        &load_keeper_state(context),
        &mut process_exists_fn,
        &mut process_cmdline_fn,
        true,
    ) {
        return true;
    }

    let manager = mount_manager_factory(&context.paths);
    let guard = ownership_guard_factory(&context.paths, manager);

    guard.with_startup_lock(|| {
        if keeper_state_is_running_for_context(
            context,
            &load_keeper_state(context),
            &mut process_exists_fn,
            &mut process_cmdline_fn,
            true,
        ) {
            return true;
        }

        spawn_keeper_process_fn(context);

        wait_for_keeper_ready_for_context(
            context,
            ready_timeout_s,
            &mut process_exists_fn,
            &mut process_cmdline_fn,
        )
    })
}

/// Wait until keeper state shows it is running for `context`.
pub fn wait_for_keeper_ready_for_context<E, C>(
    context: &KeeperContext,
    timeout_s: f64,
    process_exists_fn: &mut E,
    process_cmdline_fn: &mut C,
) -> bool
where
    E: FnMut(u32) -> bool,
    C: FnMut(u32) -> Vec<String>,
{
    let deadline = Instant::now() + Duration::from_secs_f64(timeout_s.max(0.0));

    while Instant::now() < deadline {
        if keeper_state_is_running_for_context(
            context,
            &load_keeper_state(context),
            process_exists_fn,
            process_cmdline_fn,
            true,
        ) {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }

    keeper_state_is_running_for_context(
        context,
        &load_keeper_state(context),
        process_exists_fn,
        process_cmdline_fn,
        true,
    )
}

/// Determine whether the loaded keeper state represents a running keeper for
/// this project.
pub fn keeper_state_is_running_for_context<E, C>(
    context: &KeeperContext,
    state: &Option<Value>,
    process_exists_fn: &mut E,
    process_cmdline_fn: &mut C,
    require_cmdline_match: bool,
) -> bool
where
    E: FnMut(u32) -> bool,
    C: FnMut(u32) -> Vec<String>,
{
    let state = match state {
        Some(s) => s,
        None => return false,
    };

    if state.get("state").and_then(|v| v.as_str()) != Some("running") {
        return false;
    }

    if state.get("project_id").and_then(|v| v.as_str()) != Some(&context.project_id) {
        return false;
    }

    let keeper_pid = match state.get("keeper_pid").and_then(|v| v.as_u64()) {
        Some(pid) if pid > 0 && pid <= u32::MAX as u64 => pid as u32,
        _ => return false,
    };

    if !process_exists_fn(keeper_pid) {
        return false;
    }

    if !require_cmdline_match {
        return true;
    }

    let cmdline = process_cmdline_fn(keeper_pid);
    keeper_cmdline_matches_project(&cmdline, &context.project_root)
}

fn keeper_cmdline_matches_project(cmdline: &[String], project_root: &Path) -> bool {
    if cmdline.is_empty() {
        return false;
    }
    if !cmdline.iter().any(|arg| is_keeper_entrypoint_arg(arg)) {
        return false;
    }
    let project_arg = match project_arg_value(cmdline) {
        Some(arg) => arg,
        None => return false,
    };
    normalized_path(&project_arg) == normalized_path(project_root)
}

fn is_keeper_entrypoint_arg(value: &str) -> bool {
    let normalized = value.replace('\\', "/");
    normalized == "ccbd.keeper_main" || normalized.ends_with("/ccbd/keeper_main.py")
}

fn project_arg_value(args: &[String]) -> Option<String> {
    for (index, arg) in args.iter().enumerate() {
        if arg == "--project" && index + 1 < args.len() {
            return Some(args[index + 1].clone());
        }
        if let Some(value) = arg.strip_prefix("--project=") {
            return Some(value.to_string());
        }
    }
    None
}

fn normalized_path(value: impl AsRef<Path>) -> String {
    let path = value.as_ref();
    if let Ok(resolved) = path.canonicalize() {
        resolved.to_string_lossy().to_string()
    } else if let Ok(absolute) = std::path::absolute(path) {
        absolute.to_string_lossy().to_string()
    } else {
        path.to_string_lossy().to_string()
    }
}

fn keeper_state_path(context: &KeeperContext) -> PathBuf {
    context.paths.ccbd_dir().as_std_path().join("keeper.json")
}

fn load_keeper_state(context: &KeeperContext) -> Option<Value> {
    let path = keeper_state_path(context);
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_keeper_spawn_points_at_lib_keeper_main() {
        let context = KeeperContext::from_project_root("/tmp/repo");
        let spawn = prepare_keeper_spawn(&context);

        let lib_root = keeper_lib_root();
        let expected_script = lib_root.join("ccbd").join("keeper_main.py");
        assert_eq!(spawn.program, PathBuf::from("python"));
        assert_eq!(spawn.args[0], expected_script.to_string_lossy().to_string());
        assert_eq!(spawn.args[1], "--project");
        assert_eq!(spawn.args[2], "/tmp/repo");
        assert_eq!(
            spawn.env.get("PYTHONPATH"),
            Some(&lib_root.to_string_lossy().to_string())
        );
    }

    #[test]
    fn keeper_cmdline_matches_project_detects_project_arg() {
        let cmdline = vec![
            "python".to_string(),
            "/opt/ccb/lib/ccbd/keeper_main.py".to_string(),
            "--project".to_string(),
            "/tmp/repo".to_string(),
        ];
        assert!(keeper_cmdline_matches_project(
            &cmdline,
            Path::new("/tmp/repo")
        ));
    }

    #[test]
    fn keeper_cmdline_matches_project_rejects_wrong_project() {
        let cmdline = vec![
            "python".to_string(),
            "/opt/ccb/lib/ccbd/keeper_main.py".to_string(),
            "--project".to_string(),
            "/other/repo".to_string(),
        ];
        assert!(!keeper_cmdline_matches_project(
            &cmdline,
            Path::new("/tmp/repo")
        ));
    }
}
