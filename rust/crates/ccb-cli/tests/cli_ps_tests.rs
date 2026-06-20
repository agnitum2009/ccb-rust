use std::path::PathBuf;

use ccb_agents::models::{AgentRuntime, AgentState, RuntimeBindingSource};
use ccb_agents::store::AgentRuntimeStore;
use ccb_cli::context::{CliContext, CliContextBuilder};
use ccb_cli::models::{ParsedCommand, ParsedPsCommand};
use ccb_cli::services::ps::ps_summary;
use ccb_storage::paths::PathLayout;
use serde_json::Value;
use tempfile::TempDir;

fn build_context(project_root: PathBuf) -> CliContext {
    CliContextBuilder::new(ParsedCommand::Ps(ParsedPsCommand::new(None)))
        .cwd(project_root.clone())
        .build()
        .expect("build context")
}

fn write_runtime(paths: &PathLayout, _agent_name: &str, runtime: &AgentRuntime) {
    let store = AgentRuntimeStore::new(paths.clone());
    store.save(runtime).expect("save runtime");
}

#[test]
fn test_ps_summary_includes_tmux_socket_and_pane_observation() {
    let tmp = TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-ps");
    let ccb_dir = project_root.join(".ccb");
    std::fs::create_dir_all(&ccb_dir).unwrap();
    std::fs::write(ccb_dir.join("ccb.config"), "agent1:codex\n").unwrap();

    let context = build_context(project_root.clone());
    let workspace_path = context.paths.workspace_path("agent1", None);

    let runtime = AgentRuntime {
        agent_name: "agent1".into(),
        state: AgentState::Idle,
        queue_depth: 0,
        backend_type: "pane_backed".into(),
        binding_source: RuntimeBindingSource::ProviderSession,
        runtime_ref: Some("tmux:%52".into()),
        session_ref: Some("session-2".into()),
        session_file: None,
        session_id: Some("session-2".into()),
        workspace_path: Some(workspace_path.to_string()),
        project_id: context.project.project_id.clone(),
        terminal_backend: Some("tmux".into()),
        tmux_socket_name: Some("sock-a".into()),
        tmux_socket_path: Some("/tmp/ccb.sock".into()),
        tmux_window_name: Some("main".into()),
        tmux_window_id: Some("@1".into()),
        pane_id: Some("%41".into()),
        active_pane_id: Some("%52".into()),
        pane_title_marker: Some("CCB-agent1-demo".into()),
        pane_state: Some("alive".into()),
        ..Default::default()
    };
    write_runtime(&context.paths, "agent1", &runtime);

    let summary = ps_summary(&context, &ParsedPsCommand::new(None));
    let summary: Value = serde_json::to_value(summary).unwrap();

    assert_eq!(summary["ccbd_state"], "mounted");
    let agents = summary["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 1);
    let agent = &agents[0];
    assert_eq!(agent["agent_name"], "agent1");
    assert_eq!(agent["provider"], "codex");
    assert_eq!(agent["runtime_ref"], "tmux:%52");
    assert_eq!(agent["session_ref"], "session-2");
    assert_eq!(agent["tmux_socket_name"], "sock-a");
    assert_eq!(agent["tmux_socket_path"], "/tmp/ccb.sock");
    assert_eq!(agent["tmux_window_name"], "main");
    assert_eq!(agent["tmux_window_id"], "@1");
    assert_eq!(agent["pane_id"], "%41");
    assert_eq!(agent["active_pane_id"], "%52");
    assert_eq!(agent["pane_title_marker"], "CCB-agent1-demo");
    assert_eq!(agent["pane_state"], "alive");
}
