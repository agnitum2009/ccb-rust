from __future__ import annotations

from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace

from agents.models import (
    AgentRuntime,
    AgentSpec,
    AgentState,
    PermissionMode,
    ProjectConfig,
    QueuePolicy,
    RestoreMode,
    RuntimeMode,
    WindowSpec,
    ToolWindowSpec,
    WorkspaceMode,
)
from ccbd.api_models import DeliveryScope, JobRecord, JobStatus, MessageEnvelope, TargetKind
from ccbd.models import MountState
from ccbd.project_view import (
    AgentActivityFacts,
    ProjectViewDependencies,
    ProjectViewSequenceCache,
    ProjectViewService,
    ProjectViewStateStore,
    resolve_agent_activity,
)
import ccbd.project_view.service as project_view_service
from ccbd.services.dispatcher import JobDispatcher
from ccbd.services.mount import MountManager
from ccbd.services.project_namespace import ProjectNamespaceController
from ccbd.services.project_namespace_state import ProjectNamespaceState, ProjectNamespaceStateStore
from ccbd.services.registry import AgentRegistry
from completion.models import CompletionConfidence, CompletionDecision, CompletionStatus
from message_bureau import CallbackEdgeStore, CallbackEdgeState
from message_bureau.models import AttemptRecord, AttemptState, ReplyRecord, ReplyTerminalStatus
from project.ids import compute_project_id
from provider_hooks.activity import load_activity, write_activity
from storage.paths import PathLayout


NOW = '2026-05-20T12:00:00Z'


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding='utf-8')


def _spec(name: str, provider: str) -> AgentSpec:
    return AgentSpec(
        name=name,
        provider=provider,
        target='.',
        workspace_mode=WorkspaceMode.INPLACE,
        workspace_root=None,
        runtime_mode=RuntimeMode.PANE_BACKED,
        restore_default=RestoreMode.AUTO,
        permission_default=PermissionMode.MANUAL,
        queue_policy=QueuePolicy.SERIAL_PER_AGENT,
    )


def _runtime(agent_name: str, *, project_id: str, state: AgentState = AgentState.IDLE, health: str = 'healthy') -> AgentRuntime:
    return AgentRuntime(
        agent_name=agent_name,
        state=state,
        pid=100,
        started_at=NOW,
        last_seen_at=NOW,
        runtime_ref=f'tmux:%{agent_name}',
        session_ref=f'{agent_name}-session',
        workspace_path='/tmp/workspace',
        project_id=project_id,
        backend_type='tmux',
        queue_depth=0,
        socket_path=None,
        health=health,
        provider=None,
        pane_id=f'%{agent_name[-1]}',
        pane_state='alive',
        reconcile_state='steady',
    )


def _config() -> ProjectConfig:
    agents = {
        'agent1': _spec('agent1', 'codex'),
        'agent2': _spec('agent2', 'claude'),
        'agent3': _spec('agent3', 'codex'),
    }
    return ProjectConfig(
        version=2,
        default_agents=('agent1', 'agent2', 'agent3'),
        agents=agents,
        cmd_enabled=False,
        layout_spec='agent1:codex, agent2:claude',
        windows=(
            WindowSpec(name='main', order=0, layout_spec='agent1:codex, agent2:claude', agent_names=('agent1', 'agent2')),
            WindowSpec(name='ops', order=1, layout_spec='agent3:codex', agent_names=('agent3',)),
        ),
        entry_window='main',
    )


def _config_with_tool_window() -> ProjectConfig:
    base = _config()
    return ProjectConfig(
        version=2,
        default_agents=base.default_agents,
        agents=base.agents,
        cmd_enabled=False,
        layout_spec=base.layout_spec,
        windows=base.windows,
        tool_windows=(ToolWindowSpec(name='neovim', order=0, command='ccb-nvim'),),
        entry_window='main',
    )


def _message(project_id: str, *, sender: str, target: str) -> MessageEnvelope:
    return MessageEnvelope(
        project_id=project_id,
        to_agent=target,
        from_actor=sender,
        body='work',
        task_id=None,
        reply_to='agent1' if sender != 'user' else None,
        message_type='ask',
        delivery_scope=DeliveryScope.SINGLE,
    )


def _reply_delivery_message(
    project_id: str,
    *,
    source_agent: str,
    target: str,
    source_job_id: str,
    reply_id: str = 'reply_1',
    body: str | None = None,
) -> MessageEnvelope:
    return MessageEnvelope(
        project_id=project_id,
        to_agent=target,
        from_actor='system',
        body=body if body is not None else f'CCB_REPLY from={source_agent} reply={reply_id} status=completed job={source_job_id}\n\nOK',
        task_id=f'reply:{reply_id}',
        reply_to=None,
        message_type='reply_delivery',
        delivery_scope=DeliveryScope.SINGLE,
    )


def _submit(dispatcher: JobDispatcher, project_id: str, *, sender: str, target: str, body: str = 'work') -> str:
    receipt = dispatcher.submit(
        MessageEnvelope(
            project_id=project_id,
            to_agent=target,
            from_actor=sender,
            body=body,
            task_id=None,
            reply_to='agent1' if sender != 'user' else None,
            message_type='ask',
            delivery_scope=DeliveryScope.SINGLE,
        )
    )
    return receipt.jobs[0].job_id


def _job(
    project_id: str,
    *,
    job_id: str,
    sender: str,
    target: str,
    status: JobStatus,
    updated_at: str = NOW,
    terminal_reason: str | None = None,
    body: str = 'work',
    silence_on_success: bool = False,
) -> JobRecord:
    terminal_decision = None
    if status in {JobStatus.FAILED, JobStatus.CANCELLED, JobStatus.INCOMPLETE}:
        terminal_decision = {'reason': terminal_reason or status.value}
    elif status is JobStatus.COMPLETED:
        terminal_decision = {'reason': terminal_reason or 'task_complete'}
    return JobRecord(
        job_id=job_id,
        submission_id=None,
        agent_name=target,
        provider='codex',
        request=replace(
            _message(project_id, sender=sender, target=target),
            body=body,
            silence_on_success=silence_on_success,
        ),
        status=status,
        terminal_decision=terminal_decision,
        cancel_requested_at=None,
        created_at='2026-05-20T11:59:00Z',
        updated_at=updated_at,
        target_kind=TargetKind.AGENT,
        target_name=target,
    )


def _reply_delivery_job(
    project_id: str,
    *,
    job_id: str,
    source_agent: str,
    source_job_id: str,
    target: str,
    status: JobStatus,
    updated_at: str = NOW,
    reply_id: str = 'reply_1',
    body: str | None = None,
) -> JobRecord:
    terminal_decision = None
    if status in {JobStatus.FAILED, JobStatus.CANCELLED, JobStatus.INCOMPLETE}:
        terminal_decision = {'reason': status.value}
    elif status is JobStatus.COMPLETED:
        terminal_decision = {'reason': 'task_complete'}
    return JobRecord(
        job_id=job_id,
        submission_id=None,
        agent_name=target,
        provider='codex',
        request=_reply_delivery_message(
            project_id,
            source_agent=source_agent,
            target=target,
            source_job_id=source_job_id,
            reply_id=reply_id,
            body=body,
        ),
        status=status,
        terminal_decision=terminal_decision,
        cancel_requested_at=None,
        created_at='2026-05-20T12:00:01Z',
        updated_at=updated_at,
        target_kind=TargetKind.AGENT,
        target_name=target,
        provider_options={'reply_delivery': True, 'reply_delivery_reply_id': reply_id},
    )


def _record_reply_for_source(dispatcher: JobDispatcher, source: JobRecord, *, reply_id: str) -> None:
    attempt_id = f'att_{source.job_id}'
    message_id = f'msg_{source.job_id}'
    dispatcher._message_bureau_control._attempt_store.append(
        AttemptRecord(
            attempt_id=attempt_id,
            message_id=message_id,
            agent_name=source.agent_name,
            provider=source.provider,
            job_id=source.job_id,
            retry_index=0,
            health_snapshot_ref=None,
            started_at=source.created_at,
            updated_at=source.updated_at,
            attempt_state=AttemptState.COMPLETED,
        )
    )
    dispatcher._message_bureau_control._reply_store.append(
        ReplyRecord(
            reply_id=reply_id,
            message_id=message_id,
            attempt_id=attempt_id,
            agent_name=source.agent_name,
            terminal_status=ReplyTerminalStatus.COMPLETED,
            reply='OK',
            diagnostics={},
            finished_at=source.updated_at,
        )
    )


def _decision(*, reply: str = 'done', status: CompletionStatus = CompletionStatus.COMPLETED) -> CompletionDecision:
    return CompletionDecision(
        terminal=True,
        status=status,
        reason='task_complete' if status is CompletionStatus.COMPLETED else status.value,
        confidence=CompletionConfidence.EXACT,
        reply=reply,
        anchor_seen=True,
        reply_started=True,
        reply_stable=True,
        provider_turn_ref='turn-1',
        source_cursor=None,
        finished_at=NOW,
        diagnostics={},
    )


