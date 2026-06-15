from __future__ import annotations

from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace

import pytest

from agents.config_loader import load_project_config
from agents.models import AgentRuntime, AgentState
from ccbd.app import CcbdApp
from ccbd.app_runtime.service_graph import CcbdServiceGraphDependencies, build_ccbd_service_graph
from ccbd.lifecycle_report_store import CcbdShutdownReportStore, CcbdStartupReportStore
from ccbd.metrics import ControlPlaneMetrics
from ccbd.models import CcbdStartupAgentResult
from ccbd.project_view import ProjectViewStateStore
from ccbd.reload_runtime_mount import run_additive_agent_mounts
from ccbd.services import CcbdLifecycleStore, MountManager, OwnershipGuard, SnapshotWriter
from ccbd.services.project_namespace import ProjectNamespaceController
from ccbd.services.project_namespace_state import ProjectNamespaceStateStore
from ccbd.services.start_policy import CcbdStartPolicy, CcbdStartPolicyStore
from ccbd.start_flow_runtime import StartFlowSummary
from fault_injection import FaultInjectionService
from provider_core.catalog import build_default_provider_catalog
from provider_execution.registry import build_default_execution_registry
from provider_execution.service import ExecutionService
from provider_execution.state_store import ExecutionStateStore
from storage.paths import PathLayout


BASE_CONFIG = """version = 2
entry_window = "main"

[windows]
main = "agent1:codex, agent2:claude"

[ui.sidebar]
mode = "every_window"
width = "15%"
bottom_height = 20
"""

ADD_AGENT_CONFIG = BASE_CONFIG.replace(
    'agent1:codex, agent2:claude',
    'agent1:codex, (agent2:claude; agent3:codex)',
)

ADD_AGENT_WITH_EXTRA_RUNTIME_CONFIG = BASE_CONFIG.replace(
    'agent1:codex, agent2:claude',
    'agent1:codex, agent2:claude, agent4:codex, agent3:codex',
)

NOW = '2026-05-29T00:00:00Z'


def test_additive_runtime_mount_helper_mounts_only_new_agent(tmp_path: Path, monkeypatch) -> None:
    project_root = _project(tmp_path / 'repo-runtime-mount', ADD_AGENT_CONFIG)
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)
    _seed_runtime(graph.runtime_service, 'agent1', pane_id='%1')
    _seed_runtime(graph.runtime_service, 'agent2', pane_id='%2')
    patch_result = _patch_result(agent_panes={'agent3': '%3'})
    namespace = _namespace(app)
    calls: list[dict[str, object]] = []
    _forbid_transaction_side_effects(app, monkeypatch)
    _forbid_namespace_recreate_paths(app, monkeypatch)

    def _fake_start_flow(**kwargs):
        calls.append(kwargs)
        assert kwargs['runtime_service'] is graph.runtime_service
        assert kwargs['requested_agents'] == ('agent3',)
        assert kwargs['namespace_agent_panes'] == {'agent3': '%3'}
        assert kwargs['cleanup_tmux_orphans'] is False
        assert kwargs['interactive_tmux_layout'] is True
        assert kwargs['restore'] is True
        assert kwargs['auto_permission'] is True
        kwargs['runtime_service'].attach(
            agent_name='agent3',
            workspace_path=str(app.paths.workspace_path('agent3')),
            backend_type='pane-backed',
            runtime_ref='tmux:%3',
            session_ref='session-agent3',
            health='healthy',
            provider='codex',
            terminal_backend='tmux',
            pane_id='%3',
            active_pane_id='%3',
            pane_state='alive',
            tmux_socket_path=namespace.tmux_socket_path,
            tmux_window_name='main',
            slot_key='agent3',
            window_id=namespace.workspace_window_id,
            workspace_epoch=namespace.workspace_epoch,
            lifecycle_state='idle',
            managed_by='ccbd',
            binding_source='provider-session',
        )
        return StartFlowSummary(
            project_root=str(project_root),
            project_id=app.project_id,
            started=('agent3',),
            socket_path=str(app.paths.ccbd_socket_path),
            actions_taken=('use_namespace_topology:agent3', 'launch_runtime:agent3'),
            agent_results=(
                CcbdStartupAgentResult(
                    agent_name='agent3',
                    provider='codex',
                    action='launched',
                    health='healthy',
                    workspace_path=str(app.paths.workspace_path('agent3')),
                    runtime_ref='tmux:%3',
                    session_ref='session-agent3',
                    lifecycle_state='idle',
                    binding_source='provider-session',
                    terminal_backend='tmux',
                    tmux_socket_path=namespace.tmux_socket_path,
                    tmux_window_name='main',
                    pane_id='%3',
                    active_pane_id='%3',
                    pane_state='alive',
                ),
            ),
        )

    before_agent1 = graph.registry.get('agent1').to_record()
    before_agent2 = graph.registry.get('agent2').to_record()

    result = run_additive_agent_mounts(
        app,
        graph,
        namespace=namespace,
        patch_result=patch_result,
        run_start_flow_fn=_fake_start_flow,
    )

    assert result.status == 'mounted'
    assert result.requested_agents == ('agent3',)
    assert result.mounted_agents == ('agent3',)
    assert result.runtime_authority_written_agents == ('agent3',)
    assert result.preserved_runtime_unchanged_agents == ('agent1', 'agent2')
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False
    assert result.diagnostics['cleanup_tmux_orphans'] is False
    assert result.diagnostics['config_watch_started'] is False
    assert graph.registry.get('agent1').to_record() == before_agent1
    assert graph.registry.get('agent2').to_record() == before_agent2
    assert graph.registry.get('agent3').pane_id == '%3'
    assert calls and calls[0]['project_id'] == app.project_id
    assert app.service_graph.version == 1


