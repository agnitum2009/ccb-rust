//! Concrete `ensure_agent_runtime` orchestration.
//!
//! Mirrors Python `lib/cli/services/runtime_launch_runtime/ensure.py` +
//! `tmux_runtime.py` by composing `ProviderLauncher::build_plan` with tmux
//! pane lifecycle and CCB pane identity.

use std::collections::HashMap;
use std::path::Path;

use ccbr_agents::models::{
    AgentApiSpec, AgentSpec as CcbAgentSpec, PermissionMode, ProviderProfileSpec, QueuePolicy,
    RestoreMode, RuntimeMode, WorkspaceMode,
};
use ccbr_terminal::identity::pane_visual;
use ccbr_terminal::layouts::TmuxLayoutBackend;
use serde_json::Value;

use crate::provider_launcher::{LaunchContext, ProviderLauncher};
use crate::start_runtime::agent_runtime_models::{
    AgentSpec, Command, Context, EnsureAgentRuntimeFn, EnsureAgentRuntimeResult, Plan,
    RuntimeBinding,
};

/// Clipboard sink used by tmux copy-mode key bindings (y, Enter, mouse drag).
///
/// Writes the copied text to a temp file, then tries Wayland (`wl-copy`),
/// X11 (`xclip`, `xsel`), or macOS (`pbcopy`) in the background and cleans up.
const CLIPBOARD_PIPE_COMMAND: &str = "sh -lc 'tmp=$(mktemp \"${TMPDIR:-/tmp}/ccbr-clipboard.XXXXXX\") || exit 0; cat >\"$tmp\"; if command -v wl-copy >/dev/null 2>&1 && [ -n \"${WAYLAND_DISPLAY:-}\" ]; then (wl-copy <\"$tmp\"; rm -f \"$tmp\") >/dev/null 2>&1 & elif command -v xclip >/dev/null 2>&1 && [ -n \"${DISPLAY:-}\" ]; then (xclip -selection clipboard <\"$tmp\"; rm -f \"$tmp\") >/dev/null 2>&1 & elif command -v xsel >/dev/null 2>&1 && [ -n \"${DISPLAY:-}\" ]; then (xsel --clipboard --input <\"$tmp\"; rm -f \"$tmp\") >/dev/null 2>&1 & elif command -v pbcopy >/dev/null 2>&1; then pbcopy <\"$tmp\"; rm -f \"$tmp\"; else rm -f \"$tmp\"; fi'";

type TmuxBackendFactory =
    Box<dyn Fn(Option<String>, Option<String>) -> Box<dyn TmuxLayoutBackend> + Send + Sync>;

/// Dependency-injected `ensure_agent_runtime` implementation.
pub struct EnsureAgentRuntimeImpl {
    launcher: ProviderLauncher,
    backend_factory: TmuxBackendFactory,
    allow_detached_fallback: bool,
    min_pane_width: u32,
    min_pane_height: u32,
}

impl EnsureAgentRuntimeImpl {
    /// Create an instance with a custom tmux backend factory.
    pub fn new<F>(launcher: ProviderLauncher, backend_factory: F) -> Self
    where
        F: Fn(Option<String>, Option<String>) -> Box<dyn TmuxLayoutBackend> + Send + Sync + 'static,
    {
        Self {
            launcher,
            backend_factory: Box::new(backend_factory),
            allow_detached_fallback: true,
            min_pane_width: 20,
            min_pane_height: 8,
        }
    }

    /// Create an instance that uses the real `ccbr-terminal::TmuxBackend`.
    pub fn with_default_backend(launcher: ProviderLauncher) -> Self {
        Self::new(launcher, |socket_name, socket_path| {
            Box::new(ccbr_terminal::TmuxBackend::new(socket_name, socket_path))
        })
    }

    /// Control whether a detached tmux session may be used when the project
    /// namespace cannot allocate a stable pane.
    pub fn with_allow_detached_fallback(mut self, allow: bool) -> Self {
        self.allow_detached_fallback = allow;
        self
    }

    /// Set the minimum pane dimensions required for a fresh pane.
    pub fn with_min_pane_size(mut self, width: u32, height: u32) -> Self {
        self.min_pane_width = width;
        self.min_pane_height = height;
        self
    }

    fn backend(&self, tmux_socket_path: Option<&str>) -> Box<dyn TmuxLayoutBackend> {
        let path = tmux_socket_path.map(String::from);
        (self.backend_factory)(path.clone(), path)
    }
}

