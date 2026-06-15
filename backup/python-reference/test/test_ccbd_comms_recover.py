from __future__ import annotations

from dataclasses import replace
from pathlib import Path

import pytest

from agents.models import (
    AgentRuntime,
    AgentSpec,
    AgentState,
    PermissionMode,
    ProjectConfig,
    QueuePolicy,
    RestoreMode,
    RuntimeMode,
    WorkspaceMode,
)
from ccbd.api_models import DeliveryScope, JobStatus, MessageEnvelope, TargetKind
from ccbd.project_view import ProjectViewDependencies, ProjectViewService
from ccbd.services.dispatcher import JobDispatcher
from ccbd.services.mount import MountManager
from ccbd.services.project_namespace_state import ProjectNamespaceStateStore
from ccbd.services.registry import AgentRegistry
from completion.models import CompletionConfidence, CompletionDecision, CompletionStatus
from mailbox_kernel import DeliveryLease, InboundEventStatus, InboundEventStore, InboundEventType, LeaseState, MailboxState, MailboxStore
from message_bureau import AttemptState, AttemptStore, MessageStore
from project.ids import compute_project_id
from project.resolver import ProjectContext
from storage.paths import PathLayout

NOW = '2026-05-22T12:00:00Z'


def _bootstrap(project_root: Path) -> ProjectContext:
    project_root.mkdir()
    config_dir = project_root / '.ccb'
    config_dir.mkdir()
    (config_dir / 'ccb.config').write_text('agent1:codex, agent2:claude, agent3:codex\n', encoding='utf-8')
    return ProjectContext(
        cwd=project_root,
        project_root=project_root,
        config_dir=config_dir,
        project_id=compute_project_id(project_root),
        source='test',
    )