def test_additive_runtime_mount_failure_does_not_publish_or_update_lease_lifecycle(
    tmp_path: Path,
    monkeypatch,
) -> None:
    project_root = _project(tmp_path / 'repo-runtime-mount-fail', ADD_AGENT_CONFIG)
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)
    _seed_runtime(graph.runtime_service, 'agent1', pane_id='%1')
    _seed_runtime(graph.runtime_service, 'agent2', pane_id='%2')
    _forbid_transaction_side_effects(app, monkeypatch)
    _forbid_namespace_recreate_paths(app, monkeypatch)

    def _failing_start_flow(**_kwargs):
        raise RuntimeError('provider launch failed')

    result = run_additive_agent_mounts(
        app,
        graph,
        namespace=_namespace(app),
        patch_result=_patch_result(agent_panes={'agent3': '%3'}),
        run_start_flow_fn=_failing_start_flow,
    )

    assert result.status == 'failed'
    assert result.partial is False
    assert result.runtime_authority_written_agents == ()
    assert result.diagnostics['reason'] == 'runtime_mount_failed'
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False
    assert graph.registry.get('agent3') is None
    assert app.service_graph.version == 1


def test_additive_runtime_mount_partial_failure_writes_only_new_agent_authority(
    tmp_path: Path,
    monkeypatch,
) -> None:
    project_root = _project(tmp_path / 'repo-runtime-mount-partial-fail', ADD_AGENT_CONFIG)
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)
    _seed_runtime(graph.runtime_service, 'agent1', pane_id='%1')
    _seed_runtime(graph.runtime_service, 'agent2', pane_id='%2')
    _forbid_transaction_side_effects(app, monkeypatch)
    _forbid_namespace_recreate_paths(app, monkeypatch)
    before_agent1 = graph.registry.get('agent1').to_record()
    before_agent2 = graph.registry.get('agent2').to_record()

    def _partially_failing_start_flow(**kwargs):
        kwargs['runtime_service'].attach(
            agent_name='agent3',
            workspace_path=str(app.paths.workspace_path('agent3')),
            backend_type='pane-backed',
            runtime_ref='tmux:%3',
            session_ref='session-agent3',
            health='degraded',
            provider='codex',
            pane_id='%3',
            active_pane_id='%3',
            lifecycle_state='degraded',
            managed_by='ccbd',
            binding_source='provider-session',
        )
        raise RuntimeError('provider degraded after authority write')

    result = run_additive_agent_mounts(
        app,
        graph,
        namespace=_namespace(app),
        patch_result=_patch_result(agent_panes={'agent3': '%3'}),
        run_start_flow_fn=_partially_failing_start_flow,
    )

    assert result.status == 'failed'
    assert result.partial is True
    assert result.runtime_authority_written_agents == ('agent3',)
    assert result.diagnostics['runtime_authority_scope'] == 'new_agents_only'
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False
    assert graph.registry.get('agent1').to_record() == before_agent1
    assert graph.registry.get('agent2').to_record() == before_agent2
    assert graph.registry.get('agent3').pane_id == '%3'
    assert app.service_graph.version == 1


