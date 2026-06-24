//! Mirrors Python `test/test_daemon_start_preparation.py`.
//!
//! Regression tests for `ccbr_daemon::start_preparation::prepare_start_agents`.

use std::path::Path;

use camino::Utf8Path;
use ccbr_agents::models::{
    AgentSpec, PermissionMode, ProjectConfig, ProviderProfileSpec, QueuePolicy, RestoreMode,
    RuntimeMode, WorkspaceMode,
};
use ccbr_agents::store::{AgentRestoreStore, AgentSpecStore};
use ccbr_daemon::start_preparation::{
    prepare_start_agents, AgentBinding, ProjectBindingFilterFn, ResolveAgentBindingFn,
    RestoreStateBuilder, StartContext,
};
use ccbr_storage::paths::PathLayout;

fn tmp_dir() -> (tempfile::TempDir, camino::Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let utf8 = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    (dir, utf8)
}

fn spec(name: &str, provider: &str) -> AgentSpec {
    AgentSpec {
        name: name.into(),
        provider: provider.into(),
        target: ".".into(),
        workspace_mode: WorkspaceMode::Inplace,
        workspace_root: None,
        runtime_mode: RuntimeMode::PaneBacked,
        restore_default: RestoreMode::Auto,
        permission_default: PermissionMode::Manual,
        queue_policy: QueuePolicy::SerialPerAgent,
        workspace_path: None,
        workspace_group: None,
        provider_command_template: None,
        model: None,
        startup_args: Vec::new(),
        env: Default::default(),
        api: Default::default(),
        provider_profile: ProviderProfileSpec::default(),
        branch_template: None,
        labels: Vec::new(),
        description: None,
        role: None,
        watch_paths: Vec::new(),
    }
}

struct DummyResolve;

impl ResolveAgentBindingFn for DummyResolve {
    fn resolve(
        &self,
        _provider: &str,
        agent_name: &str,
        workspace_path: &Utf8Path,
        _project_root: &Path,
        _ensure_usable: bool,
    ) -> Option<AgentBinding> {
        Some(AgentBinding {
            agent_name: agent_name.into(),
            workspace_path: workspace_path.as_std_path().to_path_buf(),
        })
    }
}

struct DummyFilter;

impl ProjectBindingFilterFn for DummyFilter {
    fn filter(
        &self,
        raw_binding: Option<&AgentBinding>,
        _cmd_enabled: bool,
        _tmux_socket_path: Option<&str>,
        _tmux_session_name: Option<&str>,
        _workspace_window_id: Option<&str>,
        _agent_name: &str,
        _project_id: &str,
        _window_name: Option<&str>,
    ) -> Option<AgentBinding> {
        raw_binding.cloned()
    }
}

struct RejectingFilter;

impl ProjectBindingFilterFn for RejectingFilter {
    fn filter(
        &self,
        _raw_binding: Option<&AgentBinding>,
        _cmd_enabled: bool,
        _tmux_socket_path: Option<&str>,
        _tmux_session_name: Option<&str>,
        _workspace_window_id: Option<&str>,
        _agent_name: &str,
        _project_id: &str,
        _window_name: Option<&str>,
    ) -> Option<AgentBinding> {
        None
    }
}

struct DummyRestoreBuilder;

impl RestoreStateBuilder for DummyRestoreBuilder {
    fn build(&self, mode: &str) -> ccbr_agents::models::AgentRestoreState {
        ccbr_agents::restore::build_restore_state(mode)
    }
}