impl EnsureAgentRuntimeFn for EnsureAgentRuntimeImpl {
    fn call(
        &self,
        context: &Context,
        command: &Command,
        spec: &AgentSpec,
        plan: &Plan,
        binding_hint: Option<&RuntimeBinding>,
        assigned_pane_id: Option<&str>,
        style_index: usize,
        tmux_socket_path: Option<&str>,
    ) -> Result<EnsureAgentRuntimeResult, String> {
        // 1. Reuse an already-alive binding when no explicit pane is assigned.
        //    Foreign bindings (different project/socket or explicitly marked) are
        //    never reused; they are treated as stale and replaced.
        if assigned_pane_id.is_none() {
            if let Some(binding) = binding_hint {
                if !binding_is_foreign(binding, &context.project_id, tmux_socket_path)
                    && binding.runtime_ref.is_some()
                    && binding.session_ref.is_some()
                    && binding_alive(binding, &*self.backend(tmux_socket_path))
                {
                    return Ok(EnsureAgentRuntimeResult {
                        launched: false,
                        binding: Some(binding.clone()),
                    });
                }
            }
        }

        // 2. Best-effort cleanup of any stale pane from the hint.
        if let Some(binding) = binding_hint {
            if let Some(pane_id) = pane_id_from_runtime_ref(binding) {
                let _ = kill_pane(&*self.backend(tmux_socket_path), &pane_id);
            }
        }

        // 3. Build provider launch plan (writes the session file).
        let full_spec = to_full_agent_spec(spec, command, plan);
        let placeholder_pane = "%0";
        let launch_ctx = LaunchContext {
            provider: &full_spec.provider,
            agent_name: &full_spec.name,
            project_id: &context.project_id,
            project_root: &context.project_root,
            workspace_path: &plan.workspace_path,
            pane_id: assigned_pane_id.unwrap_or(placeholder_pane),
            socket_path: tmux_socket_path.unwrap_or(""),
            restore: command.restore,
            command_template: None,
            startup_args: &full_spec.startup_args,
            auto_permission: false,
            spec: Some(&full_spec),
        };
        let launch_result = self
            .launcher
            .build_plan(&launch_ctx)
            .map_err(|e| format!("build launch plan failed: {e}"))?;
        let session_path = launch_result
            .session_path
            .ok_or("launch plan missing session path")?;

        let launch_session_id = format!("{}-{}-launch", context.project_id, spec.name);

        // 4. Launch / respawn the tmux pane.
        let backend = self.backend(tmux_socket_path);
        let pane_id = if let Some(pane_id) = assigned_pane_id {
            backend
                .tmux_run(
                    &[
                        "respawn-pane",
                        "-k",
                        "-t",
                        pane_id,
                        "-c",
                        &plan.workspace_path,
                        &launch_result.command,
                    ],
                    true,
                    false,
                )
                .map_err(|e| format!("failed to respawn pane {pane_id}: {e}"))?;
            pane_id.to_string()
        } else {
            match backend.create_pane(
                &launch_result.command,
                &plan.workspace_path,
                "right",
                50,
                None,
            ) {
                Ok(pane_id) => {
                    if pane_meets_minimum_size(
                        &*backend,
                        &pane_id,
                        self.min_pane_width,
                        self.min_pane_height,
                    ) {
                        pane_id
                    } else {
                        best_effort_kill_pane(&*backend, &pane_id);
                        if self.allow_detached_fallback {
                            create_detached_tmux_pane(
                                &*backend,
                                &launch_result.command,
                                &plan.workspace_path,
                                &launch_session_id,
                            )?
                        } else {
                            return Err(format!(
                                "project namespace launch could not allocate stable tmux pane for {}",
                                spec.name
                            ));
                        }
                    }
                }
                Err(e) => {
                    let msg = e.to_string().to_lowercase();
                    let no_space = msg.contains("split-window failed")
                        || msg.contains("no space for new pane");
                    if self.allow_detached_fallback && no_space {
                        create_detached_tmux_pane(
                            &*backend,
                            &launch_result.command,
                            &plan.workspace_path,
                            &launch_session_id,
                        )?
                    } else if no_space {
                        return Err(format!(
                            "project namespace launch could not allocate stable tmux pane for {}",
                            spec.name
                        ));
                    } else {
                        return Err(format!("failed to create pane: {e}"));
                    }
                }
            }
        };

        // 5. Rewrite the session payload with the real pane id.
        if let Some(payload) = launch_result.session_payload {
            rewrite_session_payload(&session_path, &payload, &pane_id)
                .map_err(|e| format!("failed to update session file with pane id: {e}"))?;
        }

        // 6. Apply CCB pane identity metadata.
        apply_identity(
            &*backend,
            &pane_id,
            &spec.name,
            &spec.name,
            &context.project_id,
            style_index,
            &launch_session_id,
        );

        // 7. Build the refreshed binding.
        let session_ref = session_path.to_string_lossy().to_string();
        let binding = RuntimeBinding {
            runtime_ref: Some(format!("tmux:{pane_id}")),
            session_ref: Some(session_ref.clone()),
            terminal: Some("tmux".to_string()),
            pane_id: Some(pane_id.clone()),
            active_pane_id: Some(pane_id),
            session_file: Some(session_ref),
            tmux_socket_path: tmux_socket_path.map(String::from),
            ccbr_session_id: Some(launch_session_id),
            ..RuntimeBinding::default()
        };

        Ok(EnsureAgentRuntimeResult {
            launched: true,
            binding: Some(binding),
        })
    }
}

