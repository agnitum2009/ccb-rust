from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

import pytest

from agents.config_loader import load_project_config
from ccbd.app import CcbdApp
from ccbd.app_runtime.service_graph import CcbdServiceGraphDependencies, build_ccbd_service_graph
from ccbd.lifecycle_report_store import CcbdShutdownReportStore, CcbdStartupReportStore
from ccbd.metrics import ControlPlaneMetrics
from ccbd.project_view import ProjectViewStateStore
from ccbd.reload_runtime_mount import AdditiveRuntimeMountResult
from ccbd.reload_transaction import publish_additive_reload_transaction
from ccbd.services import CcbdLifecycleStore, MountManager, OwnershipGuard, SnapshotWriter
from ccbd.services.lifecycle import build_lifecycle
from ccbd.services.project_namespace import ProjectNamespaceController
from ccbd.services.project_namespace_runtime import NamespacePatchApplyResult
from ccbd.services.project_namespace_state import ProjectNamespaceStateStore
from ccbd.services.start_policy import CcbdStartPolicyStore
from fault_injection import FaultInjectionService
from provider_core.catalog import build_default_provider_catalog
from provider_execution.registry import build_default_execution_registry
from provider_execution.service import ExecutionService
from provider_execution.state_store import ExecutionStateStore


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

NOW = '2026-05-29T00:00:00Z'


def test_reload_transaction_updates_signatures_then_publishes_graph(tmp_path: Path) -> None:
    app = _started_app(tmp_path / 'repo-publish-success', BASE_CONFIG)
    old_graph = app.service_graph
    new_graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)
    namespace = _namespace(app)

    result = publish_additive_reload_transaction(
        app,
        new_graph,
        namespace=namespace,
        namespace_patch_result=_namespace_patch_result(),
        runtime_mount_result=_runtime_mount_result(),
    )

    assert result.status == 'published'
    assert result.old_graph_version == 1
    assert result.published_graph_version == 2
    assert result.old_config_signature == old_graph.config_identity['config_signature']
    assert result.new_config_signature == new_graph.config_identity['config_signature']
    assert app.service_graph is new_graph
    assert app.config is new_graph.config
    assert app.config_identity == new_graph.config_identity
    assert app.registry is new_graph.registry
    assert app.control_plane_metrics.service_graph_version == 2
    lease = app.mount_manager.load_state()
    lifecycle = app.lifecycle_store.load()
    assert lease.config_signature == new_graph.config_identity['config_signature']
    assert app.lease.config_signature == new_graph.config_identity['config_signature']
    assert lifecycle.config_signature == new_graph.config_identity['config_signature']
    assert lifecycle.namespace_epoch == namespace.namespace_epoch
    assert result.diagnostics['graph_published'] is True
    assert result.diagnostics['lease_or_lifecycle_written'] is True
    assert result.diagnostics['config_watch_started'] is False
    assert result.diagnostics['unload_or_replace_executed'] is False


def test_reload_transaction_blocks_namespace_patch_failure_without_publish(tmp_path: Path) -> None:
    app = _started_app(tmp_path / 'repo-namespace-fail', BASE_CONFIG)
    old_graph = app.service_graph
    old_lease = app.mount_manager.load_state().to_record()
    old_lifecycle = app.lifecycle_store.load().to_record()
    new_graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)

    result = publish_additive_reload_transaction(
        app,
        new_graph,
        namespace=_namespace(app),
        namespace_patch_result=NamespacePatchApplyResult(
            status='failed',
            diagnostics={'reason': 'namespace_patch_failed'},
        ),
        runtime_mount_result=_runtime_mount_result(),
    )

    assert result.status == 'blocked'
    assert result.diagnostics['reason'] == 'namespace_patch_not_applied'
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False
    assert app.service_graph is old_graph
    assert app.config_identity == old_graph.config_identity
    assert app.mount_manager.load_state().to_record() == old_lease
    assert app.lifecycle_store.load().to_record() == old_lifecycle