def test_additive_runtime_mount_blocks_preserved_agent_target_without_mutation(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-runtime-mount-preserved', ADD_AGENT_CONFIG)
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)
    _seed_runtime(graph.runtime_service, 'agent1', pane_id='%1')

    result = run_additive_agent_mounts(
        app,
        graph,
        namespace=_namespace(app),
        patch_result=_patch_result(agent_panes={'agent1': '%1'}),
        run_start_flow_fn=lambda **_kwargs: (_ for _ in ()).throw(AssertionError('must not mount preserved agent')),
    )

    assert result.status == 'blocked'
    assert result.diagnostics['reason'] == 'preserved_agent_mount_blocked'
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False
    assert app.service_graph.version == 1


def test_additive_runtime_mount_blocks_agent_with_existing_runtime_authority(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-runtime-mount-existing', ADD_AGENT_CONFIG)
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)
    _seed_runtime(graph.runtime_service, 'agent3', pane_id='%old')

    result = run_additive_agent_mounts(
        app,
        graph,
        namespace=_namespace(app),
        patch_result=SimpleNamespace(status='applied', agent_panes={'agent3': '%3'}, preserved_before={}),
        run_start_flow_fn=lambda **_kwargs: (_ for _ in ()).throw(AssertionError('must not remount existing runtime')),
    )

    assert result.status == 'blocked'
    assert result.requested_agents == ('agent3',)
    assert result.diagnostics['reason'] == 'runtime_authority_already_exists'
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False
    assert graph.registry.get('agent3').pane_id == '%old'


def test_additive_runtime_mount_reuses_retired_same_name_runtime_residue(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-runtime-mount-retired-residue', ADD_AGENT_CONFIG)
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)
    old = _seed_runtime(graph.runtime_service, 'agent3', pane_id='%old')
    graph.registry.upsert_authority(
        replace(
            old,
            state=AgentState.STOPPED,
            pid=None,
            runtime_ref=None,
            session_ref=None,
            socket_path=None,
            health='stopped',
            runtime_pid=None,
            pane_id=None,
            active_pane_id=None,
            pane_state=None,
            desired_state='stopped',
            reconcile_state='stopped',
            session_file=str(app.paths.ccb_dir / '.codex-agent3-session'),
            session_id='old-session-id',
        )
    )

    def _fake_start_flow(**kwargs):
        kwargs['runtime_service'].attach(
            agent_name='agent3',
            workspace_path=str(app.paths.workspace_path('agent3')),
            backend_type='pane-backed',
            runtime_ref='tmux:%3',
            session_ref='session-agent3-new',
            health='healthy',
            provider='codex',
            terminal_backend='tmux',
            pane_id='%3',
            active_pane_id='%3',
            pane_state='alive',
            tmux_socket_path=str(app.paths.ccbd_tmux_socket_path),
            tmux_window_name='main',
            slot_key='agent3',
            lifecycle_state='idle',
            managed_by='ccbd',
            binding_source='provider-session',
        )
        return StartFlowSummary(
            project_root=str(project_root),
            project_id=app.project_id,
            started=('agent3',),
            socket_path=str(app.paths.ccbd_socket_path),
        )

    result = run_additive_agent_mounts(
        app,
        graph,
        namespace=_namespace(app),
        patch_result=SimpleNamespace(status='applied', agent_panes={'agent3': '%3'}, preserved_before={}),
        run_start_flow_fn=_fake_start_flow,
    )

    assert result.status == 'mounted'
    assert result.requested_agents == ('agent3',)
    assert result.mounted_agents == ('agent3',)
    assert result.runtime_authority_written_agents == ('agent3',)
    assert result.diagnostics['reason'] is None
    mounted = graph.registry.get('agent3')
    assert mounted.state is AgentState.IDLE
    assert mounted.desired_state == 'mounted'
    assert mounted.runtime_ref == 'tmux:%3'
    assert mounted.session_ref == 'session-agent3-new'
    assert mounted.session_file == str(app.paths.ccb_dir / '.codex-agent3-session')
    assert mounted.session_id == 'old-session-id'
    assert mounted.pane_id == '%3'


def test_additive_runtime_mount_detects_preserved_runtime_authority_change(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-runtime-mount-preserved-change', ADD_AGENT_CONFIG)
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)
    _seed_runtime(graph.runtime_service, 'agent1', pane_id='%1')
    _seed_runtime(graph.runtime_service, 'agent2', pane_id='%2')

    def _bad_start_flow(**kwargs):
        current = kwargs['runtime_service']._registry.get('agent2')
        kwargs['runtime_service'].mutate_runtime_authority(current, pane_id='%99')
        kwargs['runtime_service'].attach(
            agent_name='agent3',
            workspace_path=str(app.paths.workspace_path('agent3')),
            backend_type='pane-backed',
            runtime_ref='tmux:%3',
            session_ref='session-agent3',
            health='healthy',
            provider='codex',
            pane_id='%3',
            active_pane_id='%3',
            lifecycle_state='idle',
            managed_by='ccbd',
            binding_source='provider-session',
        )
        return StartFlowSummary(
            project_root=str(project_root),
            project_id=app.project_id,
            started=('agent3',),
            socket_path=str(app.paths.ccbd_socket_path),
        )

    result = run_additive_agent_mounts(
        app,
        graph,
        namespace=_namespace(app),
        patch_result=_patch_result(agent_panes={'agent3': '%3'}),
        run_start_flow_fn=_bad_start_flow,
    )

    assert result.status == 'failed'
    assert result.diagnostics['reason'] == 'preserved_runtime_authority_changed'
    assert result.diagnostics['runtime_authority_scope'] == 'preserved_agent_changed'
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False