#[test]
fn test_prepare_start_agents_persists_spec_restore_and_provider_home() {
    let (_tmp, project_root) = tmp_dir();
    let source_home = project_root.join("source-home");
    std::fs::create_dir_all(&source_home).unwrap();

    // Isolate provider source home so the test does not depend on the real $HOME.
    std::env::set_var("HOME", source_home.as_str());
    std::env::remove_var("CCB_SOURCE_HOME");

    let layout = PathLayout::new(&project_root);

    let mut config = ProjectConfig::default();
    config.agents.insert("a1".into(), spec("a1", "droid"));

    let context = StartContext {
        restore_mode: None,
        auto_permission: false,
        project_id: "test-project".into(),
        project_root: project_root.as_std_path().to_path_buf(),
    };

    let prepared = prepare_start_agents(
        &["a1".into()],
        &config,
        &layout,
        &context,
        &context.project_root,
        &context.project_id,
        None,
        None,
        None,
        &DummyResolve,
        &DummyFilter,
        &DummyRestoreBuilder,
    )
    .unwrap();

    assert_eq!(prepared.len(), 1);
    let agent = &prepared[0];
    assert_eq!(agent.agent_name, "a1");
    assert_eq!(agent.spec.name, "a1");
    assert!(!agent.stale_binding);
    assert_eq!(
        agent.plan.workspace_path,
        project_root.as_std_path().to_path_buf()
    );

    // Spec persisted.
    let spec_store = AgentSpecStore::new(layout.clone());
    assert!(spec_store.load("a1").unwrap().is_some());

    // Restore state persisted.
    let restore_store = AgentRestoreStore::new(layout.clone());
    assert!(restore_store.load("a1").unwrap().is_some());

    // Provider runtime and state home materialized.
    assert!(layout.agent_provider_runtime_dir("a1", "droid").is_dir());
    let state_home = layout.agent_provider_state_dir("a1", "droid").join("home");
    assert!(state_home.is_dir());
}

#[test]
fn test_prepare_start_agents_detects_stale_binding() {
    let (_tmp, project_root) = tmp_dir();
    let source_home = project_root.join("source-home");
    std::fs::create_dir_all(&source_home).unwrap();

    std::env::set_var("HOME", source_home.as_str());
    std::env::remove_var("CCB_SOURCE_HOME");

    let layout = PathLayout::new(&project_root);

    let mut config = ProjectConfig::default();
    config.agents.insert("a1".into(), spec("a1", "droid"));

    let context = StartContext {
        restore_mode: None,
        auto_permission: false,
        project_id: "test-project".into(),
        project_root: project_root.as_std_path().to_path_buf(),
    };

    let prepared = prepare_start_agents(
        &["a1".into()],
        &config,
        &layout,
        &context,
        &context.project_root,
        &context.project_id,
        Some("/tmp/tmux.sock"),
        None,
        None,
        &DummyResolve,
        &RejectingFilter,
        &DummyRestoreBuilder,
    )
    .unwrap();

    assert_eq!(prepared.len(), 1);
    let agent = &prepared[0];
    assert!(agent.raw_binding.is_some());
    assert!(agent.binding.is_none());
    assert!(agent.stale_binding);
}

#[test]
fn test_prepare_start_agents_resolves_window_name() {
    use ccbr_agents::models::WindowSpec;

    let (_tmp, project_root) = tmp_dir();
    let source_home = project_root.join("source-home");
    std::fs::create_dir_all(&source_home).unwrap();

    std::env::set_var("HOME", source_home.as_str());
    std::env::remove_var("CCB_SOURCE_HOME");

    let layout = PathLayout::new(&project_root);

    let mut config = ProjectConfig::default();
    config.agents.insert("a1".into(), spec("a1", "droid"));
    config.windows = Some(vec![WindowSpec {
        name: "review".into(),
        order: 0,
        layout_spec: "a1".into(),
        agent_names: vec!["a1".into()],
    }]);

    let context = StartContext {
        restore_mode: None,
        auto_permission: false,
        project_id: "test-project".into(),
        project_root: project_root.as_std_path().to_path_buf(),
    };

    let prepared = prepare_start_agents(
        &["a1".into()],
        &config,
        &layout,
        &context,
        &context.project_root,
        &context.project_id,
        None,
        None,
        None,
        &DummyResolve,
        &DummyFilter,
        &DummyRestoreBuilder,
    )
    .unwrap();

    assert_eq!(prepared[0].window_name, Some("review".to_string()));
}
