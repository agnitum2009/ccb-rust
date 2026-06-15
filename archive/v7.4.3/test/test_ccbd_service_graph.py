from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

from agents.config_loader import load_project_config
from agents.store import AgentRestoreStore
from ccbd.api_models import AcceptedJobReceipt, JobStatus, SubmitReceipt, TargetKind
from ccbd.app import CcbdApp
from ccbd.app_runtime.service_graph import (
    SERVICE_GRAPH_RETAINED_COUNT_SCOPE,
    CcbdServiceGraphDependencies,
    build_ccbd_service_graph,
)
from ccbd.lifecycle_report_store import CcbdShutdownReportStore, CcbdStartupReportStore
from ccbd.metrics import ControlPlaneMetrics
from ccbd.models import LeaseHealth, LeaseInspection
from ccbd.project_view import ProjectViewStateStore
from ccbd.services import CcbdLifecycleStore, MountManager, OwnershipGuard, SnapshotWriter
from ccbd.services.project_namespace import ProjectNamespaceController
from ccbd.services.project_namespace_state import ProjectNamespaceStateStore
from ccbd.services.start_policy import CcbdStartPolicyStore
from fault_injection import FaultInjectionService
from project.ids import compute_project_id
from provider_core.catalog import build_default_provider_catalog
from provider_execution.registry import build_default_execution_registry
from provider_execution.service import ExecutionService
from provider_execution.state_store import ExecutionStateStore
from storage.paths import PathLayout

NOW = '2026-05-29T00:00:00Z'