def _project_view_service(
    *,
    project_root: Path,
    project_id: str,
    layout: PathLayout,
    config: ProjectConfig,
    registry: AgentRegistry,
    dispatcher: JobDispatcher,
) -> ProjectViewService:
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(
        project_id=project_id,
        pid=123,
        socket_path=layout.ccbd_socket_path,
        generation=7,
        started_at=NOW,
    )
    return ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            paths=layout,
            clock=lambda: NOW,
        )
    )


def test_project_view_uses_provider_activity_without_ccb_job(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-activity'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    runtime = _runtime('agent1', project_id=project_id)
    runtime.session_id = 'codex-session-1'
    registry.upsert(runtime)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    write_activity(
        provider='codex',
        project_id=project_id,
        agent_name='agent1',
        runtime_dir=layout.agent_provider_runtime_dir('agent1', 'codex'),
        state='active',
        source='codex_hook',
        event_name='UserPromptSubmit',
        ccb_session_id='ccb-agent1-launch',
        provider_session_id='codex-session-1',
        pane_id='%1',
        workspace_path='/tmp/workspace',
        updated_at=NOW,
    )

    response = _project_view_service(
        project_root=project_root,
        project_id=project_id,
        layout=layout,
        config=config,
        registry=registry,
        dispatcher=dispatcher,
    ).build_response()

    agent = response['view']['agents'][0]
    assert agent['activity_state'] == 'active'
    assert agent['activity_source'] == 'codex_hook'
    assert agent['activity_reason'] == 'provider_userpromptsubmit'
    assert agent['last_progress_at'] == NOW


def test_project_view_provider_failed_overrides_running_job_metadata(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-failed'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    runtime = _runtime('agent1', project_id=project_id)
    runtime.session_id = 'ccb-agent1-session'
    registry.upsert(runtime)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    running = _job(project_id, job_id='job_running_1234', sender='user', target='agent1', status=JobStatus.RUNNING)
    dispatcher._append_job(running)
    dispatcher._state.mark_active_for(TargetKind.AGENT, 'agent1', running.job_id)
    write_activity(
        provider='codex',
        project_id=project_id,
        agent_name='agent1',
        runtime_dir=layout.agent_provider_runtime_dir('agent1', 'codex'),
        state='failed',
        source='codex_hook',
        event_name='Stop',
        ccb_session_id='ccb-agent1-session',
        pane_id='%1',
        workspace_path='/tmp/workspace',
        diagnostics={'reason': 'api_error'},
        updated_at=NOW,
    )

    response = _project_view_service(
        project_root=project_root,
        project_id=project_id,
        layout=layout,
        config=config,
        registry=registry,
        dispatcher=dispatcher,
    ).build_response()

    agent = response['view']['agents'][0]
    assert agent['activity_state'] == 'failed'
    assert agent['activity_source'] == 'codex_hook'
    assert agent['activity_reason'] == 'api_error'
    assert agent['current_job_id'] == 'job_running_1234'


def test_project_view_ignores_provider_activity_for_wrong_pane(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-wrong-pane'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    runtime = _runtime('agent1', project_id=project_id)
    runtime.session_id = 'ccb-agent1-session'
    registry.upsert(runtime)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    write_activity(
        provider='codex',
        project_id=project_id,
        agent_name='agent1',
        runtime_dir=layout.agent_provider_runtime_dir('agent1', 'codex'),
        state='active',
        source='codex_hook',
        ccb_session_id='ccb-agent1-session',
        pane_id='%99',
        workspace_path='/tmp/workspace',
        updated_at=NOW,
    )

    response = _project_view_service(
        project_root=project_root,
        project_id=project_id,
        layout=layout,
        config=config,
        registry=registry,
        dispatcher=dispatcher,
    ).build_response()

    agent = response['view']['agents'][0]
    assert agent['activity_state'] == 'idle'
    assert agent['activity_source'] == 'pane_liveness'


def test_project_view_skips_capture_pane_when_provider_activity_is_fresh(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-no-capture'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    runtime = _runtime('agent1', project_id=project_id)
    runtime.session_id = 'ccb-agent1-session'
    registry.upsert(runtime)
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-provider-no-capture',
            layout_version=2,
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    write_activity(
        provider='codex',
        project_id=project_id,
        agent_name='agent1',
        runtime_dir=layout.agent_provider_runtime_dir('agent1', 'codex'),
        state='active',
        source='codex_hook',
        ccb_session_id='ccb-agent1-session',
        pane_id='%1',
        workspace_path='/tmp/workspace',
        updated_at=NOW,
    )
    backend = _SnapshotBackend()
    controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: backend,
    )

    response = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            paths=layout,
            clock=lambda: NOW,
        )
    ).build_response()

    assert response['view']['agents'][0]['activity_source'] == 'codex_hook'
    assert [args for args in backend.calls if args[:3] == ['capture-pane', '-p', '-t']] == []


def test_project_view_marks_stale_provider_activity_failed_from_pane_error(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-pane-error'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    runtime = _runtime('agent1', project_id=project_id)
    runtime.session_id = 'ccb-agent1-session'
    registry.upsert(runtime)
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-provider-pane-error',
            layout_version=2,
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    write_activity(
        provider='codex',
        project_id=project_id,
        agent_name='agent1',
        runtime_dir=layout.agent_provider_runtime_dir('agent1', 'codex'),
        state='active',
        source='codex_hook',
        ccb_session_id='ccb-agent1-session',
        pane_id='%1',
        workspace_path='/tmp/workspace',
        updated_at='2026-05-20T11:59:50Z',
    )

    class ErrorPaneBackend(_SnapshotBackend):
        def _tmux_run(self, args: list[str], *, capture=False, check=False, timeout=None):
            self.calls.append(list(args))
            if args[:3] == ['capture-pane', '-p', '-t']:
                return type(
                    'CP',
                    (),
                    {
                        'returncode': 0,
                        'stdout': (
                            'ERROR: Reconnecting... 5/5\n'
                            'ERROR: stream disconnected before completion: error sending request for url '
                            '(http://127.0.0.1:9/responses)\n'
                        ),
                        'stderr': '',
                    },
                )()
            return self._snapshot_run(args, capture=capture, check=check, timeout=timeout)

    backend = ErrorPaneBackend()
    controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: backend,
    )

    response = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            paths=layout,
            clock=lambda: NOW,
        )
    ).build_response()

    agent = response['view']['agents'][0]
    assert agent['activity_state'] == 'failed'
    assert agent['activity_source'] == 'provider_pane'
    assert agent['activity_reason'] == 'provider_terminal_error'
    assert [args for args in backend.calls if args[:3] == ['capture-pane', '-p', '-t']] == [
        ['capture-pane', '-p', '-t', '%1', '-S', '-30']
    ]


def test_project_view_marks_stale_provider_activity_failed_from_http_status_error(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-http-error'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    runtime = _runtime('agent1', project_id=project_id)
    runtime.session_id = 'codex-session-1'
    registry.upsert(runtime)
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-provider-http-error',
            layout_version=2,
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    write_activity(
        provider='codex',
        project_id=project_id,
        agent_name='agent1',
        runtime_dir=layout.agent_provider_runtime_dir('agent1', 'codex'),
        state='active',
        source='codex_hook',
        ccb_session_id='ccb-agent1-launch',
        provider_session_id='codex-session-1',
        pane_id='%1',
        workspace_path='/tmp/workspace',
        updated_at='2026-05-20T11:59:50Z',
    )

    class HttpErrorPaneBackend(_SnapshotBackend):
        def _tmux_run(self, args: list[str], *, capture=False, check=False, timeout=None):
            self.calls.append(list(args))
            if args[:3] == ['capture-pane', '-p', '-t']:
                return type(
                    'CP',
                    (),
                    {
                        'returncode': 0,
                        'stdout': (
                            '› trigger CCB_HTTP_429_SMOKE and stop\n'
                            '■ exceeded retry limit, last status: 429 Too Many Requests\n'
                            '› Find and fix a bug in @filename\n'
                        ),
                        'stderr': '',
                    },
                )()
            return self._snapshot_run(args, capture=capture, check=check, timeout=timeout)

    backend = HttpErrorPaneBackend()
    controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: backend,
    )

    response = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            paths=layout,
            clock=lambda: NOW,
        )
    ).build_response()

    agent = response['view']['agents'][0]
    assert agent['activity_state'] == 'failed'
    assert agent['activity_source'] == 'provider_pane'
    assert agent['activity_reason'] == 'provider_terminal_error'

    activity_payload = load_activity(layout.agent_provider_runtime_dir('agent1', 'codex'))
    assert activity_payload is not None
    assert activity_payload['state'] == 'failed'
    assert activity_payload['source'] == 'provider_pane'
    assert activity_payload['diagnostics']['reason'] == 'provider_terminal_error'

    clean_backend = _SnapshotBackend()
    clean_controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: clean_backend,
    )
    second_response = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=clean_controller,
            paths=layout,
            clock=lambda: NOW,
            cache_ttl_ms=0,
        )
    ).build_response()

    second_agent = second_response['view']['agents'][0]
    assert second_agent['activity_state'] == 'failed'
    assert second_agent['activity_source'] == 'provider_pane'
    assert second_agent['activity_reason'] == 'provider_terminal_error'
    assert [args for args in clean_backend.calls if args[:3] == ['capture-pane', '-p', '-t']] == []