fn binding_alive(binding: &RuntimeBinding, backend: &dyn TmuxLayoutBackend) -> bool {
    pane_id_from_runtime_ref(binding)
        .map(|pane_id| backend.is_alive(&pane_id))
        .unwrap_or(false)
}

fn binding_is_foreign(
    binding: &RuntimeBinding,
    project_id: &str,
    tmux_socket_path: Option<&str>,
) -> bool {
    if binding.pane_state.as_deref() == Some("foreign") {
        return true;
    }
    if let Some(binding_project) = binding.ccbr_project_id.as_deref() {
        if binding_project != project_id {
            return true;
        }
    }
    if let (Some(a), Some(b)) = (binding.tmux_socket_path.as_deref(), tmux_socket_path) {
        if a != b {
            return true;
        }
    }
    false
}

fn pane_id_from_runtime_ref(binding: &RuntimeBinding) -> Option<String> {
    binding
        .runtime_ref
        .as_deref()
        .filter(|r| r.starts_with("tmux:"))
        .map(|r| r["tmux:".len()..].to_string())
}

fn kill_pane(backend: &dyn TmuxLayoutBackend, pane_id: &str) -> Result<(), String> {
    backend
        .tmux_run(&["kill-pane", "-t", pane_id], false, false)
        .map_err(|e| format!("failed to kill stale pane {pane_id}: {e}"))?;
    Ok(())
}

fn best_effort_kill_pane(backend: &dyn TmuxLayoutBackend, pane_id: &str) {
    let _ = backend.tmux_run(&["kill-pane", "-t", pane_id], false, false);
}

fn pane_meets_minimum_size(
    backend: &dyn TmuxLayoutBackend,
    pane_id: &str,
    min_width: u32,
    min_height: u32,
) -> bool {
    let output = match backend.tmux_run(
        &[
            "list-panes",
            "-t",
            pane_id,
            "-F",
            "#{pane_width} #{pane_height}",
        ],
        true,
        true,
    ) {
        Ok(o) => o,
        Err(_) => return false,
    };
    let mut parts = output.split_whitespace();
    let width = parts
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let height = parts
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    width >= min_width && height >= min_height
}

fn create_detached_tmux_pane(
    backend: &dyn TmuxLayoutBackend,
    cmd: &str,
    cwd: &str,
    session_name: &str,
) -> Result<String, String> {
    let target_session = format!(
        "{}-{}-{}",
        session_name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        std::process::id()
    );
    prepare_detached_tmux_server(backend)?;
    backend
        .tmux_run(
            &[
                "new-session",
                "-d",
                "-x",
                "160",
                "-y",
                "48",
                "-s",
                &target_session,
                "-c",
                cwd,
                "-F",
                "#{pane_id}",
                "bash",
                "-c",
                "sleep 365d",
            ],
            true,
            false,
        )
        .map_err(|e| format!("new-session failed: {e}"))?;
    let pane_id = backend
        .tmux_run(
            &["list-panes", "-t", &target_session, "-F", "#{pane_id}"],
            true,
            true,
        )
        .map_err(|e| format!("list-panes failed: {e}"))?;
    let pane_id = pane_id.lines().next().unwrap_or("").trim().to_string();
    if pane_id.is_empty() {
        return Err(format!(
            "failed to create detached tmux pane for session {target_session}"
        ));
    }
    backend
        .tmux_run(
            &["respawn-pane", "-k", "-t", &pane_id, "-c", cwd, cmd],
            true,
            false,
        )
        .map_err(|e| format!("respawn-pane failed: {e}"))?;
    Ok(pane_id)
}