def test_reload_transaction_blocks_runtime_mount_failure_without_publish(tmp_path: Path) -> None:
    app = _started_app(tmp_path / 'repo-runtime-fail', BASE_CONFIG)
    old_graph = app.service_graph
    old_lease = app.mount_manager.load_state().to_record()
    old_lifecycle = app.lifecycle_store.load().to_record()
    new_graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)

    result = publish_additive_reload_transaction(
        app,
        new_graph,
        namespace=_namespace(app),
        namespace_patch_result=_namespace_patch_result(),
        runtime_mount_result=AdditiveRuntimeMountResult(
            status='failed',
            requested_agents=('agent3',),
            runtime_authority_written_agents=('agent3',),
            partial=True,
            diagnostics={
                'reason': 'runtime_mount_failed',
                'graph_published': False,
                'lease_or_lifecycle_written': False,
            },
        ),
    )

    assert result.status == 'blocked'
    assert result.diagnostics['reason'] == 'runtime_mount_not_ready'
    assert result.runtime_mount['partial'] is True
    assert result.runtime_mount['runtime_authority_written_agents'] == ['agent3']
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False
    assert app.service_graph is old_graph
    assert app.mount_manager.load_state().to_record() == old_lease
    assert app.lifecycle_store.load().to_record() == old_lifecycle


def test_reload_transaction_signature_failure_does_not_publish_graph(tmp_path: Path) -> None:
    app = _started_app(tmp_path / 'repo-signature-fail', BASE_CONFIG)
    old_graph = app.service_graph
    old_lifecycle = app.lifecycle_store.load().to_record()
    new_graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)
    app.lifecycle_store.save(app.lifecycle_store.load().with_updates(owner_daemon_instance_id='other-daemon'))

    result = publish_additive_reload_transaction(
        app,
        new_graph,
        namespace=_namespace(app),
        namespace_patch_result=_namespace_patch_result(),
        runtime_mount_result=_runtime_mount_result(),
    )

    assert result.status == 'failed'
    assert result.diagnostics['reason'] == 'signature_handoff_failed'
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False
    assert app.service_graph is old_graph
    assert app.config_identity == old_graph.config_identity
    assert app.mount_manager.load_state().config_signature == old_graph.config_identity['config_signature']
    assert app.lifecycle_store.load().config_signature == old_lifecycle['config_signature']


def test_reload_transaction_lifecycle_write_failure_rolls_back_lease_signature(tmp_path: Path) -> None:
    app = _started_app(tmp_path / 'repo-lifecycle-write-fail', BASE_CONFIG)
    old_graph = app.service_graph
    new_graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)

    result = publish_additive_reload_transaction(
        app,
        new_graph,
        namespace=_namespace(app),
        namespace_patch_result=_namespace_patch_result(),
        runtime_mount_result=_runtime_mount_result(),
        update_lifecycle_config_signature_fn=lambda *_args, **_kwargs: (_ for _ in ()).throw(
            RuntimeError('lifecycle write failed')
        ),
    )

    assert result.status == 'failed'
    assert result.diagnostics['reason'] == 'signature_handoff_failed'
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False
    assert result.diagnostics['signature_rollback']['attempted'] is True
    assert result.diagnostics['signature_rollback']['complete'] is True
    assert result.lease['config_signature'] == old_graph.config_identity['config_signature']
    assert result.lifecycle['config_signature'] == old_graph.config_identity['config_signature']
    assert app.service_graph is old_graph
    assert app.config_identity == old_graph.config_identity
    assert app.mount_manager.load_state().config_signature == old_graph.config_identity['config_signature']
    assert app.lifecycle_store.load().config_signature == old_graph.config_identity['config_signature']


def test_reload_transaction_stale_lease_generation_blocks_before_publish(tmp_path: Path) -> None:
    app = _started_app(tmp_path / 'repo-stale-lease', BASE_CONFIG)
    old_graph = app.service_graph
    new_graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)
    app.mount_manager.mark_mounted(
        project_id=app.project_id,
        pid=app.pid,
        socket_path=app.paths.ccbd_socket_path,
        generation=2,
        config_signature=old_graph.config_identity['config_signature'],
        daemon_instance_id=app.daemon_instance_id,
    )

    result = publish_additive_reload_transaction(
        app,
        new_graph,
        namespace=_namespace(app),
        namespace_patch_result=_namespace_patch_result(),
        runtime_mount_result=_runtime_mount_result(),
    )

    assert result.status == 'failed'
    assert result.diagnostics['reason'] == 'signature_handoff_failed'
    assert 'generation changed' in result.diagnostics['error']
    assert result.diagnostics['graph_published'] is False
    assert app.service_graph is old_graph
    assert app.mount_manager.load_state().config_signature == old_graph.config_identity['config_signature']