def test_project_view_marks_stale_codex_activity_idle_from_prompt(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-pane-idle'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    runtime = _runtime('agent1', project_id=project_id)
    runtime.session_id = 'codex-session-1'
    registry.upsert(runtime)
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-provider-pane-idle',
            layout_version=2,
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    write_activity(
        provider='codex',
        project_id=project_id,
        agent_name='agent1',
        runtime_dir=layout.agent_provider_runtime_dir('agent1', 'codex'),
        state='active',
        source='codex_hook',
        ccb_session_id='ccb-agent1-launch',
        provider_session_id='codex-session-1',
        pane_id='%1',
        workspace_path='/tmp/workspace',
        updated_at='2026-05-20T11:59:50Z',
    )

    class IdlePromptBackend(_SnapshotBackend):
        def _tmux_run(self, args: list[str], *, capture=False, check=False, timeout=None):
            self.calls.append(list(args))
            if args[:3] == ['capture-pane', '-p', '-t']:
                return type(
                    'CP',
                    (),
                    {
                        'returncode': 0,
                        'stdout': (
                            '› print exactly CCB_OK and stop\n\n'
                            '• CCB_OK\n\n'
                            '› Use /skills to list available skills\n'
                            '  gpt-5.5 xhigh · ~/repo\n'
                        ),
                        'stderr': '',
                    },
                )()
            return self._snapshot_run(args, capture=capture, check=check, timeout=timeout)

    backend = IdlePromptBackend()
    controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: backend,
    )

    response = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            paths=layout,
            clock=lambda: NOW,
        )
    ).build_response()

    agent = response['view']['agents'][0]
    assert agent['activity_state'] == 'idle'
    assert agent['activity_source'] == 'provider_pane'
    assert agent['activity_reason'] == 'provider_prompt_idle'