fn prepare_detached_tmux_server(backend: &dyn TmuxLayoutBackend) -> Result<(), String> {
    let commands: Vec<Vec<&str>> = vec![
        vec!["start-server"],
        vec!["set-option", "-g", "destroy-unattached", "off"],
        vec!["set-option", "-g", "mouse", "on"],
        vec!["set-option", "-g", "history-limit", "50000"],
        vec!["set-option", "-g", "set-clipboard", "on"],
        vec!["set-option", "-g", "focus-events", "on"],
        vec!["set-option", "-g", "escape-time", "10"],
        vec!["set-window-option", "-g", "mode-keys", "vi"],
        vec![
            "bind-key",
            "-T",
            "copy-mode-vi",
            "v",
            "send-keys",
            "-X",
            "begin-selection",
        ],
        vec![
            "bind-key",
            "-T",
            "copy-mode-vi",
            "C-v",
            "send-keys",
            "-X",
            "rectangle-toggle",
        ],
        vec![
            "bind-key",
            "-T",
            "copy-mode-vi",
            "y",
            "send-keys",
            "-X",
            "copy-pipe-and-cancel",
            CLIPBOARD_PIPE_COMMAND,
        ],
        vec![
            "bind-key",
            "-T",
            "copy-mode-vi",
            "Enter",
            "send-keys",
            "-X",
            "copy-pipe-and-cancel",
            CLIPBOARD_PIPE_COMMAND,
        ],
        vec![
            "bind-key",
            "-T",
            "copy-mode-vi",
            "MouseDragEnd1Pane",
            "send-keys",
            "-X",
            "copy-pipe-and-cancel",
            CLIPBOARD_PIPE_COMMAND,
        ],
    ];
    for args in commands {
        let _ = backend.tmux_run(&args, false, false);
    }
    Ok(())
}

fn apply_identity(
    backend: &dyn TmuxLayoutBackend,
    pane_id: &str,
    title: &str,
    agent_label: &str,
    project_id: &str,
    style_index: usize,
    session_id: &str,
) {
    backend.set_pane_title(pane_id, title);
    let visual = pane_visual(
        project_id,
        agent_label,
        Some(style_index as i32),
        false,
        "agent",
    );
    let options = [
        ("@ccbr_label_style", visual.label_style.as_str()),
        ("@ccbr_border_style", visual.border_style.as_str()),
        (
            "@ccbr_active_border_style",
            visual.active_border_style.as_str(),
        ),
        ("@ccbr_agent", agent_label),
        ("@ccbr_role", "agent"),
        ("@ccbr_slot", agent_label),
        ("@ccbr_project_id", project_id),
        ("@ccbr_session_id", session_id),
        ("@ccbr_managed_by", "ccbd"),
    ];
    for (name, value) in options {
        backend.set_pane_user_option(pane_id, name, value);
    }
    backend.set_pane_style(
        pane_id,
        Some(&visual.border_style),
        Some(&visual.active_border_style),
    );
}

fn rewrite_session_payload(
    session_path: &Path,
    payload: &Value,
    pane_id: &str,
) -> Result<(), String> {
    let mut updated = payload.clone();
    if let Some(obj) = updated.as_object_mut() {
        obj.insert(
            "pane_id".to_string(),
            serde_json::Value::String(pane_id.to_string()),
        );
        obj.insert(
            "tmux_session".to_string(),
            serde_json::Value::String(pane_id.to_string()),
        );
    }
    std::fs::write(
        session_path,
        serde_json::to_string(&updated).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("failed to rewrite session file: {e}"))?;
    Ok(())
}

