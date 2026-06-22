//! Mirrors Python `lib/provider_backends/claude/launcher_runtime/restore.py`.

use camino::{Utf8Path, Utf8PathBuf};

/// Restore target for a Claude launch: the working directory and whether the
/// session has history that can be continued.
#[derive(Debug, Clone)]
pub struct ClaudeRestoreTarget {
    pub run_cwd: Utf8PathBuf,
    pub has_history: bool,
}

/// Minimal parity resolver: returns the requested workspace path with no
/// history continuation. Full history scanning is deferred.
pub fn resolve_claude_restore_target(
    _spec: &ccb_agents::models::AgentSpec,
    runtime_dir: &Utf8Path,
    restore: bool,
    workspace_path: Option<&Utf8Path>,
) -> ClaudeRestoreTarget {
    let workspace_path = workspace_path
        .map(|p| p.to_path_buf())
        .or_else(|| infer_workspace_path(runtime_dir))
        .unwrap_or_else(|| Utf8PathBuf::from("."));
    if !restore {
        return ClaudeRestoreTarget {
            run_cwd: workspace_path,
            has_history: false,
        };
    }
    ClaudeRestoreTarget {
        run_cwd: workspace_path,
        has_history: false,
    }
}

fn infer_workspace_path(runtime_dir: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut current = Some(runtime_dir);
    while let Some(p) = current {
        if p.file_name() == Some(".ccb") {
            return Some(p.parent().unwrap_or(p).to_path_buf());
        }
        current = p.parent();
    }
    None
}