def _config() -> ProjectConfig:
    agents = {
        name: AgentSpec(
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
        for name, provider in (('agent1', 'codex'), ('agent2', 'claude'), ('agent3', 'codex'))
    }
    return ProjectConfig(version=2, default_agents=('agent1', 'agent2', 'agent3'), agents=agents, cmd_enabled=False)


def _runtime(agent_name: str, *, project_id: str, layout: PathLayout, state: AgentState = AgentState.IDLE, health: str = 'healthy', pane_state: str = 'alive') -> AgentRuntime:
    return AgentRuntime(
        agent_name=agent_name,
        state=state,
        pid=100,
        started_at=NOW,
        last_seen_at=NOW,
        runtime_ref=f'tmux:%{agent_name[-1]}',
        session_ref=f'{agent_name}-session',
        workspace_path=str(layout.workspace_path(agent_name)),
        project_id=project_id,
        backend_type='tmux',
        queue_depth=0,
        socket_path=None,
        health=health,
        pane_id=f'%{agent_name[-1]}',
        pane_state=pane_state,
    )


def _dispatcher(project_root: Path) -> tuple[ProjectContext, PathLayout, ProjectConfig, AgentRegistry, JobDispatcher]:
    ctx = _bootstrap(project_root)
    layout = PathLayout(project_root)
    config = _config()
    registry = AgentRegistry(layout, config)
    for agent_name in config.agents:
        registry.upsert(_runtime(agent_name, project_id=ctx.project_id, layout=layout))
    dispatcher = JobDispatcher(layout, config, registry, clock=lambda: NOW)
    return ctx, layout, config, registry, dispatcher


def _submit(dispatcher: JobDispatcher, ctx: ProjectContext, *, sender: str, target: str, body: str = 'work') -> str:
    receipt = dispatcher.submit(
        MessageEnvelope(
            project_id=ctx.project_id,
            to_agent=target,
            from_actor=sender,
            body=body,
            task_id=None,
            reply_to=None,
            message_type='ask',
            delivery_scope=DeliveryScope.SINGLE,
        )
    )
    return receipt.jobs[0].job_id


def _decision(status: CompletionStatus, *, reason: str | None = None, reply: str = '') -> CompletionDecision:
    return CompletionDecision(
        terminal=True,
        status=status,
        reason=reason or status.value,
        confidence=CompletionConfidence.OBSERVED,
        reply=reply,
        anchor_seen=False,
        reply_started=False,
        reply_stable=False,
        provider_turn_ref=None,
        source_cursor=None,
        finished_at=NOW,
        diagnostics={},
    )


def test_comms_recover_does_not_cancel_healthy_running_job(tmp_path: Path) -> None:
    ctx, _layout, _config, _registry, dispatcher = _dispatcher(tmp_path / 'repo-running-healthy')
    job_id = _submit(dispatcher, ctx, sender='agent2', target='agent1')
    dispatcher.tick()

    payload = dispatcher.comms_recover({'job_id': job_id})

    assert payload['status'] == 'noop'
    assert payload['noop_reason'] == 'not_recoverable'
    assert dispatcher.get(job_id).status is JobStatus.RUNNING
    attempts = AttemptStore(dispatcher._layout).list_message(MessageStore(dispatcher._layout).list_all()[-1].message_id)
    assert {attempt.attempt_id for attempt in attempts} == {attempts[0].attempt_id}


def test_comms_recover_accepts_provider_prompt_idle_hint_for_running_job(tmp_path: Path) -> None:
    ctx, layout, _config, _registry, dispatcher = _dispatcher(tmp_path / 'repo-running-prompt-idle')
    stuck = _submit(dispatcher, ctx, sender='agent1', target='agent3', body='cancelled in provider')
    waiting = _submit(dispatcher, ctx, sender='agent1', target='agent3', body='next task')
    dispatcher.tick()

    payload = dispatcher.comms_recover({'job_id': stuck, 'block_reason': 'provider_prompt_idle'})

    assert payload['status'] == 'recovered'
    assert payload['block_reason'] == 'provider_prompt_idle'
    assert payload['cancelled_old']['job_id'] == stuck
    assert payload['retried_job']['agent_name'] == 'agent3'
    assert [job['job_id'] for job in payload['next_started']] == [waiting]
    assert dispatcher.get(stuck).status is JobStatus.CANCELLED
    assert dispatcher.get(waiting).status is JobStatus.RUNNING
    attempts = AttemptStore(layout).list_message(MessageStore(layout).list_all()[0].message_id)
    assert max(attempt.retry_index for attempt in attempts) == 1


def test_comms_recover_accepts_provider_prompt_idle_stale_hint_for_running_job(tmp_path: Path) -> None:
    ctx, _layout, _config, _registry, dispatcher = _dispatcher(tmp_path / 'repo-running-prompt-idle-stale')
    stuck = _submit(dispatcher, ctx, sender='agent1', target='agent3', body='lost anchor')
    dispatcher.tick()

    payload = dispatcher.comms_recover({'job_id': stuck, 'block_reason': 'provider_prompt_idle_stale'})

    assert payload['status'] == 'recovered'
    assert payload['block_reason'] == 'provider_prompt_idle_stale'
    assert payload['cancelled_old']['job_id'] == stuck
    assert payload['retried_job']['agent_name'] == 'agent3'
    assert dispatcher.get(stuck).status is JobStatus.CANCELLED


def test_comms_recover_accepts_provider_prompt_input_stuck_hint_for_running_job(tmp_path: Path) -> None:
    ctx, _layout, _config, _registry, dispatcher = _dispatcher(tmp_path / 'repo-running-input-stuck')
    stuck = _submit(dispatcher, ctx, sender='agent1', target='agent3', body='input stuck')
    dispatcher.tick()

    payload = dispatcher.comms_recover({'job_id': stuck, 'block_reason': 'provider_prompt_input_stuck'})

    assert payload['status'] == 'recovered'
    assert payload['block_reason'] == 'provider_prompt_input_stuck'
    assert payload['cancelled_old']['job_id'] == stuck
    assert payload['retried_job']['agent_name'] == 'agent3'
    assert dispatcher.get(stuck).status is JobStatus.CANCELLED


def test_comms_recover_rejects_unknown_running_hint(tmp_path: Path) -> None:
    ctx, layout, _config, _registry, dispatcher = _dispatcher(tmp_path / 'repo-running-unknown-hint')
    job_id = _submit(dispatcher, ctx, sender='agent2', target='agent1')
    dispatcher.tick()

    payload = dispatcher.comms_recover({'job_id': job_id, 'block_reason': 'provider_idle_untrusted'})

    assert payload['status'] == 'noop'
    assert payload['noop_reason'] == 'not_recoverable'
    assert dispatcher.get(job_id).status is JobStatus.RUNNING
    attempts = AttemptStore(layout).list_message(MessageStore(layout).list_all()[-1].message_id)
    assert {attempt.attempt_id for attempt in attempts} == {attempts[0].attempt_id}


def test_comms_recover_cancels_stale_running_and_starts_waiting_job(tmp_path: Path) -> None:
    ctx, layout, _config, registry, dispatcher = _dispatcher(tmp_path / 'repo-stale-running')
    stuck = _submit(dispatcher, ctx, sender='agent2', target='agent1', body='stuck')
    waiting_1 = _submit(dispatcher, ctx, sender='agent2', target='agent1', body='waiting 1')
    waiting_2 = _submit(dispatcher, ctx, sender='agent2', target='agent1', body='waiting 2')
    dispatcher.tick()
    registry.upsert(replace(registry.get('agent1'), state=AgentState.DEGRADED, health='pane-dead', pane_state='dead'))

    payload = dispatcher.comms_recover({'job_id': stuck})

    assert payload['status'] == 'recovered'
    assert payload['block_reason'] == 'pane_dead'
    assert payload['cancelled_old']['job_id'] == stuck
    assert payload['retried_job']['agent_name'] == 'agent1'
    assert [job['job_id'] for job in payload['next_started']] == [waiting_1]
    assert dispatcher.get(stuck).status is JobStatus.CANCELLED
    assert dispatcher.get(waiting_1).status is JobStatus.RUNNING
    assert dispatcher.get(waiting_2).status is JobStatus.QUEUED
    attempts = AttemptStore(layout).list_message(MessageStore(layout).list_all()[0].message_id)
    assert max(attempt.retry_index for attempt in attempts) == 1


def test_comms_recover_is_idempotent_after_retry(tmp_path: Path) -> None:
    ctx, layout, _config, registry, dispatcher = _dispatcher(tmp_path / 'repo-idempotent')
    job_id = _submit(dispatcher, ctx, sender='agent2', target='agent1')
    dispatcher.tick()
    registry.upsert(replace(registry.get('agent1'), state=AgentState.DEGRADED, health='pane-dead', pane_state='dead'))

    first = dispatcher.comms_recover({'job_id': job_id})
    second = dispatcher.comms_recover({'job_id': job_id})

    assert first['retried_job']['job_id'] == second['latest_job_id']
    assert second['status'] == 'noop'
    assert second['noop_reason'] == 'already_retried'
    attempts = AttemptStore(layout).list_message(MessageStore(layout).list_all()[-1].message_id)
    assert len({attempt.job_id for attempt in attempts}) == 2


def test_comms_recover_releases_only_targeted_mailbox_head(tmp_path: Path) -> None:
    ctx, layout, _config, _registry, dispatcher = _dispatcher(tmp_path / 'repo-mailbox-head')
    job_id = _submit(dispatcher, ctx, sender='agent2', target='agent1')
    dispatcher.tick()
    dispatcher.complete(job_id, _decision(CompletionStatus.INCOMPLETE, reason='manual_fail'))
    attempt = AttemptStore(layout).get_latest_by_job_id(job_id)
    inbound = InboundEventStore(layout).get_latest_for_attempt('agent1', attempt.attempt_id)
    assert inbound is not None
    dispatcher._message_bureau_control._inbound_store.append(replace(inbound, status=InboundEventStatus.DELIVERING, finished_at=None))
    dispatcher._message_bureau_control._lease_store.save(
        DeliveryLease(
            agent_name='agent1',
            inbound_event_id=inbound.inbound_event_id,
            lease_version=1,
            acquired_at=NOW,
            last_progress_at=NOW,
            expires_at=None,
            lease_state=LeaseState.ACQUIRED,
        )
    )
    dispatcher._message_bureau_control._mailbox_kernel.refresh_mailbox('agent1', updated_at=NOW)
    unrelated = _submit(dispatcher, ctx, sender='agent2', target='agent2', body='unrelated')

    payload = dispatcher.comms_recover({'job_id': job_id})

    assert payload['released_event']['inbound_event_id'] == inbound.inbound_event_id
    next_lease = dispatcher._message_bureau_control._lease_store.load('agent1')
    assert next_lease is not None
    assert next_lease.inbound_event_id != inbound.inbound_event_id
    assert dispatcher._message_bureau_control._inbound_store.get_latest('agent1', inbound.inbound_event_id).status is InboundEventStatus.ABANDONED
    assert dispatcher.get(unrelated).status is JobStatus.ACCEPTED
    mailbox = MailboxStore(layout).load('agent1')
    assert mailbox is not None
    assert mailbox.mailbox_state in {MailboxState.IDLE, MailboxState.BLOCKED, MailboxState.DELIVERING}


def test_comms_recover_reply_delivery_race_is_noop_after_delivery_completes(tmp_path: Path) -> None:
    ctx, layout, _config, _registry, dispatcher = _dispatcher(tmp_path / 'repo-reply-race')
    source = _submit(dispatcher, ctx, sender='agent2', target='agent1')
    dispatcher.tick()
    dispatcher.complete(source, _decision(CompletionStatus.COMPLETED, reply='OK'))
    dispatcher.tick()
    delivery = next(job for job in dispatcher._job_store.list_agent('agent2') if job.request.message_type == 'reply_delivery')
    dispatcher.complete(delivery.job_id, _decision(CompletionStatus.COMPLETED, reply='delivered'))

    payload = dispatcher.comms_recover({'job_id': source, 'reply_delivery_job_id': delivery.job_id})

    assert payload['status'] == 'noop'
    assert payload['noop_reason'] == 'not_recoverable'
    attempts = AttemptStore(layout).list_all()
    delivery_attempts = [attempt for attempt in attempts if attempt.job_id == delivery.job_id]
    assert len({attempt.attempt_id for attempt in delivery_attempts}) == 1
    assert dispatcher.inbox('agent2')['item_count'] == 0


def test_comms_recover_failed_reply_delivery_resets_reply_head_and_schedules_delivery(tmp_path: Path) -> None:
    ctx, layout, _config, _registry, dispatcher = _dispatcher(tmp_path / 'repo-reply-delivery-recover')
    source = _submit(dispatcher, ctx, sender='agent2', target='agent1')
    dispatcher.tick()
    dispatcher.complete(source, _decision(CompletionStatus.COMPLETED, reply='OK'))
    dispatcher.tick()
    delivery = next(job for job in dispatcher._job_store.list_agent('agent2') if job.request.message_type == 'reply_delivery')
    dispatcher.complete(delivery.job_id, _decision(CompletionStatus.FAILED, reason='pane_dead'))
    failed_head = dispatcher._message_bureau_control._mailbox_kernel.head_pending_event('agent2')
    assert failed_head is not None
    assert failed_head.event_type is InboundEventType.TASK_REPLY

    payload = dispatcher.comms_recover({'job_id': source, 'reply_delivery_job_id': delivery.job_id})

    assert payload['status'] == 'recovered'
    assert payload['retried_job']['job_id'] != delivery.job_id
    assert payload['retried_job']['request']['message_type'] == 'reply_delivery'
    assert payload['next_started'][0]['job_id'] == payload['retried_job']['job_id']
    assert AttemptStore(layout).get_latest_by_job_id(payload['retried_job']['job_id']) is not None
    retry_head = dispatcher._message_bureau_control._inbound_store.get_latest('agent2', failed_head.inbound_event_id)
    assert retry_head.status is InboundEventStatus.DELIVERING
    assert payload['recoverability_after']['recoverable'] is False


def test_comms_recover_failed_reply_delivery_is_idempotent_after_new_delivery_starts(tmp_path: Path) -> None:
    ctx, layout, _config, _registry, dispatcher = _dispatcher(tmp_path / 'repo-reply-delivery-idempotent')
    source = _submit(dispatcher, ctx, sender='agent2', target='agent1')
    dispatcher.tick()
    dispatcher.complete(source, _decision(CompletionStatus.COMPLETED, reply='OK'))
    dispatcher.tick()
    delivery = next(job for job in dispatcher._job_store.list_agent('agent2') if job.request.message_type == 'reply_delivery')
    dispatcher.complete(delivery.job_id, _decision(CompletionStatus.FAILED, reason='pane_dead'))

    first = dispatcher.comms_recover({'job_id': source, 'reply_delivery_job_id': delivery.job_id})
    second = dispatcher.comms_recover({'job_id': source, 'reply_delivery_job_id': delivery.job_id})

    assert first['retried_job']['job_id'] == second['latest_job_id']
    assert second['status'] == 'noop'
    assert second['noop_reason'] == 'already_retried'
    reply_delivery_jobs = [
        job
        for job in dispatcher._job_store.list_agent('agent2')
        if job.request.message_type == 'reply_delivery'
    ]
    assert len({job.job_id for job in reply_delivery_jobs}) == 2
    assert AttemptStore(layout).get_latest_by_job_id(first['retried_job']['job_id']).attempt_state is AttemptState.RUNNING


def test_project_view_marks_recoverable_and_clears_after_recovery(tmp_path: Path) -> None:
    ctx, layout, config, registry, dispatcher = _dispatcher(tmp_path / 'repo-project-view-recoverable')
    job_id = _submit(dispatcher, ctx, sender='agent2', target='agent1')
    dispatcher.tick()
    dispatcher.complete(job_id, _decision(CompletionStatus.INCOMPLETE, reason='manual_fail'))
    mount_manager = MountManager(layout, clock=lambda: NOW)
    mount_manager.mark_mounted(project_id=ctx.project_id, pid=123, socket_path=layout.ccbd_socket_path, generation=1, started_at=NOW)
    service = ProjectViewService(
        ProjectViewDependencies(
            project_root=ctx.project_root,
            project_id=ctx.project_id,
            config=config,
            registry=registry,
            mount_manager=mount_manager,
            namespace_state_store=ProjectNamespaceStateStore(layout),
            dispatcher=dispatcher,
            clock=lambda: NOW,
            cache_ttl_ms=0,
        )
    )

    before = service.build_response()['view']['comms'][0]
    dispatcher.comms_recover({'job_id': job_id})
    after = service.build_response()['view']['comms'][0]

    assert before['recoverable'] is True
    assert before['recover_target']['job_id'] == job_id
    assert before['block_reason'] == 'job_incomplete'
    assert after['id'] != job_id
    assert after['recoverable'] is False
    assert after['recover_target'] is None