def test_project_view_returns_minimal_windows_agents_and_comms(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(
        project_id=project_id,
        pid=123,
        socket_path=layout.ccbd_socket_path,
        generation=7,
        started_at=NOW,
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    running = _job(project_id, job_id='job_running_1234', sender='agent2', target='agent1', status=JobStatus.RUNNING)
    dispatcher._append_job(running)
    dispatcher._state.mark_active_for(TargetKind.AGENT, 'agent1', running.job_id)
    queued = _job(project_id, job_id='job_queued_5678', sender='user', target='agent3', status=JobStatus.QUEUED)
    dispatcher._append_job(queued)
    dispatcher._state.enqueue_for(TargetKind.AGENT, 'agent3', queued.job_id)

    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    response = service.build_response()
    view = response['view']

    assert response['cache']['ttl_ms'] == 1000
    assert response['cache']['sequence'] == 1
    assert view['project']['display_name'] == 'repo'
    assert view['ccbd']['state'] == MountState.MOUNTED.value
    assert view['namespace']['sidebar']['view']['agents_height'] == '50%'
    assert view['namespace']['sidebar']['view']['comms_height'] == '15%'
    assert view['namespace']['sidebar']['view']['tips_height'] == '35%'
    assert view['namespace']['sidebar']['view']['comms_limit'] == 5
    assert view['namespace']['sidebar']['view']['tips'][0] == 'C-b d  detach'
    assert 'C-b h/j/k/l pane' in view['namespace']['sidebar']['view']['tips']
    assert 'copy: y yank' in view['namespace']['sidebar']['view']['tips']
    assert [window['name'] for window in view['windows']] == ['main', 'ops']
    assert view['windows'][0]['agents'] == ['agent1', 'agent2']
    assert [agent['name'] for agent in view['agents']] == ['agent1', 'agent2', 'agent3']
    assert view['agents'][0]['activity_state'] == 'active'
    assert view['agents'][0]['activity_source'] == 'ccb_job'
    assert view['agents'][0]['activity_reason'] == 'job_running'
    assert view['agents'][0]['current_job_id'] == 'job_running_1234'
    assert view['agents'][0]['queue_depth'] == 1
    assert view['agents'][2]['activity_state'] == 'pending'
    assert view['agents'][2]['activity_source'] == 'ccb_job'
    assert view['agents'][2]['activity_reason'] == 'job_queued'
    assert view['agents'][2]['current_job_id'] == 'job_queued_5678'
    assert view['agents'][2]['queue_depth'] == 1
    assert [item['id'] for item in view['comms']] == ['job_running_1234', 'job_queued_5678']
    assert view['comms'][0]['sender'] == 'agent2'
    assert view['comms'][0]['target'] == 'agent1'


def test_project_view_includes_provider_runtime_for_active_execution(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-runtime'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(
        project_id=project_id,
        pid=123,
        socket_path=layout.ccbd_socket_path,
        generation=7,
        started_at=NOW,
    )
    execution_service = SimpleNamespace(
        active_runtime_snapshots=lambda: (
            {
                'job_id': 'job_running_1234',
                'agent_name': 'agent1',
                'provider': 'codex',
                'source_kind': 'protocol_event_stream',
                'primary_authority': 'protocol_log',
                'runtime_state': {
                    'delivery_state': 'pending_anchor',
                    'anchor_seen': False,
                    'delivery_started_at': '2026-05-20T11:59:40Z',
                    'delivery_timeout_s': 120.0,
                },
            },
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, execution_service=execution_service, clock=lambda: NOW)
    running = _job(project_id, job_id='job_running_1234', sender='agent2', target='agent1', status=JobStatus.RUNNING)
    dispatcher._append_job(running)
    dispatcher._state.mark_active_for(TargetKind.AGENT, 'agent1', running.job_id)

    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    agent1 = service.build_response()['view']['agents'][0]

    assert agent1['current_job_id'] == 'job_running_1234'
    assert agent1['provider_runtime']['job_id'] == 'job_running_1234'
    assert agent1['provider_runtime']['primary_authority'] == 'protocol_log'
    assert agent1['provider_runtime']['runtime_state']['delivery_state'] == 'pending_anchor'


def test_project_view_does_not_attach_mismatched_provider_runtime_to_current_job(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-runtime-mismatch'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(
        project_id=project_id,
        pid=123,
        socket_path=layout.ccbd_socket_path,
        generation=7,
        started_at=NOW,
    )
    execution_service = SimpleNamespace(
        active_runtime_snapshots=lambda: (
            {
                'job_id': 'job_other_9999',
                'agent_name': 'agent1',
                'provider': 'codex',
                'runtime_state': {'delivery_state': 'accepted'},
            },
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, execution_service=execution_service, clock=lambda: NOW)
    running = _job(project_id, job_id='job_running_1234', sender='agent2', target='agent1', status=JobStatus.RUNNING)
    dispatcher._append_job(running)
    dispatcher._state.mark_active_for(TargetKind.AGENT, 'agent1', running.job_id)

    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    agent1 = service.build_response()['view']['agents'][0]

    assert agent1['current_job_id'] == 'job_running_1234'
    assert 'provider_runtime' not in agent1


def test_project_view_marks_multiple_orphan_provider_runtimes_as_conflict(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-runtime-conflict'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(
        project_id=project_id,
        pid=123,
        socket_path=layout.ccbd_socket_path,
        generation=7,
        started_at=NOW,
    )
    execution_service = SimpleNamespace(
        active_runtime_snapshots=lambda: (
            {'job_id': 'job_orphan_a', 'agent_name': 'agent1', 'provider': 'codex'},
            {'job_id': 'job_orphan_b', 'agent_name': 'agent1', 'provider': 'codex'},
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, execution_service=execution_service, clock=lambda: NOW)

    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    agent1 = service.build_response()['view']['agents'][0]

    assert agent1['provider_runtime']['conflict'] == 'multiple_provider_runtimes_without_control_job'
    assert agent1['provider_runtime']['runtime_count'] == 2
    assert agent1['provider_runtime']['job_ids'] == ['job_orphan_a', 'job_orphan_b']


def test_project_view_includes_tool_window_without_agent_row(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-tool-view'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config_with_tool_window()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=JobDispatcher(layout, config, registry, clock=lambda: NOW),
            clock=lambda: NOW,
        )
    )

    view = service.build_response()['view']

    assert [window['name'] for window in view['windows']] == ['main', 'ops', 'neovim']
    tool = view['windows'][2]
    assert tool['kind'] == 'tool'
    assert tool['label'] == 'neovim'
    assert tool['show_in_sidebar'] is True
    assert tool['agents'] == []
    assert [agent['name'] for agent in view['agents']] == ['agent1', 'agent2', 'agent3']


def test_project_view_hot_reloads_sidebar_view_config(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-sidebar-view'
    project_root.mkdir()
    config_path = project_root / '.ccb' / 'ccb.config'
    _write(
        config_path,
        """version = 2
entry_window = "main"

[windows]
main = "agent1:codex, agent2:claude"
ops = "agent3:codex"

[ui.sidebar.view]
agents_height = "50%"
comms_height = "15%"
tips_height = "35%"
comms_limit = 4
tips = ["C-b d detach"]
""",
    )
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    mount_manager = MountManager(layout, clock=lambda: NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
            cache_ttl_ms=0,
        )
    )

    first = service.build_response()['view']['namespace']['sidebar']['view']
    _write(
        config_path,
        """version = 2
entry_window = "main"

[windows]
main = "agent1:codex, agent2:claude"
ops = "agent3:codex"

[ui.sidebar.view]
agents_height = "60%"
comms_height = "10%"
tips_height = "30%"
comms_limit = 2
tips = ["C-b z zoom", "C-b c new win"]
""",
    )
    second = service.build_response()['view']['namespace']['sidebar']['view']

    assert first['agents_height'] == '50%'
    assert first['comms_height'] == '15%'
    assert first['tips_height'] == '35%'
    assert first['comms_limit'] == 4
    assert first['tips'] == ['C-b d detach']
    assert second['agents_height'] == '60%'
    assert second['comms_height'] == '10%'
    assert second['tips_height'] == '30%'
    assert second['comms_limit'] == 2
    assert second['tips'] == ['C-b z zoom', 'C-b c new win']


def test_project_view_reports_sidebar_view_config_error_without_losing_last_good_view(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-sidebar-view-error'
    project_root.mkdir()
    config_path = project_root / '.ccb' / 'ccb.config'
    _write(
        config_path,
        """version = 2
entry_window = "main"

[windows]
main = "agent1:codex, agent2:claude"
ops = "agent3:codex"

[ui.sidebar.view]
comms_limit = 2
tips = ["C-b z zoom"]
""",
    )
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    mount_manager = MountManager(layout, clock=lambda: NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
            cache_ttl_ms=0,
        )
    )

    _write(
        config_path,
        """version = 2
entry_window = "main"

[windows]
main = "agent1:codex, agent2:claude"
ops = "agent3:codex"

[ui.sidebar.view]
tips = ["missing comma" "next"]
""",
    )
    sidebar = service.build_response()['view']['namespace']['sidebar']

    assert sidebar['view']['comms_limit'] == config.sidebar_view.comms_limit
    assert sidebar['view']['tips'][0] == 'C-b d  detach'
    assert 'invalid TOML config' in sidebar['view_error']


def test_project_view_comms_includes_recent_terminal_jobs(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-comms-terminal'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    old_running = _job(
        project_id,
        job_id='job_running_old',
        sender='agent2',
        target='agent1',
        status=JobStatus.RUNNING,
        updated_at='2026-05-20T11:59:00Z',
    )
    dispatcher._append_job(old_running)
    dispatcher._state.mark_active_for(TargetKind.AGENT, 'agent1', old_running.job_id)
    completed = _job(
        project_id,
        job_id='job_done_recent',
        sender='agent1',
        target='agent2',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:05Z',
    )
    dispatcher._append_job(completed)
    failed_old = _job(
        project_id,
        job_id='job_failed_old',
        sender='user',
        target='agent3',
        status=JobStatus.FAILED,
        updated_at='2026-05-20T11:58:00Z',
    )
    dispatcher._append_job(failed_old)

    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    comms = service.build_response()['view']['comms']

    assert [item['id'] for item in comms[:3]] == ['job_done_recent', 'job_running_old', 'job_failed_old']
    assert comms[0]['status'] == 'completed'
    assert comms[0]['short_reason'] == 'task_complete'
    assert comms[2]['status'] == 'failed'


def test_project_view_filters_dismissed_comms_from_shared_state(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-comms-dismissed'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    dismissed = _job(
        project_id,
        job_id='job_dismissed',
        sender='agent1',
        target='agent2',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:05Z',
    )
    kept = _job(
        project_id,
        job_id='job_kept',
        sender='agent2',
        target='agent1',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:04Z',
    )
    dispatcher._append_job(dismissed)
    dispatcher._append_job(kept)
    state_store = ProjectViewStateStore(layout, project_id=project_id)
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            state_store=state_store,
            clock=lambda: NOW,
            cache_ttl_ms=0,
        )
    )

    before = service.build_response()['view']['comms']
    state_store.dismiss_comms('job_dismissed')
    after = service.build_response()['view']['comms']

    assert [item['id'] for item in before[:2]] == ['job_dismissed', 'job_kept']
    assert [item['id'] for item in after] == ['job_kept']


def test_project_view_terminal_comms_do_not_mark_agent_failed(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-terminal-agent-clean'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    registry.upsert(_runtime('agent3', project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    cancelled = _job(
        project_id,
        job_id='job_cancelled_recent',
        sender='agent1',
        target='agent3',
        status=JobStatus.CANCELLED,
        updated_at=NOW,
        body='cancelled ask',
    )
    dispatcher._append_job(cancelled)
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    view = service.build_response()['view']
    agent3 = next(agent for agent in view['agents'] if agent['name'] == 'agent3')

    assert agent3['activity_state'] == 'idle'
    assert agent3['activity_reason'] == 'pane_alive'
    assert 'current_job_id' not in agent3
    assert view['comms'][0]['id'] == cancelled.job_id
    assert view['comms'][0]['status'] == 'cancelled'


def test_project_view_marks_callback_parent_waiting_for_child(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-callback-waiting-agent'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)

    parent_job_id = dispatcher.submit(
        MessageEnvelope(
            project_id=project_id,
            to_agent='agent1',
            from_actor='user',
            body='root task',
            task_id='task-callback-wait',
            reply_to=None,
            message_type='ask',
            delivery_scope=DeliveryScope.SINGLE,
        )
    ).jobs[0].job_id
    dispatcher.tick()
    child_job_id = dispatcher.submit(
        MessageEnvelope(
            project_id=project_id,
            to_agent='agent2',
            from_actor='agent1',
            body='child task',
            task_id='task-callback-wait',
            reply_to=None,
            message_type='ask',
            delivery_scope=DeliveryScope.SINGLE,
            route_options={'mode': 'callback'},
        )
    ).jobs[0].job_id
    edge = CallbackEdgeStore(layout).get_latest_for_child_job(child_job_id)
    assert edge is not None
    assert edge.state is CallbackEdgeState.PENDING
    dispatcher.tick()
    dispatcher.complete(parent_job_id, _decision(reply='delegated to child'))

    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    view = service.build_response()['view']
    agent1 = next(agent for agent in view['agents'] if agent['name'] == 'agent1')
    agent2 = next(agent for agent in view['agents'] if agent['name'] == 'agent2')

    assert agent1['activity_state'] == 'pending'
    assert agent1['activity_source'] == 'callback'
    assert agent1['activity_reason'] == 'callback_waiting_child'
    assert agent1['callback_waiting_child_job_id'] == child_job_id
    assert agent1['callback_waiting_child_agent'] == 'agent2'
    assert agent1['callback_waiting_state'] == 'pending'
    assert agent2['activity_state'] == 'active'
    assert agent2['activity_source'] == 'ccb_job'
    assert agent2['activity_reason'] == 'job_running'
    assert agent2['current_job_id'] == child_job_id


def test_project_view_comms_collapses_retry_attempts_by_message(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-comms-retry-lineage'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    first = _job(
        project_id,
        job_id='job_retry_failed',
        sender='agent2',
        target='agent1',
        status=JobStatus.FAILED,
        updated_at='2026-05-20T12:00:01Z',
        terminal_reason='transport_error',
        body='recover this task',
    )
    latest = _job(
        project_id,
        job_id='job_retry_completed',
        sender='agent2',
        target='agent1',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:02Z',
        body='recover this task',
    )
    dispatcher._append_job(first)
    dispatcher._append_job(latest)
    dispatcher._message_bureau_control._attempt_store.append(
        AttemptRecord(
            attempt_id='att_retry_failed',
            message_id='msg_retry_lineage',
            agent_name='agent1',
            provider='codex',
            job_id=first.job_id,
            retry_index=0,
            health_snapshot_ref=None,
            started_at=first.created_at,
            updated_at=first.updated_at,
            attempt_state=AttemptState.FAILED,
        )
    )
    dispatcher._message_bureau_control._attempt_store.append(
        AttemptRecord(
            attempt_id='att_retry_completed',
            message_id='msg_retry_lineage',
            agent_name='agent1',
            provider='codex',
            job_id=latest.job_id,
            retry_index=1,
            health_snapshot_ref=None,
            started_at=latest.created_at,
            updated_at=latest.updated_at,
            attempt_state=AttemptState.COMPLETED,
        )
    )
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    comms = service.build_response()['view']['comms']

    assert [item['id'] for item in comms] == [latest.job_id]
    assert comms[0]['status'] == 'completed'
    assert comms[0]['status_label'] == 'back'
    assert comms[0]['recoverable'] is False


def test_project_view_comms_folds_reply_delivery_into_source_ask(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-comms-replies'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    source = _job(
        project_id,
        job_id='job_source_1234',
        sender='agent2',
        target='agent3',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:01Z',
        body='review the cross-window routing result',
    )
    dispatcher._append_job(source)
    reply_delivery = _reply_delivery_job(
        project_id,
        job_id='job_delivery_5678',
        source_agent='agent3',
        source_job_id=source.job_id,
        target='agent2',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:02Z',
    )
    dispatcher._append_job(reply_delivery)
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    comms = service.build_response()['view']['comms']

    assert [item['id'] for item in comms] == [source.job_id]
    assert comms[0]['sender'] == 'agent2'
    assert comms[0]['target'] == 'agent3'
    assert comms[0]['status'] == 'completed'
    assert comms[0]['business_status'] == 'replied'
    assert comms[0]['status_label'] == 'done'
    assert comms[0]['body_preview'] == 'review the cross-window routing result'
    assert comms[0]['reply_status'] == 'completed'
    assert comms[0]['reply_delivery_job_id'] == reply_delivery.job_id


def test_project_view_comms_folds_reply_delivery_by_reply_record_without_body_job(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-comms-replies-structured'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    source = _job(
        project_id,
        job_id='job_source_structured',
        sender='agent2',
        target='agent3',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:01Z',
        body='check structured reply delivery folding',
    )
    dispatcher._append_job(source)
    _record_reply_for_source(dispatcher, source, reply_id='reply_structured')
    reply_delivery = _reply_delivery_job(
        project_id,
        job_id='job_delivery_structured',
        source_agent='agent3',
        source_job_id='',
        target='agent2',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:02Z',
        reply_id='reply_structured',
        body='CCB_REPLY from=agent3 reply=reply_structured status=completed\n\nOK',
    )
    dispatcher._append_job(reply_delivery)
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    comms = service.build_response()['view']['comms']

    assert [item['id'] for item in comms] == [source.job_id]
    assert comms[0]['business_status'] == 'replied'
    assert comms[0]['reply_status'] == 'completed'
    assert comms[0]['reply_delivery_job_id'] == reply_delivery.job_id
    assert comms[0]['body_preview'] == 'check structured reply delivery folding'


def test_project_view_resolves_reply_delivery_sources_without_jsonl_list_all(tmp_path: Path, monkeypatch) -> None:
    project_root = tmp_path / 'repo-comms-indexed-replies'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    for index in range(3):
        source = _job(
            project_id,
            job_id=f'job_source_indexed_{index}',
            sender='agent2',
            target='agent3',
            status=JobStatus.COMPLETED,
            updated_at=f'2026-05-20T12:00:0{index}Z',
            body=f'indexed reply delivery {index}',
        )
        dispatcher._append_job(source)
        _record_reply_for_source(dispatcher, source, reply_id=f'reply_indexed_{index}')
        dispatcher._append_job(
            _reply_delivery_job(
                project_id,
                job_id=f'job_delivery_indexed_{index}',
                source_agent='agent3',
                source_job_id='',
                target='agent2',
                status=JobStatus.COMPLETED,
                updated_at=f'2026-05-20T12:00:1{index}Z',
                reply_id=f'reply_indexed_{index}',
                body=f'CCB_REPLY from=agent3 reply=reply_indexed_{index} status=completed\n\nOK',
            )
        )
    attempt_store = dispatcher._message_bureau_control._attempt_store
    reply_store = dispatcher._message_bureau_control._reply_store
    message_store = dispatcher._message_bureau_control._message_store
    monkeypatch.setattr(attempt_store, 'list_all', lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError('unexpected attempt list_all')))
    monkeypatch.setattr(attempt_store, 'list_message', lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError('unexpected attempt list_message')))
    monkeypatch.setattr(reply_store, 'list_all', lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError('unexpected reply list_all')))
    monkeypatch.setattr(message_store, 'list_all', lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError('unexpected message list_all')))
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    comms = service.build_response()['view']['comms']

    assert [item['id'] for item in comms] == [
        'job_source_indexed_2',
        'job_source_indexed_1',
        'job_source_indexed_0',
    ]
    assert {item['business_status'] for item in comms} == {'replied'}