def test_additive_runtime_mount_guards_existing_non_target_runtime_authority(tmp_path: Path) -> None:
    project_root = _project(
        tmp_path / 'repo-runtime-mount-non-target-change',
        ADD_AGENT_WITH_EXTRA_RUNTIME_CONFIG,
    )
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    graph = _build_graph(app, ADD_AGENT_WITH_EXTRA_RUNTIME_CONFIG, version=2)
    _seed_runtime(graph.runtime_service, 'agent1', pane_id='%1')
    _seed_runtime(graph.runtime_service, 'agent2', pane_id='%2')
    _seed_runtime(graph.runtime_service, 'agent4', pane_id='%4')

    def _bad_start_flow(**kwargs):
        current = kwargs['runtime_service']._registry.get('agent4')
        kwargs['runtime_service'].mutate_runtime_authority(current, pane_id='%44')
        kwargs['runtime_service'].attach(
            agent_name='agent3',
            workspace_path=str(app.paths.workspace_path('agent3')),
            backend_type='pane-backed',
            runtime_ref='tmux:%3',
            session_ref='session-agent3',
            health='healthy',
            provider='codex',
            pane_id='%3',
            active_pane_id='%3',
            lifecycle_state='idle',
            managed_by='ccbd',
            binding_source='provider-session',
        )
        return StartFlowSummary(
            project_root=str(project_root),
            project_id=app.project_id,
            started=('agent3',),
            socket_path=str(app.paths.ccbd_socket_path),
        )

    result = run_additive_agent_mounts(
        app,
        graph,
        namespace=_namespace(app),
        patch_result=_patch_result(agent_panes={'agent3': '%3'}),
        run_start_flow_fn=_bad_start_flow,
    )

    assert result.status == 'failed'
    assert result.diagnostics['reason'] == 'preserved_runtime_authority_changed'
    assert result.diagnostics['preserved_runtime_changed_agents'] == ['agent4']
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False


def test_additive_runtime_mount_blocks_foreign_namespace_project(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-runtime-mount-foreign-namespace', ADD_AGENT_CONFIG)
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)
    namespace = _namespace(app)
    namespace.project_id = 'foreign-project'

    result = run_additive_agent_mounts(
        app,
        graph,
        namespace=namespace,
        patch_result=_patch_result(agent_panes={'agent3': '%3'}),
        run_start_flow_fn=lambda **_kwargs: (_ for _ in ()).throw(AssertionError('must not mount foreign namespace')),
    )

    assert result.status == 'blocked'
    assert result.diagnostics['reason'] == 'namespace_project_mismatch'
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False


