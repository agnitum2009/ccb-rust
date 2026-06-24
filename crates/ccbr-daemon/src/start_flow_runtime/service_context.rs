//! Mirrors Python `lib/ccbrd/start_flow_runtime/service_context.py`.
//!
//! Builds the start-flow CLI context and records namespace-level actions.

use std::path::{Path, PathBuf};

use ccbr_storage::paths::PathLayout;

/// Minimal mirror of `cli.models.ParsedStartCommand`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedStartCommand {
    pub project: Option<String>,
    pub agent_names: Vec<String>,
    pub restore: bool,
    pub auto_permission: bool,
}

impl ParsedStartCommand {
    pub fn new(
        project: Option<String>,
        agent_names: Vec<String>,
        restore: bool,
        auto_permission: bool,
    ) -> Self {
        Self {
            project,
            agent_names,
            restore,
            auto_permission,
        }
    }
}

/// Minimal mirror of `project.resolver.ProjectContext` for the start flow.
#[derive(Debug, Clone, PartialEq)]
pub struct StartFlowProjectContext {
    pub cwd: PathBuf,
    pub project_root: PathBuf,
    pub config_dir: PathBuf,
    pub project_id: String,
    pub source: String,
}

/// Minimal mirror of `cli.context.CliContext` for the start flow.
#[derive(Debug, Clone)]
pub struct StartFlowContext {
    pub command: ParsedStartCommand,
    pub cwd: PathBuf,
    pub project: StartFlowProjectContext,
    pub paths: PathLayout,
}

/// Build the start command and CLI context used by the start flow service.
///
/// Mirrors Python `build_start_context`.
pub fn build_start_context(
    project_root: impl AsRef<Path>,
    project_id: &str,
    paths: &PathLayout,
    requested_agents: &[String],
    restore: bool,
    auto_permission: bool,
) -> (ParsedStartCommand, StartFlowContext) {
    let project_root = project_root.as_ref().to_path_buf();
    let command = ParsedStartCommand::new(
        Some(project_root.to_string_lossy().to_string()),
        requested_agents.to_vec(),
        restore,
        auto_permission,
    );

    let context = StartFlowContext {
        command: command.clone(),
        cwd: project_root.clone(),
        project: StartFlowProjectContext {
            cwd: project_root.clone(),
            project_root: project_root.clone(),
            config_dir: paths.ccbr_dir().as_std_path().to_path_buf(),
            project_id: project_id.to_string(),
            source: "ccbrd".to_string(),
        },
        paths: paths.clone(),
    };

    (command, context)
}

/// Record the namespace ensure action if a tmux session is configured.
///
/// Mirrors Python `record_namespace_action`.
pub fn record_namespace_action(
    actions_taken: &mut Vec<String>,
    tmux_socket_path: Option<&str>,
    tmux_session_name: Option<&str>,
    namespace_epoch: Option<i64>,
) {
    if tmux_socket_path.is_none() || tmux_session_name.is_none() {
        return;
    }
    let epoch = namespace_epoch
        .map(|e| e.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    actions_taken.push(format!(
        "ensure_namespace:epoch={epoch},session={}",
        tmux_session_name.unwrap()
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(project_root: &str) -> PathLayout {
        PathLayout::new(project_root)
    }

    #[test]
    fn test_build_start_context() {
        let paths = test_paths("/tmp/ccbr-test");
        let agents = vec!["claude".to_string(), "codex".to_string()];
        let (command, context) = build_start_context(
            "/tmp/ccbr-test",
            "ccbr-test-id",
            &paths,
            &agents,
            true,
            false,
        );

        assert_eq!(command.project, Some("/tmp/ccbr-test".to_string()));
        assert_eq!(command.agent_names, agents);
        assert!(command.restore);
        assert!(!command.auto_permission);

        assert_eq!(context.command, command);
        assert_eq!(context.cwd, PathBuf::from("/tmp/ccbr-test"));
        assert_eq!(context.project.project_id, "ccbr-test-id");
        assert_eq!(context.project.source, "ccbrd");
        assert_eq!(
            context.project.config_dir,
            PathBuf::from("/tmp/ccbr-test/.ccbr")
        );
    }

    #[test]
    fn test_record_namespace_action_records_when_configured() {
        let mut actions = Vec::new();
        record_namespace_action(
            &mut actions,
            Some("/tmp/tmux.sock"),
            Some("ccbr-sess"),
            Some(7),
        );
        assert_eq!(actions, vec!["ensure_namespace:epoch=7,session=ccbr-sess"]);
    }

    #[test]
    fn test_record_namespace_action_skips_when_unconfigured() {
        let mut actions = Vec::new();
        record_namespace_action(&mut actions, None, Some("ccbr-sess"), Some(7));
        record_namespace_action(&mut actions, Some("/tmp/tmux.sock"), None, Some(7));
        record_namespace_action(&mut actions, None, None, Some(7));
        assert!(actions.is_empty());
    }

    #[test]
    fn test_record_namespace_action_unknown_epoch() {
        let mut actions = Vec::new();
        record_namespace_action(
            &mut actions,
            Some("/tmp/tmux.sock"),
            Some("ccbr-sess"),
            None,
        );
        assert_eq!(
            actions,
            vec!["ensure_namespace:epoch=unknown,session=ccbr-sess"]
        );
    }
}