def test_reply_delivery_lookup_fallback_avoids_jsonl_list_all(tmp_path: Path, monkeypatch) -> None:
    project_root = tmp_path / 'repo-comms-fallback'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    source = _job(
        project_id,
        job_id='job_source_fallback',
        sender='agent2',
        target='agent3',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:01Z',
        body='fallback reply delivery',
    )
    dispatcher._append_job(source)
    _record_reply_for_source(dispatcher, source, reply_id='reply_fallback')
    delivery = _reply_delivery_job(
        project_id,
        job_id='job_delivery_fallback',
        source_agent='agent3',
        source_job_id='',
        target='agent2',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:02Z',
        reply_id='reply_fallback',
        body='CCB_REPLY from=agent3 reply=reply_fallback status=completed\n\nOK',
    )
    dispatcher._append_job(delivery)
    attempt_store = dispatcher._message_bureau_control._attempt_store
    reply_store = dispatcher._message_bureau_control._reply_store
    message_store = dispatcher._message_bureau_control._message_store
    monkeypatch.setattr(attempt_store, 'list_all', lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError('unexpected attempt list_all')))
    monkeypatch.setattr(attempt_store, 'list_message', lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError('unexpected attempt list_message')))
    monkeypatch.setattr(reply_store, 'list_all', lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError('unexpected reply list_all')))
    monkeypatch.setattr(message_store, 'list_all', lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError('unexpected message list_all')))

    deliveries = project_view_service._reply_deliveries_by_source_job_id(dispatcher, (source, delivery))

    assert deliveries == {source.job_id: delivery}


def test_project_view_recent_jobs_uses_bounded_tail_reads(tmp_path: Path, monkeypatch) -> None:
    project_root = tmp_path / 'repo-comms-tail'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    for index in range(160):
        dispatcher._append_job(
            _job(
                project_id,
                job_id=f'job_tail_{index:03d}',
                sender='cmd',
                target='agent1',
                status=JobStatus.COMPLETED,
                updated_at=f'2026-05-20T12:{index // 60:02d}:{index % 60:02d}Z',
                body=f'tail job {index}',
            )
        )
    monkeypatch.setattr(
        dispatcher._job_store,
        'list_agent',
        lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError('unexpected full list_agent')),
    )
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    comms = service.build_response()['view']['comms']

    assert [item['id'] for item in comms] == [
        'job_tail_159',
        'job_tail_158',
        'job_tail_157',
        'job_tail_156',
        'job_tail_155',
        'job_tail_154',
        'job_tail_153',
        'job_tail_152',
    ]


def test_project_view_backfills_reply_delivery_source_outside_recent_tail(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-comms-delivery-tail'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    source = _job(
        project_id,
        job_id='job_old_reply_source',
        sender='agent2',
        target='agent3',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T10:00:00Z',
        body='old source outside recent tail',
    )
    dispatcher._append_job(source)
    _record_reply_for_source(dispatcher, source, reply_id='reply_old_source')
    for index in range(140):
        dispatcher._append_job(
            _job(
                project_id,
                job_id=f'job_tail_filler_{index:03d}',
                sender='cmd',
                target='agent3',
                status=JobStatus.COMPLETED,
                updated_at=f'2026-05-20T11:{index // 60:02d}:{index % 60:02d}Z',
                body=f'filler {index}',
            )
        )
    delivery = _reply_delivery_job(
        project_id,
        job_id='job_recent_reply_delivery',
        source_agent='agent3',
        source_job_id='',
        target='agent2',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:30Z',
        reply_id='reply_old_source',
        body='CCB_REPLY from=agent3 reply=reply_old_source status=completed\n\nOK',
    )
    dispatcher._append_job(delivery)
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    comms = service.build_response()['view']['comms']

    assert comms[0]['id'] == source.job_id
    assert comms[0]['business_status'] == 'replied'
    assert comms[0]['reply_delivery_job_id'] == delivery.job_id


def test_project_view_comms_marks_agent_reply_delivery_pending(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-comms-pending-reply'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    source = _job(
        project_id,
        job_id='job_source_waiting',
        sender='agent2',
        target='agent3',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:01Z',
    )
    dispatcher._append_job(source)
    cmd_source = _job(
        project_id,
        job_id='job_cmd_source',
        sender='cmd',
        target='agent3',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:02Z',
    )
    dispatcher._append_job(cmd_source)
    silent_source = _job(
        project_id,
        job_id='job_silent_source',
        sender='agent1',
        target='agent3',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:03Z',
        silence_on_success=True,
    )
    dispatcher._append_job(silent_source)
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    comms_by_id = {item['id']: item for item in service.build_response()['view']['comms']}

    assert comms_by_id[source.job_id]['business_status'] == 'delivering'
    assert comms_by_id[cmd_source.job_id]['business_status'] == 'replied'
    assert comms_by_id[silent_source.job_id]['business_status'] == 'completed'


def test_project_view_comms_cleans_instructional_body_preview(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-comms-preview-cleanup'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    english = _job(
        project_id,
        job_id='job_reply_exactly',
        sender='cmd',
        target='agent3',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:01Z',
        body='Reply exactly: COMMS_BUSINESS_VIEW_OK',
    )
    chinese = _job(
        project_id,
        job_id='job_only_reply',
        sender='cmd',
        target='agent2',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:02Z',
        body='只回复 CONCURRENT_A_OK',
    )
    probe = _job(
        project_id,
        job_id='job_probe_reply',
        sender='cmd',
        target='agent1',
        status=JobStatus.COMPLETED,
        updated_at='2026-05-20T12:00:03Z',
        body='只回复 D23R_OK',
    )
    dispatcher._append_job(english)
    dispatcher._append_job(chinese)
    dispatcher._append_job(probe)
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
        )
    )

    comms_by_id = {item['id']: item for item in service.build_response()['view']['comms']}

    assert comms_by_id[english.job_id]['body_preview'] == 'smoke: comms business view'
    assert comms_by_id[chinese.job_id]['body_preview'] == 'smoke: concurrent a'
    assert comms_by_id[probe.job_id]['body_preview'] == 'probe: D23R'