def test_service_graph_can_be_built_twice_without_writing_runtime_authority(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-build-twice')
    paths = PathLayout(project_root)
    paths.ensure_runtime_state_root(created_at=NOW)
    config = load_project_config(project_root).config
    project_id = compute_project_id(project_root)
    provider_catalog = build_default_provider_catalog()
    mount_manager = MountManager(paths, clock=lambda: NOW)
    lifecycle_store = CcbdLifecycleStore(paths)
    restore_store = AgentRestoreStore(paths)
    startup_report_store = CcbdStartupReportStore(paths)
    shutdown_report_store = CcbdShutdownReportStore(paths)
    namespace_state_store = ProjectNamespaceStateStore(paths)
    project_view_state_store = ProjectViewStateStore(paths, project_id=project_id)
    start_policy_store = CcbdStartPolicyStore(paths)
    ownership_guard = OwnershipGuard(paths, mount_manager, clock=lambda: NOW)
    project_namespace = ProjectNamespaceController(paths, project_id, clock=lambda: NOW)
    execution_service = ExecutionService(
        build_default_execution_registry(),
        clock=lambda: NOW,
        state_store=ExecutionStateStore(paths),
        fault_injection=FaultInjectionService(paths, clock=lambda: NOW),
    )
    snapshot_writer = SnapshotWriter(paths, clock=lambda: NOW)
    metrics = ControlPlaneMetrics()

    def _deps(version: int) -> CcbdServiceGraphDependencies:
        return CcbdServiceGraphDependencies(
            project_root=project_root,
            project_id=project_id,
            paths=paths,
            config=config,
            provider_catalog=provider_catalog,
            mount_manager=mount_manager,
            lifecycle_store=lifecycle_store,
            restore_store=restore_store,
            namespace_state_store=namespace_state_store,
            project_view_state_store=project_view_state_store,
            project_namespace=project_namespace,
            ownership_guard=ownership_guard,
            startup_report_store=startup_report_store,
            shutdown_report_store=shutdown_report_store,
            start_policy_store=start_policy_store,
            execution_service=execution_service,
            snapshot_writer=snapshot_writer,
            control_plane_metrics=metrics,
            clock=lambda: NOW,
            request_timeout_s=0.0,
            daemon_generation_getter=lambda: None,
            mount_missing_runtime_fn=lambda _agent_name: False,
            supervision_suspended_fn=lambda: False,
            version=version,
        )

    graph1 = build_ccbd_service_graph(_deps(1))
    graph2 = build_ccbd_service_graph(_deps(2))

    assert graph1 is not graph2
    assert graph1.version == 1
    assert graph2.version == 2
    assert graph1.created_at == NOW
    assert graph2.created_at == NOW
    assert graph1.config_identity == graph2.config_identity
    assert graph1.registry is not graph2.registry
    assert graph1.runtime_service is not graph2.runtime_service
    assert graph1.runtime_supervisor._mount_manager is mount_manager
    assert graph1.runtime_supervisor._ownership_guard is ownership_guard
    assert graph1.runtime_supervisor._startup_report_store is startup_report_store
    assert graph1.dispatcher._execution_service is execution_service
    assert graph1.dispatcher._snapshot_writer is snapshot_writer
    assert graph1.project_view_service._deps.mount_manager is mount_manager
    assert graph1.project_view_service._deps.namespace_controller is project_namespace
    assert graph1.project_focus_service._deps.project_view_service is graph1.project_view_service
    assert graph1.health_monitor._ownership_guard is ownership_guard
    assert graph1.ping_payload_services.config is graph1.config
    assert graph1.ping_payload_services.registry is graph1.registry
    assert graph1.ping_payload_services.health_monitor is graph1.health_monitor
    assert not paths.agent_runtime_path('alpha').exists()
    assert not paths.agent_runtime_path('beta').exists()


def test_ccbd_app_bootstrap_publishes_startup_graph_fields(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-app-bootstrap')

    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)

    assert app.service_graph.version == 1
    assert app.service_graph.created_at == NOW
    assert app.config is app.service_graph.config
    assert app.config_identity is app.service_graph.config_identity
    assert app.registry is app.service_graph.registry
    assert app.runtime_service is app.service_graph.runtime_service
    assert app.runtime_supervisor is app.service_graph.runtime_supervisor
    assert app.runtime_supervision is app.service_graph.runtime_supervision
    assert app.completion_tracker is app.service_graph.completion_tracker
    assert app.dispatcher is app.service_graph.dispatcher
    assert app.project_view_service is app.service_graph.project_view_service
    assert app.project_focus_service is app.service_graph.project_focus_service
    assert app.health_monitor is app.service_graph.health_monitor
    assert app.runtime_supervisor._mount_manager is app.mount_manager
    assert app.runtime_supervisor._ownership_guard is app.ownership_guard
    assert app.control_plane_metrics.service_graph_version == 1
    assert app.control_plane_metrics.service_graph_created_at == NOW
    assert app.control_plane_metrics.service_graph_retained_count == 1
    assert app.control_plane_metrics.service_graph_retained_count_scope == SERVICE_GRAPH_RETAINED_COUNT_SCOPE
    assert {'submit', 'project_view', 'project_focus_agent', 'ping'} <= set(app.socket_server._handlers)


def test_handlers_route_graph_bound_services_through_current_graph(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-handler-routing')
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    first_graph = app.service_graph
    replacement = _routing_graph(first_graph, version=2)

    app.publish_service_graph(replacement)

    submit = app.socket_server._handlers['submit'](
        {
            'project_id': app.project_id,
            'to_agent': 'alpha',
            'from_actor': 'user',
            'body': 'hello',
            'delivery_scope': 'single',
        }
    )
    project_view = app.socket_server._handlers['project_view']({'schema_version': 1})
    focus_agent = app.socket_server._handlers['project_focus_agent']({'agent': 'alpha'})
    focus_window = app.socket_server._handlers['project_focus_window']({'window': 'main'})
    queue = app.socket_server._handlers['queue']({'target': 'all'})
    ping = app.socket_server._handlers['ping']({'target': 'ccbd'})

    assert submit['job_id'] == 'job_graph_2'
    assert project_view == {'graph_version': 2, 'schema_version': 1}
    assert focus_agent == {'focused': 'agent', 'graph_version': 2, 'agent': 'alpha'}
    assert focus_window == {'focused': 'window', 'graph_version': 2, 'window': 'main'}
    assert queue == {'target': 'all', 'graph_version': 2}
    assert ping['diagnostics']['service_graph_version'] == 2
    assert ping['known_agents'] == ['alpha', 'beta', 'gamma']
    assert replacement.dispatcher.calls == [('submit', 'hello'), ('queue', 'all', None)]
    assert replacement.project_view_service.calls == [('project_view', 1)]
    assert replacement.project_focus_service.calls == [('focus_agent', 'alpha', None), ('focus_window', 'main', None)]
    assert replacement.registry.calls == []
    assert replacement.health_monitor.calls == ['local_daemon_health']


def test_handler_wrappers_do_not_reload_config_or_rebuild_graph_per_request(tmp_path: Path, monkeypatch) -> None:
    project_root = _project(tmp_path / 'repo-handler-routing-no-reload')
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    first_graph = app.service_graph
    replacement = _routing_graph(first_graph, version=3)
    build_calls = 0

    def _unexpected_load(*_args, **_kwargs):
        raise AssertionError('handler wrapper must not load .ccb/ccb.config')

    def _unexpected_build(*_args, **_kwargs):
        nonlocal build_calls
        build_calls += 1
        raise AssertionError('handler wrapper must not rebuild service graph')

    monkeypatch.setattr('ccbd.app_runtime.handlers.load_project_config', _unexpected_load, raising=False)
    monkeypatch.setattr('ccbd.app_runtime.handlers.build_ccbd_service_graph', _unexpected_build, raising=False)

    app.publish_service_graph(replacement)

    assert app.socket_server._handlers['project_view']({}) == {'graph_version': 3, 'schema_version': 1}
    assert app.socket_server._handlers['queue']({'target': 'all'}) == {'target': 'all', 'graph_version': 3}
    assert build_calls == 0


def test_ping_all_and_agent_routes_through_current_graph(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-handler-routing-ping-all')
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    replacement = _routing_graph(app.service_graph, version=4)

    app.publish_service_graph(replacement)

    all_payload = app.socket_server._handlers['ping']({'target': 'all'})
    agent_payload = app.socket_server._handlers['ping']({'target': 'gamma'})

    assert [item['agent_name'] for item in all_payload['agents']] == ['alpha', 'beta', 'gamma']
    assert agent_payload['agent_name'] == 'gamma'
    assert replacement.registry.calls == [
        ('spec_for', 'alpha'),
        ('get', 'alpha'),
        ('spec_for', 'beta'),
        ('get', 'beta'),
        ('spec_for', 'gamma'),
        ('get', 'gamma'),
        ('spec_for', 'gamma'),
        ('get', 'gamma'),
    ]
    assert replacement.health_monitor.calls == ['local_daemon_health', 'local_daemon_health']


def test_after_response_action_keeps_request_graph(tmp_path: Path, monkeypatch) -> None:
    project_root = _project(tmp_path / 'repo-handler-routing-after-response')
    app = CcbdApp(project_root, clock=lambda: NOW, pid=4242)
    graph2 = _routing_graph(app.service_graph, version=2)
    graph3 = _routing_graph(app.service_graph, version=3)
    restarted: list[tuple[str, tuple[str, ...]]] = []

    def _fake_restart(proxy, *, agent_names: tuple[str, ...]):
        restarted.append((proxy.config_identity['config_signature'], agent_names))
        return ()

    monkeypatch.setattr('ccbd.handlers.project_restart.restart_project_agent_panes_in_place', _fake_restart)

    app.publish_service_graph(graph2)
    payload, after_response = app.socket_server._handlers['project_restart_panes']({})
    app.publish_service_graph(graph3)
    after_response()

    assert payload['agent_names'] == ['alpha', 'beta', 'gamma']
    assert restarted == [('sig-2', ('alpha', 'beta', 'gamma'))]


def _project(project_root: Path) -> Path:
    config_dir = project_root / '.ccb'
    config_dir.mkdir(parents=True, exist_ok=True)
    (config_dir / 'ccb.config').write_text('alpha:codex, beta:claude\n', encoding='utf-8')
    return project_root


def _routing_graph(base, *, version: int):
    config = _RoutingConfig(('alpha', 'beta', 'gamma'))
    registry = _RoutingRegistry(('alpha', 'beta', 'gamma'))
    health_monitor = _RoutingHealthMonitor(version)
    return SimpleNamespace(
        version=version,
        created_at=f'2026-05-29T00:00:0{version}Z',
        config=config,
        config_identity={
            'known_agents': ('alpha', 'beta', 'gamma'),
            'config_signature': f'sig-{version}',
        },
        registry=registry,
        runtime_service=base.runtime_service,
        runtime_supervisor=base.runtime_supervisor,
        runtime_supervision=base.runtime_supervision,
        completion_tracker=base.completion_tracker,
        dispatcher=_RoutingDispatcher(version),
        project_view_service=_RoutingProjectView(version),
        project_focus_service=_RoutingProjectFocus(version),
        health_monitor=health_monitor,
        ping_payload_services=SimpleNamespace(
            config=config,
            registry=registry,
            health_monitor=health_monitor,
        ),
    )


class _RoutingConfig:
    def __init__(self, agents: tuple[str, ...]) -> None:
        self.agents = {agent: SimpleNamespace(name=agent, provider='codex') for agent in agents}

    def to_record(self) -> dict[str, object]:
        return {
            'version': 2,
            'default_agents': list(self.agents),
            'agents': {name: {'name': name, 'provider': 'codex'} for name in self.agents},
        }


class _RoutingRegistry:
    def __init__(self, agents: tuple[str, ...]) -> None:
        self._agents = agents
        self.calls: list[tuple[str, str]] = []

    def list_known_agents(self):
        return self._agents

    def spec_for(self, agent_name: str):
        self.calls.append(('spec_for', agent_name))
        return SimpleNamespace(name=agent_name, provider='codex')

    def get(self, agent_name: str):
        self.calls.append(('get', agent_name))
        return None


class _RoutingDispatcher:
    def __init__(self, version: int) -> None:
        self.version = version
        self.calls: list[tuple] = []
        self._timing_sink = ControlPlaneMetrics()

    def submit(self, envelope):
        self.calls.append(('submit', envelope.body))
        return SubmitReceipt(
            accepted_at=NOW,
            jobs=(
                AcceptedJobReceipt(
                    job_id=f'job_graph_{self.version}',
                    agent_name=envelope.to_agent,
                    target_kind=TargetKind.AGENT,
                    target_name=envelope.to_agent,
                    status=JobStatus.ACCEPTED,
                    accepted_at=NOW,
                ),
            ),
        )

    def queue(self, target: str = 'all', *, detail: bool | None = None) -> dict:
        self.calls.append(('queue', target, detail))
        return {'target': target, 'graph_version': self.version}


class _RoutingProjectView:
    def __init__(self, version: int) -> None:
        self.version = version
        self.calls: list[tuple[str, int]] = []

    def build_response(self, *, schema_version: int = 1) -> dict:
        self.calls.append(('project_view', schema_version))
        return {'graph_version': self.version, 'schema_version': schema_version}


class _RoutingProjectFocus:
    def __init__(self, version: int) -> None:
        self.version = version
        self.calls: list[tuple] = []

    def focus_agent(self, *, agent: str, namespace_epoch: int | None = None) -> dict:
        self.calls.append(('focus_agent', agent, namespace_epoch))
        return {'focused': 'agent', 'graph_version': self.version, 'agent': agent}

    def focus_window(self, *, window: str, namespace_epoch: int | None = None) -> dict:
        self.calls.append(('focus_window', window, namespace_epoch))
        return {'focused': 'window', 'graph_version': self.version, 'window': window}


class _RoutingHealthMonitor:
    def __init__(self, version: int) -> None:
        self.version = version
        self.calls: list[str] = []

    def local_daemon_health(self):
        self.calls.append('local_daemon_health')
        return LeaseInspection(
            lease=None,
            health=LeaseHealth.HEALTHY,
            pid_alive=True,
            socket_connectable=True,
            heartbeat_fresh=True,
            takeover_allowed=False,
            reason=f'graph-{self.version}',
        )

    def daemon_health(self):
        self.calls.append('daemon_health')
        return self.local_daemon_health()
