//! Mirrors Python `lib/provider_backends/claude/launcher_runtime/restore.py`.

use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};
use ccbr_agents::models::RestoreMode;

use crate::claude::launcher_runtime::history::ClaudeHistoryLocator;
use crate::claude::launcher_runtime::home::resolve_claude_home_layout;

/// Restore target for a Claude launch: the working directory and whether the
/// session has history that can be continued.
#[derive(Debug, Clone)]
pub struct ClaudeRestoreTarget {
    pub run_cwd: Utf8PathBuf,
    pub has_history: bool,
}

/// Resolve whether Claude should continue an existing session.
///
/// This scans the isolated Claude home for project/session history. When
/// `restore` is false or the agent's `restore_default` is `Fresh`, history is
/// ignored and `--continue` is not requested.
pub fn resolve_claude_restore_target(
    spec: &ccbr_agents::models::AgentSpec,
    runtime_dir: &Utf8Path,
    restore: bool,
    workspace_path: Option<&Utf8Path>,
) -> ClaudeRestoreTarget {
    let (session_home, session_work_dir) = managed_session_home_and_work_dir(runtime_dir);

    let workspace_path = session_work_dir
        .or_else(|| workspace_path.map(|p| p.to_path_buf()))
        .or_else(|| infer_workspace_path(runtime_dir))
        .unwrap_or_else(|| Utf8PathBuf::from("."));

    let default_target = ClaudeRestoreTarget {
        run_cwd: workspace_path.clone(),
        has_history: false,
    };

    if !restore || spec.restore_default == RestoreMode::Fresh {
        return default_target;
    }

    let home_dir =
        session_home.unwrap_or_else(|| resolve_claude_home_layout(runtime_dir, None).home_root);
    let (_session_id, has_history, best_cwd) =
        claude_history_state(&workspace_path, &workspace_path, &spec.env, &home_dir);

    let run_cwd = best_cwd
        .and_then(|p| existing_dir(&p))
        .unwrap_or(workspace_path);

    ClaudeRestoreTarget {
        run_cwd,
        has_history,
    }
}

/// Load the managed home and working directory from the project session file.
///
/// Mirrors Python behavior: the session file's `claude_home` is only trusted
/// when it lives inside the project's managed Claude state directory.
fn managed_session_home_and_work_dir(
    runtime_dir: &Utf8Path,
) -> (Option<Utf8PathBuf>, Option<Utf8PathBuf>) {
    let session_path =
        match crate::session_paths::session_file_for_runtime_dir("claude", runtime_dir) {
            Some(p) => p,
            None => return (None, None),
        };
    let payload = match crate::session_paths::read_session_payload(&session_path) {
        Some(p) => p,
        None => return (None, None),
    };
    let work_dir = payload
        .get("work_dir")
        .and_then(|v| v.as_str())
        .map(Utf8PathBuf::from)
        .filter(|p| !p.as_str().is_empty());
    let home = payload
        .get("claude_home")
        .and_then(|v| v.as_str())
        .map(Utf8PathBuf::from)
        .filter(|p| !p.as_str().is_empty());

    let managed_home = resolve_claude_home_layout(runtime_dir, None).home_root;
    let home = home.filter(|h| h.as_str().starts_with(managed_home.as_str()));
    (home, work_dir)
}

/// Scan Claude history for a workspace.
///
/// Returns `(session_id, has_history, best_cwd)`. `session_id` is only present
/// when a valid UUID session file was found.
pub fn claude_history_state(
    invocation_dir: &Utf8Path,
    project_root: &Utf8Path,
    env: &HashMap<String, String>,
    home_dir: &Utf8Path,
) -> (Option<String>, bool, Option<Utf8PathBuf>) {
    let locator = ClaudeHistoryLocator::new(invocation_dir, project_root, env, home_dir);
    locator.latest_session_id()
}

fn infer_workspace_path(runtime_dir: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut current = Some(runtime_dir);
    while let Some(p) = current {
        if p.file_name() == Some(".ccbr") {
            return Some(p.parent().unwrap_or(p).to_path_buf());
        }
        current = p.parent();
    }
    None
}