def test_project_view_sequence_ignores_generated_at_only(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    timestamps = iter(['2026-05-20T12:00:00Z', '2026-05-20T12:00:01Z'])
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: next(timestamps),
            sequence_cache=ProjectViewSequenceCache(),
        )
    )

    first = service.build_response()
    second = service.build_response()

    assert first['cache']['generated_at'] == second['cache']['generated_at']
    assert first['view']['generated_at'] == second['view']['generated_at']
    assert first['cache']['sequence'] == second['cache']['sequence']


def test_project_view_sequence_changes_when_content_changes(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    registry.upsert(_runtime('agent1', project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
            sequence_cache=ProjectViewSequenceCache(),
            cache_ttl_ms=0,
        )
    )

    first = service.build_response()
    running = _job(project_id, job_id='job_running_1234', sender='agent2', target='agent1', status=JobStatus.RUNNING)
    dispatcher._append_job(running)
    dispatcher._state.mark_active_for(TargetKind.AGENT, 'agent1', running.job_id)
    second = service.build_response()

    assert first['cache']['sequence'] == 1
    assert second['cache']['sequence'] == 2
    assert first['view']['agents'][0]['activity_state'] == 'idle'
    assert second['view']['agents'][0]['activity_state'] == 'active'
    assert second['view']['agents'][0]['activity_source'] == 'ccb_job'
    assert second['view']['agents'][0]['activity_reason'] == 'job_running'
    assert second['view']['agents'][0]['current_job_id'] == 'job_running_1234'
    assert [item['id'] for item in second['view']['comms']] == ['job_running_1234']


def test_project_view_cache_hit_skips_tmux_calls(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-cache-tmux'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    registry.upsert(_runtime('agent1', project_id=project_id))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-cache-tmux',
            layout_version=2,
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    backend = _SnapshotBackend()
    controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: backend,
    )
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            clock=lambda: NOW,
        )
    )

    first = service.build_response()
    backend.calls.clear()
    second = service.build_response()

    assert first is second
    assert backend.calls == []


def test_project_view_updates_build_cache_and_tmux_metrics(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-project-view-metrics'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    registry.upsert(_runtime('agent3', project_id=project_id, state=AgentState.BUSY))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-project-view-metrics',
            layout_version=2,
        )
    )
    old_job = _job(
        project_id,
        job_id='job_project_view_metrics',
        sender='agent1',
        target='agent3',
        status=JobStatus.RUNNING,
        updated_at='2026-05-20T11:57:00Z',
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    dispatcher._append_job(old_job)
    dispatcher._state.mark_active_for(TargetKind.AGENT, 'agent3', old_job.job_id)
    backend = _ProviderIdleWithoutAnchorBackend()
    controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: backend,
    )
    metrics = SimpleNamespace(
        last_project_view_build_duration_s=None,
        last_project_view_response_duration_s=None,
        project_view_cache_hits=0,
        project_view_cache_misses=0,
        last_project_view_tmux_command_count=None,
        last_project_view_capture_pane_count=None,
        last_project_view_store_scan_count=None,
    )
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            clock=lambda: NOW,
            metrics=metrics,
        )
    )

    first = service.build_response()
    second = service.build_response()

    assert first is second
    assert metrics.project_view_cache_misses == 1
    assert metrics.project_view_cache_hits == 1
    assert metrics.last_project_view_build_duration_s >= 0.0
    assert metrics.last_project_view_response_duration_s >= 0.0
    assert metrics.last_project_view_tmux_command_count == 4
    assert metrics.last_project_view_capture_pane_count == 1
    assert metrics.last_project_view_store_scan_count == 3
    service.invalidate_cache()
    service.build_response()
    assert metrics.project_view_cache_misses == 2
    assert metrics.last_project_view_tmux_command_count == 4
    assert metrics.last_project_view_capture_pane_count == 1
    assert metrics.last_project_view_store_scan_count == 3


class _FocusBackend:
    def _tmux_run(self, args: list[str], *, capture=False, check=False, timeout=None):
        del check, timeout
        assert capture is True
        if args[:3] == ['display-message', '-p', '-t']:
            return type('CP', (), {'returncode': 0, 'stdout': 'ops\t%2\tagent\tagent3\n', 'stderr': ''})()
        raise AssertionError(args)


class _SnapshotBackend:
    def __init__(self) -> None:
        self.calls: list[list[str]] = []

    def _tmux_run(self, args: list[str], *, capture=False, check=False, timeout=None):
        self.calls.append(list(args))
        return self._snapshot_run(args, capture=capture, check=check, timeout=timeout)

    def _snapshot_run(self, args: list[str], *, capture=False, check=False, timeout=None):
        del check, timeout
        assert capture is True
        if args[:3] == ['display-message', '-p', '-t']:
            return type('CP', (), {'returncode': 0, 'stdout': 'main\t%11\tagent\tagent1\n', 'stderr': ''})()
        if args[:2] == ['list-windows', '-t']:
            return type(
                'CP',
                (),
                {
                    'returncode': 0,
                    'stdout': 'main\t@1\t0\nops\t@2\t1\n',
                    'stderr': '',
                },
            )()
        if args[:2] == ['list-panes', '-a']:
            return type(
                'CP',
                (),
                {
                    'returncode': 0,
                    'stdout': (
                        'ccb-snap\tmain\t%90\tproj-snap\tsidebar\tmain\tmain\n'
                        'ccb-snap\tops\t%91\tproj-snap\tsidebar\tops\tops\n'
                        'other\tmain\t%99\tproj-snap\tsidebar\tmain\tmain\n'
                    ),
                    'stderr': '',
                },
            )()
        if args[:3] == ['capture-pane', '-p', '-t']:
            return type('CP', (), {'returncode': 0, 'stdout': '', 'stderr': ''})()
        raise AssertionError(args)


class _ProviderPromptBackend(_SnapshotBackend):
    def _tmux_run(self, args: list[str], *, capture=False, check=False, timeout=None):
        self.calls.append(list(args))
        if args[:3] == ['capture-pane', '-p', '-t']:
            return type(
                'CP',
                (),
                {
                    'returncode': 0,
                    'stdout': (
                        'Do you trust the contents of this directory?\n'
                        'Working with untrusted contents comes with higher risk.\n'
                        'Press enter to continue\n'
                    ),
                    'stderr': '',
                },
            )()
        return self._snapshot_run(args, capture=capture, check=check, timeout=timeout)


class _ProviderIdleAfterRequestBackend(_SnapshotBackend):
    def __init__(self, job_id: str) -> None:
        super().__init__()
        self._job_id = job_id

    def _tmux_run(self, args: list[str], *, capture=False, check=False, timeout=None):
        self.calls.append(list(args))
        if args[:3] == ['capture-pane', '-p', '-t']:
            return type(
                'CP',
                (),
                {
                    'returncode': 0,
                    'stdout': (
                        f'❯ CCB_REQ_ID: {self._job_id}\n\n'
                        '  cancelled in provider\n\n'
                        '● cancelled\n'
                        '────────────────────────────────\n'
                        '❯ \n'
                        '🤖 Sonnet 4.6 | 📁 repo\n'
                    ),
                    'stderr': '',
                },
            )()
        return self._snapshot_run(args, capture=capture, check=check, timeout=timeout)


class _ProviderIdleWithoutAnchorBackend(_SnapshotBackend):
    def _tmux_run(self, args: list[str], *, capture=False, check=False, timeout=None):
        self.calls.append(list(args))
        if args[:3] == ['capture-pane', '-p', '-t']:
            return type(
                'CP',
                (),
                {
                    'returncode': 0,
                    'stdout': (
                        'Claude Code v2.1.142\n'
                        '/repo\n\n'
                        '────────────────────────────────\n'
                        '❯ \n'
                        '🤖 Sonnet 4.6 | 📁 repo\n'
                    ),
                    'stderr': '',
                },
            )()
        return self._snapshot_run(args, capture=capture, check=check, timeout=timeout)


def test_project_view_marks_active_window_and_agent_from_namespace_focus(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-focus'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=2,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-focus',
            layout_version=2,
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: _FocusBackend(),
    )
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            clock=lambda: NOW,
        )
    )

    view = service.build_response()['view']

    assert view['namespace']['active_window'] == 'ops'
    assert view['namespace']['active_pane_id'] == '%2'
    assert [window['active'] for window in view['windows']] == [False, True]
    assert [agent['active'] for agent in view['agents']] == [False, False, True]