def test_project_reload_non_dry_run_no_change_noops_after_runtime_mount_helper(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-runtime-mount-block-no-change', BASE_CONFIG)
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    old_graph = app.service_graph

    payload = app.socket_server._handlers['project_reload_config']({'dry_run': False})

    assert payload['status'] == 'noop'
    assert payload['stage'] == 'no_op'
    assert payload['plan_class'] == 'no_change'
    assert payload['diagnostics']['reason'] == 'no_change'
    assert payload['diagnostics']['graph_published'] is False
    assert payload['diagnostics']['reason'] == 'no_change'
    assert app.service_graph is old_graph
    assert app.control_plane_metrics.last_reload_duration_s is not None


def _build_graph(app: CcbdApp, config_text: str, *, version: int):
    config = load_project_config(_project(app.project_root, config_text)).config
    return build_ccbd_service_graph(
        CcbdServiceGraphDependencies(
            project_root=app.project_root,
            project_id=app.project_id,
            paths=app.paths,
            config=config,
            provider_catalog=build_default_provider_catalog(),
            mount_manager=MountManager(app.paths, clock=app.clock),
            lifecycle_store=CcbdLifecycleStore(app.paths),
            restore_store=app.restore_store,
            namespace_state_store=ProjectNamespaceStateStore(app.paths),
            project_view_state_store=ProjectViewStateStore(app.paths, project_id=app.project_id),
            project_namespace=ProjectNamespaceController(app.paths, app.project_id, clock=app.clock),
            ownership_guard=OwnershipGuard(app.paths, app.mount_manager, clock=app.clock),
            startup_report_store=CcbdStartupReportStore(app.paths),
            shutdown_report_store=CcbdShutdownReportStore(app.paths),
            start_policy_store=_start_policy_store(app.paths),
            execution_service=ExecutionService(
                build_default_execution_registry(),
                clock=app.clock,
                state_store=ExecutionStateStore(app.paths),
                fault_injection=FaultInjectionService(app.paths, clock=app.clock),
            ),
            snapshot_writer=SnapshotWriter(app.paths, clock=app.clock),
            control_plane_metrics=ControlPlaneMetrics(),
            clock=app.clock,
            request_timeout_s=0.0,
            daemon_generation_getter=lambda: None,
            mount_missing_runtime_fn=lambda _agent_name: False,
            supervision_suspended_fn=lambda: False,
            version=version,
        )
    )


def _seed_runtime(runtime_service, agent_name: str, *, pane_id: str) -> AgentRuntime:
    return runtime_service.attach(
        agent_name=agent_name,
        workspace_path=str(runtime_service._layout.workspace_path(agent_name)),
        backend_type='pane-backed',
        runtime_ref=f'tmux:{pane_id}',
        session_ref=f'session-{agent_name}',
        health='healthy',
        provider='codex',
        terminal_backend='tmux',
        pane_id=pane_id,
        active_pane_id=pane_id,
        pane_state='alive',
        tmux_socket_path=str(runtime_service._layout.ccbd_tmux_socket_path),
        tmux_window_name='main',
        slot_key=agent_name,
        lifecycle_state='idle',
        managed_by='ccbd',
        binding_source='provider-session',
    )


def _patch_result(*, agent_panes: dict[str, str]):
    return SimpleNamespace(
        status='applied',
        agent_panes=agent_panes,
        preserved_before={'agent1': '%1', 'agent2': '%2'},
    )


def _namespace(app: CcbdApp):
    return SimpleNamespace(
        project_id=app.project_id,
        namespace_epoch=3,
        tmux_socket_path=str(app.paths.ccbd_tmux_socket_path),
        tmux_session_name=app.paths.ccbd_tmux_session_name,
        workspace_window_name='main',
        workspace_window_id='@main',
        workspace_epoch=1,
        ui_attachable=True,
    )


def _start_policy_store(paths: PathLayout) -> CcbdStartPolicyStore:
    store = CcbdStartPolicyStore(paths)
    store.save(
        CcbdStartPolicy(
            project_id='proj-test',
            auto_permission=True,
            recovery_restore=True,
            last_started_at=NOW,
            source='test',
        )
    )
    return store


def _forbid_transaction_side_effects(app: CcbdApp, monkeypatch) -> None:
    def _fail(*_args, **_kwargs):
        raise AssertionError('runtime mount helper must not publish graph or write lease/lifecycle')

    monkeypatch.setattr(app, 'publish_service_graph', _fail, raising=False)
    monkeypatch.setattr(app.mount_manager, 'mark_mounted', _fail, raising=False)
    monkeypatch.setattr(app.lifecycle_store, 'save', _fail, raising=False)


def _forbid_namespace_recreate_paths(app: CcbdApp, monkeypatch) -> None:
    def _fail(*_args, **_kwargs):
        raise AssertionError('runtime mount helper must not call full namespace mutation paths')

    for method_name in ('ensure', 'destroy', 'reflow_workspace'):
        monkeypatch.setattr(app.project_namespace, method_name, _fail, raising=False)
    monkeypatch.setattr('ccbd.services.project_namespace_runtime.ensure.ensure_project_namespace', _fail, raising=False)
    monkeypatch.setattr('ccbd.services.project_namespace_runtime.backend.kill_server', _fail, raising=False)
    monkeypatch.setattr('ccbd.services.project_namespace_runtime.backend.kill_window', _fail, raising=False)
    monkeypatch.setattr('ccbd.services.project_namespace_runtime.reflow.reflow_project_workspace', _fail, raising=False)


def _project(project_root: Path, config_text: str) -> Path:
    project_root.mkdir(parents=True, exist_ok=True)
    config_path = project_root / '.ccb' / 'ccb.config'
    config_path.parent.mkdir(parents=True, exist_ok=True)
    config_path.write_text(config_text, encoding='utf-8')
    return project_root