fn existing_dir(value: &Utf8Path) -> Option<Utf8PathBuf> {
    if value.as_str().trim().is_empty() {
        return None;
    }
    let expanded = if let Some(rest) = value.as_str().strip_prefix("~/") {
        std::env::var("HOME")
            .ok()
            .map(|home| Utf8PathBuf::from(home).join(rest))
            .unwrap_or_else(|| value.to_path_buf())
    } else {
        value.to_path_buf()
    };
    if expanded.exists() && expanded.is_dir() {
        Some(expanded)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccbr_agents::models::{
        AgentSpec, PermissionMode, QueuePolicy, RestoreMode, RuntimeMode, WorkspaceMode,
    };
    use serde_json::json;

    fn spec(name: &str) -> AgentSpec {
        AgentSpec {
            name: name.into(),
            provider: "claude".into(),
            target: ".".into(),
            workspace_mode: WorkspaceMode::GitWorktree,
            workspace_root: None,
            runtime_mode: RuntimeMode::PaneBacked,
            restore_default: RestoreMode::Auto,
            permission_default: PermissionMode::Manual,
            queue_policy: QueuePolicy::SerialPerAgent,
            ..AgentSpec::default_with_name(name)
        }
    }

    fn create_session_file(project_root: &std::path::Path, name: &str, payload: serde_json::Value) {
        let session_path = project_root
            .join(".ccbr")
            .join(format!(".claude-{name}-session"));
        std::fs::create_dir_all(session_path.parent().unwrap()).unwrap();
        std::fs::write(&session_path, payload.to_string()).unwrap();
    }

    fn write_claude_history(home_dir: &Utf8Path, workspace_path: &Utf8Path, session_id: &str) {
        let project_key: String = workspace_path
            .as_str()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect();
        let project_dir = home_dir.join(".claude").join("projects").join(&project_key);
        let session_env_root = home_dir.join(".claude").join("session-env");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::create_dir_all(&session_env_root).unwrap();
        std::fs::write(project_dir.join(format!("{session_id}.jsonl")), "history\n").unwrap();
        std::fs::create_dir_all(session_env_root.join(session_id)).unwrap();
    }

    #[test]
    fn restore_prefers_project_session_work_dir_and_managed_home() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root_buf = tmp.path().join("repo");
        let project_root = Utf8Path::from_path(&project_root_buf).unwrap();
        let runtime_dir = project_root
            .join(".ccbr")
            .join("agents")
            .join("reviewer")
            .join("provider-runtime")
            .join("claude");
        let workspace_path = project_root
            .join(".ccbr")
            .join("workspaces")
            .join("reviewer");
        let managed_home = project_root
            .join(".ccbr")
            .join("agents")
            .join("reviewer")
            .join("provider-state")
            .join("claude")
            .join("home");
        std::fs::create_dir_all(&runtime_dir).unwrap();
        std::fs::create_dir_all(&workspace_path).unwrap();

        create_session_file(
            project_root.as_std_path(),
            "reviewer",
            json!({
                "work_dir": workspace_path.as_str(),
                "claude_session_id": "claude-sess-1",
                "claude_home": managed_home.as_str(),
            }),
        );

        let session_id = "550e8400-e29b-41d4-a716-446655440000";
        write_claude_history(&managed_home, &workspace_path, session_id);

        let target = resolve_claude_restore_target(&spec("reviewer"), &runtime_dir, true, None);

        assert!(target.has_history);
        assert_eq!(target.run_cwd, workspace_path);
    }

    #[test]
    fn restore_ignores_non_managed_project_session_home() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root_buf = tmp.path().join("repo");
        let project_root = Utf8Path::from_path(&project_root_buf).unwrap();
        let runtime_dir = project_root
            .join(".ccbr")
            .join("agents")
            .join("reviewer")
            .join("provider-runtime")
            .join("claude");
        let workspace_path = project_root
            .join(".ccbr")
            .join("workspaces")
            .join("reviewer");
        let legacy_home_buf = tmp.path().join("legacy-home");
        let legacy_home = Utf8Path::from_path(&legacy_home_buf).unwrap();
        std::fs::create_dir_all(&runtime_dir).unwrap();
        std::fs::create_dir_all(&workspace_path).unwrap();

        create_session_file(
            project_root.as_std_path(),
            "reviewer",
            json!({
                "work_dir": workspace_path.as_str(),
                "claude_session_id": "claude-sess-1",
                "claude_home": legacy_home.as_str(),
            }),
        );

        let session_id = "550e8400-e29b-41d4-a716-446655440000";
        write_claude_history(legacy_home, &workspace_path, session_id);

        let target = resolve_claude_restore_target(&spec("reviewer"), &runtime_dir, true, None);

        assert!(!target.has_history);
    }
}