def test_project_view_reads_window_and_sidebar_tmux_metadata(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-snapshot'
    project_root.mkdir()
    layout = PathLayout(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id='proj-snap', pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id='proj-snap',
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-snap',
            layout_version=2,
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    controller = ProjectNamespaceController(
        layout,
        'proj-snap',
        backend_factory=lambda socket_path=None: _SnapshotBackend(),
    )
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id='proj-snap',
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            clock=lambda: NOW,
        )
    )

    view = service.build_response()['view']

    assert view['namespace']['active_window'] == 'main'
    assert [
        (window['name'], window['tmux_window_id'], window['tmux_window_index'], window['sidebar_pane_id'])
        for window in view['windows']
    ] == [
        ('main', '@1', 0, '%90'),
        ('ops', '@2', 1, '%91'),
    ]


def test_project_view_captures_each_running_pane_once_per_build(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-capture-once'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    registry.upsert(_runtime('agent3', project_id=project_id, state=AgentState.BUSY))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-capture-once',
            layout_version=2,
        )
    )
    old_job = _job(
        project_id,
        job_id='job_capture_once',
        sender='agent1',
        target='agent3',
        status=JobStatus.RUNNING,
        updated_at='2026-05-20T11:57:00Z',
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    dispatcher._append_job(old_job)
    dispatcher._state.mark_active_for(TargetKind.AGENT, 'agent3', old_job.job_id)
    backend = _ProviderIdleWithoutAnchorBackend()
    controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: backend,
    )
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            clock=lambda: NOW,
        )
    )

    view = service.build_response()['view']

    assert view['comms'][0]['id'] == old_job.job_id
    assert [
        args
        for args in backend.calls
        if args[:3] == ['capture-pane', '-p', '-t'] and args[3] == '%3'
    ] == [['capture-pane', '-p', '-t', '%3', '-S', '-30']]


def test_project_view_reuses_namespace_backend_within_build(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-backend-once'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    registry.upsert(_runtime('agent3', project_id=project_id, state=AgentState.BUSY))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-backend-once',
            layout_version=2,
        )
    )
    old_job = _job(
        project_id,
        job_id='job_backend_once',
        sender='agent1',
        target='agent3',
        status=JobStatus.RUNNING,
        updated_at='2026-05-20T11:57:00Z',
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    dispatcher._append_job(old_job)
    dispatcher._state.mark_active_for(TargetKind.AGENT, 'agent3', old_job.job_id)
    backend = _ProviderIdleWithoutAnchorBackend()
    backend_factory_calls = []
    controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: backend_factory_calls.append(socket_path) or backend,
    )
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            clock=lambda: NOW,
        )
    )

    service.build_response()

    assert backend_factory_calls == [str(layout.ccbd_tmux_socket_path)]


def test_project_view_marks_provider_prompt_as_pending(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-prompt'
    project_root.mkdir()
    layout = PathLayout(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    registry.upsert(_runtime('agent1', project_id='proj-prompt'))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id='proj-prompt', pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id='proj-prompt',
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-prompt',
            layout_version=2,
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    controller = ProjectNamespaceController(
        layout,
        'proj-prompt',
        backend_factory=lambda socket_path=None: _ProviderPromptBackend(),
    )
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id='proj-prompt',
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            clock=lambda: NOW,
        )
    )

    agent1 = service.build_response()['view']['agents'][0]

    assert agent1['activity_state'] == 'pending'
    assert agent1['activity_source'] == 'provider_prompt'
    assert agent1['activity_reason'] == 'provider_waiting_for_user'


def test_project_view_marks_running_job_idle_after_provider_prompt_reappears(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-idle-running'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    registry.upsert(_runtime('agent3', project_id=project_id, state=AgentState.BUSY))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-idle-running',
            layout_version=2,
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    job_id = _submit(dispatcher, project_id, sender='agent1', target='agent3', body='cancelled in provider')
    dispatcher.tick()
    job = dispatcher.get(job_id)
    assert job is not None
    job = replace(job, updated_at='2026-05-20T11:59:20Z')
    dispatcher._append_job(job)
    controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: _ProviderIdleAfterRequestBackend(job.job_id),
    )
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            clock=lambda: NOW,
        )
    )

    view = service.build_response()['view']
    agent3 = next(agent for agent in view['agents'] if agent['name'] == 'agent3')
    comm = view['comms'][0]

    assert agent3['activity_state'] == 'pending'
    assert agent3['activity_source'] == 'provider_prompt'
    assert agent3['activity_reason'] == 'provider_prompt_idle'
    assert agent3['current_job_id'] == job.job_id
    assert comm['id'] == job.job_id
    assert comm['business_status'] == 'blocked'
    assert comm['status_label'] == 'stuck'
    assert comm['recoverable'] is True
    assert comm['block_reason'] == 'provider_prompt_idle'
    assert comm['recover_target'] == {
        'job_id': job.job_id,
        'reply_delivery_job_id': None,
        'block_reason': 'provider_prompt_idle',
    }


def test_project_view_does_not_mark_fresh_running_prompt_idle_as_recoverable(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-idle-running-fresh'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    registry.upsert(_runtime('agent3', project_id=project_id, state=AgentState.BUSY))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-idle-running-fresh',
            layout_version=2,
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    job_id = _submit(dispatcher, project_id, sender='agent1', target='agent3', body='fresh running prompt')
    dispatcher.tick()
    job = dispatcher.get(job_id)
    assert job is not None
    controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: _ProviderIdleAfterRequestBackend(job.job_id),
    )
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            clock=lambda: NOW,
        )
    )

    view = service.build_response()['view']
    agent3 = next(agent for agent in view['agents'] if agent['name'] == 'agent3')
    comm = view['comms'][0]

    assert agent3['activity_state'] == 'active'
    assert agent3['activity_source'] == 'ccb_job'
    assert agent3['activity_reason'] == 'job_running'
    assert agent3['current_job_id'] == job.job_id
    assert comm['id'] == job.job_id
    assert comm['business_status'] == 'replying'
    assert comm['status_label'] == 'work'
    assert comm['recoverable'] is False
    assert comm['block_reason'] is None


def test_project_view_marks_stale_running_job_recoverable_when_provider_prompt_is_idle_without_anchor(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo-provider-idle-no-anchor'
    project_root.mkdir()
    layout = PathLayout(project_root)
    project_id = compute_project_id(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    registry.upsert(_runtime('agent3', project_id=project_id, state=AgentState.BUSY))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-idle-no-anchor',
            layout_version=2,
        )
    )
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    old_job = _job(
        project_id,
        job_id='job_prompt_idle_stale',
        sender='agent1',
        target='agent3',
        status=JobStatus.RUNNING,
        updated_at='2026-05-20T11:57:00Z',
        body='lost anchor in scrollback',
    )
    dispatcher._append_job(old_job)
    dispatcher._state.record(old_job)
    dispatcher._state.mark_active_for(TargetKind.AGENT, 'agent3', old_job.job_id)
    controller = ProjectNamespaceController(
        layout,
        project_id,
        backend_factory=lambda socket_path=None: _ProviderIdleWithoutAnchorBackend(),
    )
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=project_root,
            project_id=project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            namespace_controller=controller,
            clock=lambda: NOW,
        )
    )

    comm = service.build_response()['view']['comms'][0]

    assert comm['id'] == old_job.job_id
    assert comm['business_status'] == 'blocked'
    assert comm['status_label'] == 'stuck'
    assert comm['recoverable'] is True
    assert comm['block_reason'] == 'provider_prompt_idle_stale'
    assert comm['recover_target'] == {
        'job_id': old_job.job_id,
        'reply_delivery_job_id': None,
        'block_reason': 'provider_prompt_idle_stale',
    }


def test_activity_resolver_core_states() -> None:
    assert resolve_agent_activity(AgentActivityFacts(namespace_mounted=False), now=NOW).state == 'offline'
    queued = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='idle',
            pane_id='%1',
            pane_state='alive',
            current_job_status='queued',
            current_job_id='job_queued',
            current_job_updated_at=NOW,
        ),
        now=NOW,
    )
    assert queued.state == 'pending'
    assert queued.source == 'ccb_job'
    assert queued.reason == 'job_queued'
    assert queued.current_job_id == 'job_queued'
    running = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='idle',
            pane_id='%1',
            pane_state='alive',
            current_job_status='running',
            current_job_id='job_running',
            current_job_updated_at=NOW,
        ),
        now=NOW,
    )
    assert running.state == 'active'
    assert running.source == 'ccb_job'
    assert running.reason == 'job_running'
    assert running.current_job_id == 'job_running'
    stale = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='idle',
            pane_id='%1',
            pane_state='alive',
            current_job_status='running',
            current_job_id='job_stale',
            current_job_updated_at='2026-05-20T11:57:00Z',
        ),
        now=NOW,
    )
    assert stale.state == 'pending'
    assert stale.source == 'ccb_job'
    assert stale.reason == 'job_running_stale'
    assert resolve_agent_activity(
        AgentActivityFacts(namespace_mounted=True, pane_id='%1', pane_state='missing', reconcile_state='recovering'),
        now=NOW,
    ).reason == 'pane_missing_recovering'
    assert resolve_agent_activity(
        AgentActivityFacts(namespace_mounted=True, pane_id='%1', pane_state='missing'),
        now=NOW,
    ).reason == 'pane_missing_unowned'