def test_reload_transaction_publish_failure_rolls_back_signatures_and_keeps_app_graph_old(
    tmp_path: Path,
) -> None:
    app = _started_app(tmp_path / 'repo-publish-fail', BASE_CONFIG)
    old_graph = app.service_graph
    new_graph = _build_graph(app, ADD_AGENT_CONFIG, version=2)

    result = publish_additive_reload_transaction(
        app,
        new_graph,
        namespace=_namespace(app),
        namespace_patch_result=_namespace_patch_result(),
        runtime_mount_result=_runtime_mount_result(),
        publish_graph_fn=lambda _graph: (_ for _ in ()).throw(RuntimeError('publish failed')),
    )

    assert result.status == 'failed'
    assert result.diagnostics['reason'] == 'service_graph_publish_failed'
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False
    assert result.diagnostics['signature_rollback']['attempted'] is True
    assert result.diagnostics['signature_rollback']['complete'] is True
    assert app.service_graph is old_graph
    assert app.config_identity == old_graph.config_identity
    assert app.mount_manager.load_state().config_signature == old_graph.config_identity['config_signature']
    assert app.lifecycle_store.load().config_signature == old_graph.config_identity['config_signature']


def test_project_reload_non_dry_run_no_change_noops_after_publish_transaction_helper(tmp_path: Path) -> None:
    app = _started_app(tmp_path / 'repo-block-no-change', BASE_CONFIG)
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


def _started_app(project_root: Path, config_text: str) -> CcbdApp:
    app = CcbdApp(_project(project_root, config_text), clock=lambda: NOW, pid=4242)
    app.lease = app.mount_manager.mark_mounted(
        project_id=app.project_id,
        pid=app.pid,
        socket_path=app.paths.ccbd_socket_path,
        generation=1,
        started_at=NOW,
        config_signature=app.config_identity['config_signature'],
        keeper_pid=app.keeper_pid,
        daemon_instance_id=app.daemon_instance_id,
    )
    app.lifecycle_store.save(
        build_lifecycle(
            project_id=app.project_id,
            occurred_at=NOW,
            desired_state='running',
            phase='mounted',
            generation=1,
            keeper_pid=app.keeper_pid,
            owner_pid=app.pid,
            owner_daemon_instance_id=app.daemon_instance_id,
            config_signature=app.config_identity['config_signature'],
            socket_path=app.paths.ccbd_socket_path,
            namespace_epoch=3,
        )
    )
    return app


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
            start_policy_store=CcbdStartPolicyStore(app.paths),
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
            daemon_generation_getter=lambda: app.lease.generation if app.lease is not None else None,
            mount_missing_runtime_fn=lambda _agent_name: False,
            supervision_suspended_fn=lambda: False,
            version=version,
        )
    )


def _namespace_patch_result() -> NamespacePatchApplyResult:
    return NamespacePatchApplyResult(
        status='applied',
        agent_panes={'agent3': '%3'},
        preserved_before={'agent1': '%1', 'agent2': '%2'},
        preserved_after={'agent1': '%1', 'agent2': '%2'},
        diagnostics={
            'graph_published': False,
            'runtime_authority_written': False,
            'lease_or_lifecycle_written': False,
        },
    )


def _runtime_mount_result() -> AdditiveRuntimeMountResult:
    return AdditiveRuntimeMountResult(
        status='mounted',
        requested_agents=('agent3',),
        mounted_agents=('agent3',),
        runtime_authority_written_agents=('agent3',),
        preserved_runtime_unchanged_agents=('agent1', 'agent2'),
        diagnostics={
            'graph_published': False,
            'lease_or_lifecycle_written': False,
        },
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


def _project(project_root: Path, config_text: str) -> Path:
    project_root.mkdir(parents=True, exist_ok=True)
    config_path = project_root / '.ccb' / 'ccb.config'
    config_path.parent.mkdir(parents=True, exist_ok=True)
    config_path.write_text(config_text, encoding='utf-8')
    return project_root