fn to_full_agent_spec(spec: &AgentSpec, command: &Command, plan: &Plan) -> CcbAgentSpec {
    CcbAgentSpec {
        name: spec.name.clone(),
        provider: spec.provider.clone(),
        target: spec.name.clone(),
        workspace_mode: WorkspaceMode::Inplace,
        workspace_root: None,
        runtime_mode: RuntimeMode::PaneBacked,
        restore_default: if command.restore {
            RestoreMode::Provider
        } else {
            RestoreMode::Fresh
        },
        permission_default: PermissionMode::Manual,
        queue_policy: QueuePolicy::SerialPerAgent,
        workspace_path: Some(plan.workspace_path.clone()),
        workspace_group: None,
        provider_command_template: None,
        model: None,
        startup_args: Vec::new(),
        env: HashMap::new(),
        api: AgentApiSpec::default(),
        provider_profile: ProviderProfileSpec::default(),
        branch_template: None,
        labels: Vec::new(),
        description: None,
        role: None,
        watch_paths: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::start_runtime::test_support::{make_ensure_impl, FakeBackend};
    use std::sync::Arc;

    fn test_ctx() -> Context {
        Context {
            project_id: "proj".to_string(),
            project_root: "/tmp/proj".to_string(),
            workspace_path: "/tmp/proj".to_string(),
        }
    }

    fn test_spec(provider: &str) -> AgentSpec {
        AgentSpec {
            name: "agent1".to_string(),
            runtime_mode: "pane".to_string(),
            provider: provider.to_string(),
        }
    }

    fn test_command() -> Command {
        Command { restore: false }
    }

    #[test]
    fn test_reuse_alive_binding() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_string_lossy().to_string();
        let mut ctx = test_ctx();
        ctx.project_root = root.clone();
        ctx.workspace_path = root.clone();
        let plan = Plan {
            workspace_path: root,
        };

        let backend = Arc::new(FakeBackend::new("%99"));
        backend.mark_alive("%42", true);
        let impl_ = make_ensure_impl(backend.clone());

        let binding = RuntimeBinding {
            runtime_ref: Some("tmux:%42".to_string()),
            session_ref: Some("/tmp/proj/.ccbr/.codex-agent1-session".to_string()),
            ..RuntimeBinding::default()
        };

        let result = EnsureAgentRuntimeFn::call(
            &impl_,
            &ctx,
            &test_command(),
            &test_spec("codex"),
            &plan,
            Some(&binding),
            None,
            0,
            Some("/tmp/tmux.sock"),
        )
        .unwrap();

        assert!(!result.launched);
        assert_eq!(
            result.binding.unwrap().runtime_ref.as_deref(),
            Some("tmux:%42")
        );
        assert!(backend.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_launch_creates_pane_and_session_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_string_lossy().to_string();
        let mut ctx = test_ctx();
        ctx.project_root = root.clone();
        ctx.workspace_path = root.clone();
        let plan = Plan {
            workspace_path: root,
        };

        let backend = Arc::new(FakeBackend::new("%99"));
        let impl_ = make_ensure_impl(backend.clone());

        let result = EnsureAgentRuntimeFn::call(
            &impl_,
            &ctx,
            &test_command(),
            &test_spec("codex"),
            &plan,
            None,
            None,
            0,
            Some("/tmp/tmux.sock"),
        )
        .unwrap();

        assert!(result.launched);
        let binding = result.binding.unwrap();
        assert_eq!(binding.runtime_ref.as_deref(), Some("tmux:%99"));
        assert!(binding
            .session_ref
            .as_deref()
            .unwrap()
            .contains(".codex-agent1-session"));
        assert!(std::path::Path::new(binding.session_ref.as_deref().unwrap()).exists());
        assert!(backend.has_call("create_pane:"));
        assert!(backend.has_call("set_pane_title:"));
    }

    #[test]
    fn test_stale_binding_kills_old_pane() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_string_lossy().to_string();
        let mut ctx = test_ctx();
        ctx.project_root = root.clone();
        ctx.workspace_path = root.clone();
        let plan = Plan {
            workspace_path: root,
        };

        let backend = Arc::new(FakeBackend::new("%99"));
        backend.mark_alive("%42", false);
        let impl_ = make_ensure_impl(backend.clone());

        let binding = RuntimeBinding {
            runtime_ref: Some("tmux:%42".to_string()),
            session_ref: Some("/tmp/proj/.ccbr/.codex-agent1-session".to_string()),
            ..RuntimeBinding::default()
        };

        let result = EnsureAgentRuntimeFn::call(
            &impl_,
            &ctx,
            &test_command(),
            &test_spec("codex"),
            &plan,
            Some(&binding),
            None,
            0,
            Some("/tmp/tmux.sock"),
        )
        .unwrap();

        assert!(result.launched);
        assert_eq!(
            result.binding.unwrap().runtime_ref.as_deref(),
            Some("tmux:%99")
        );
        assert!(backend.has_call("tmux_run:kill-pane"));
    }

    #[test]
    fn test_respawn_assigned_pane() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_string_lossy().to_string();
        let mut ctx = test_ctx();
        ctx.project_root = root.clone();
        ctx.workspace_path = root.clone();
        let plan = Plan {
            workspace_path: root,
        };

        let backend = Arc::new(FakeBackend::new("%5"));
        let impl_ = make_ensure_impl(backend.clone());

        let result = EnsureAgentRuntimeFn::call(
            &impl_,
            &ctx,
            &test_command(),
            &test_spec("claude"),
            &plan,
            None,
            Some("%5"),
            1,
            Some("/tmp/tmux.sock"),
        )
        .unwrap();

        assert!(result.launched);
        assert_eq!(
            result.binding.unwrap().runtime_ref.as_deref(),
            Some("tmux:%5")
        );
        assert!(backend.has_call("tmux_run:respawn-pane"));
    }
}