def test_activity_resolver_ignores_terminal_job_status_for_top_activity() -> None:
    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='idle',
            pane_id='%1',
            pane_state='alive',
            current_job_status='failed',
            current_job_id='job_failed',
            current_job_updated_at=NOW,
        ),
        now=NOW,
    )

    assert activity.state == 'idle'
    assert activity.source == 'pane_liveness'
    assert activity.reason == 'pane_alive'
    assert activity.current_job_id is None


def test_activity_resolver_provider_prompt() -> None:
    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='idle',
            pane_id='%1',
            pane_state='alive',
            pane_text='Do you trust the contents of this directory?\nPress enter to continue\n',
        ),
        now=NOW,
    )

    assert activity.state == 'pending'
    assert activity.source == 'provider_prompt'
    assert activity.reason == 'provider_waiting_for_user'


def test_activity_resolver_ignores_stale_provider_prompt_after_codex_idle_prompt() -> None:
    pane_text = '\n'.join(
        [
            'Do you trust the contents of this directory?',
            'Press enter to continue',
            '',
            '› CCB_REQ_ID: job_old',
            '',
            '• done',
            '',
            '› Implement {feature}',
            '',
            '  gpt-5.5 xhigh · ~/yunwei/test_ccb2',
        ]
    )

    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='idle',
            pane_id='%1',
            pane_state='alive',
            pane_text=pane_text,
        ),
        now=NOW,
    )

    assert activity.state == 'idle'
    assert activity.source == 'pane_liveness'
    assert activity.reason == 'pane_alive'


def test_activity_resolver_treats_idle_prompt_after_request_as_agent_idle() -> None:
    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='busy',
            pane_id='%1',
            pane_state='alive',
            pane_text='❯ CCB_REQ_ID: job_prompt_idle_2\n\ncancelled\n\n❯ \n',
        ),
        now=NOW,
    )

    assert activity.state == 'idle'
    assert activity.source == 'pane_liveness'
    assert activity.reason == 'pane_alive'


def test_activity_resolver_ignores_fresh_job_when_provider_prompt_is_idle() -> None:
    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='busy',
            pane_id='%1',
            pane_state='alive',
            pane_text='❯ CCB_REQ_ID: job_prompt_idle_new\n\n❯ \n',
        ),
        now=NOW,
    )

    assert activity.state == 'idle'
    assert activity.reason == 'pane_alive'


def test_activity_resolver_does_not_use_job_state_without_provider_working_signal() -> None:
    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='busy',
            pane_id='%1',
            pane_state='alive',
            pane_text='❯ CCB_REQ_ID: job_still_waiting\n\nworking on task\n',
        ),
        now=NOW,
    )

    assert activity.state == 'idle'
    assert activity.source == 'pane_liveness'
    assert activity.reason == 'pane_alive'


def test_activity_resolver_keeps_input_stuck_detection_in_comms_not_agent_status() -> None:
    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='busy',
            pane_id='%1',
            pane_state='alive',
            pane_text='❯ CCB_REQ_ID: job_input_stuck\n\n  查询北京天气\n',
        ),
        now=NOW,
    )

    assert activity.state == 'idle'
    assert activity.source == 'pane_liveness'
    assert activity.reason == 'pane_alive'


def test_activity_resolver_ignores_fresh_input_stuck_job_for_agent_status() -> None:
    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='busy',
            pane_id='%1',
            pane_state='alive',
            pane_text='❯ CCB_REQ_ID: job_input_new\n\n  查询北京天气\n',
        ),
        now=NOW,
    )

    assert activity.state == 'idle'
    assert activity.reason == 'pane_alive'


def test_activity_resolver_provider_prompt_does_not_hide_running_tool() -> None:
    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='busy',
            pane_id='%1',
            pane_state='alive',
            pane_text='❯ CCB_REQ_ID: job_running_tool\n\nBash(sleep 60)\n⎿ Running… (10s)\n\n❯ \n',
        ),
        now=NOW,
    )

    assert activity.state == 'active'
    assert activity.source == 'provider_pane'
    assert activity.reason == 'provider_working'


def test_activity_resolver_provider_working_pane() -> None:
    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='idle',
            pane_id='%1',
            pane_state='alive',
            pane_text='Working (28s • esc to interrupt)',
        ),
        now=NOW,
    )

    assert activity.state == 'active'
    assert activity.source == 'provider_pane'
    assert activity.reason == 'provider_working'


def test_activity_resolver_provider_background_terminal_running_after_prompt() -> None:
    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='idle',
            pane_id='%1',
            pane_state='alive',
            pane_text=(
                '• Working (20s • esc to interrupt) · 1 background terminal running · /ps to view · /stop to close\n'
                '\n'
                '› Use /skills to list available skills\n'
                '\n'
                '  gpt-5.5 medium · ~/yunwei/test_ccb2\n'
            ),
        ),
        now=NOW,
    )

    assert activity.state == 'active'
    assert activity.source == 'provider_pane'
    assert activity.reason == 'provider_working'


def test_activity_resolver_claude_scheduled_task_shell_running_after_prompt() -> None:
    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='idle',
            pane_id='%1',
            pane_state='alive',
            pane_text=(
                '✻ Running scheduled task (Jun 11 9:33pm)\n'
                '\n'
                '● 已完成第29次，正在执行第30次。进度：29/50（58%）。\n'
                '\n'
                '✻ Sautéed for 9s · 1 shell still running\n'
                '\n'
                '─────────────────────────────────────────────────────\n'
                '❯ CCB_REQ_ID: job_86550847f237\n'
                '\n'
                '  再执行一次：循环50次，每次等待30s。\n'
                '─────────────────────────────────────────────────────\n'
                '  🤖 Sonnet 4.6 | 📁 test_ccb2 | ⏵⏵ bypass permissions on · 1 shell\n'
            ),
        ),
        now=NOW,
    )

    assert activity.state == 'active'
    assert activity.source == 'provider_pane'
    assert activity.reason == 'provider_working'


def test_activity_resolver_keeps_codex_queued_message_active() -> None:
    pane_text = '\n'.join(
        [
            '╭───────────────────────────────────────────────╮',
            '│ >_ OpenAI Codex (v0.134.0)                    │',
            '│ model:       gpt-5.5 xhigh   /model to change │',
            '╰───────────────────────────────────────────────╯',
            '',
            '› trigger CCB_REFUSED_SMOKE and stop',
            '',
            '• Working (18s • esc to interrupt)',
            '• Messages to be submitted after next tool call (press esc to interrupt and send immediately)',
            '  ↳ trigger CCB_NEXT_AFTER_FAIL and stop',
            '',
            '› Use /skills to list available skills',
            '',
            '  gpt-5.5 xhigh · ~/yunwei/test_ccb2',
        ]
    )

    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='idle',
            pane_id='%3',
            pane_state='alive',
            pane_text=pane_text,
            provider_activity_state='active',
            provider_activity_source='codex_hook',
            provider_activity_reason='provider_userpromptsubmit',
            provider_activity_updated_at=NOW,
        ),
        now=NOW,
    )

    assert activity.state == 'active'
    assert activity.source == 'codex_hook'
    assert activity.reason == 'provider_userpromptsubmit'


def test_activity_resolver_ignores_stale_provider_working_history() -> None:
    pane_text = '\n'.join(
        [
            '• Booting MCP server: puppeteer (0s • esc to interrupt)',
            '',
            '› Find and fix a bug in @filename',
            '',
            '╭───────────────────────────────────────────────╮',
            '│ >_ OpenAI Codex (v0.133.0)                    │',
            '│                                               │',
            '│ model:       gpt-5.5 xhigh   /model to change │',
            '│ directory:   ~/yunwei/ccb_sidebar_test        │',
            '│ permissions: YOLO mode                        │',
            '╰───────────────────────────────────────────────╯',
            '',
            '  gpt-5.5 xhigh · ~/yunwei/ccb_sidebar_test',
            '',
            '',
            '',
            '',
            '',
            '',
            '',
            '',
            '',
            '',
            '',
        ]
    )
    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='idle',
            pane_id='%3',
            pane_state='alive',
            pane_text=pane_text,
        ),
        now=NOW,
    )

    assert activity.state == 'idle'
    assert activity.source == 'pane_liveness'
    assert activity.reason == 'pane_alive'


def test_activity_resolver_ignores_stale_working_history_after_tail_prompt() -> None:
    pane_text = '\n'.join(
        [
            'Working (28s • esc to interrupt)',
            '',
            '› Find and fix a bug in @filename',
            '',
            '• fixed',
            '',
            '› Run /review on my current changes',
            '',
            '  gpt-5.5 xhigh · ~/yunwei/test_ccb2',
        ]
    )

    activity = resolve_agent_activity(
        AgentActivityFacts(
            namespace_mounted=True,
            runtime_state='idle',
            pane_id='%3',
            pane_state='alive',
            pane_text=pane_text,
        ),
        now=NOW,
    )

    assert activity.state == 'idle'
    assert activity.source == 'pane_liveness'
    assert activity.reason == 'pane_alive'
