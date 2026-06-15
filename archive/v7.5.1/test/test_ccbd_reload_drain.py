from __future__ import annotations

from pathlib import Path

import pytest

from ccbd.app import CcbdApp
from ccbd.reload_drain import (
    DrainBounds,
    DrainIntent,
    DrainQueue,
    DrainQueueStore,
    drain_intent_suggestions_for_reload_operations,
    plan_drain_transition,
    retire_record,
)
from ccbd.reload_plan import build_reload_dry_run_plan
from agents.config_loader import load_project_config
from cli.parser import CliParser
from cli.render import render_reload


BASE_CONFIG = """version = 2
entry_window = "main"

[windows]
main = "agent1:codex, agent2:claude"
"""


def test_drain_idle_immediately_ready() -> None:
    queue = DrainQueue.empty(DrainBounds(max_pending=2, timeout_s=10.0, max_age_s=60.0))
    result = queue.enqueue(_intent('agent1', created_at_s=0.0), now_s=0.0)

    updated = plan_drain_transition(result.record, now_s=1.0, is_busy=lambda _record: False)

    assert result.accepted is True
    assert updated.phase == 'retiring'
    assert updated.status == 'idle_ready'
    assert updated.busy is False
    assert updated.terminal is False
    assert result.queue.replace_record(updated).blocks_new_work_for('agent1') is True


def test_drain_busy_waits_until_bound() -> None:
    record = DrainQueue.empty(DrainBounds(max_pending=2, timeout_s=10.0, max_age_s=60.0)).enqueue(
        _intent('agent1', created_at_s=0.0),
        now_s=0.0,
    ).record

    updated = plan_drain_transition(record, now_s=5.0, is_busy=lambda _record: True)

    assert updated.phase == 'draining'
    assert updated.status == 'waiting'
    assert updated.busy is True
    assert updated.terminal is False


def test_drain_busy_timeout() -> None:
    record = DrainQueue.empty(DrainBounds(max_pending=2, timeout_s=10.0, max_age_s=60.0)).enqueue(
        _intent('agent1', created_at_s=0.0),
        now_s=0.0,
    ).record
    calls = 0

    def _busy(_record):
        nonlocal calls
        calls += 1
        return True

    updated = plan_drain_transition(record, now_s=10.0, is_busy=_busy)

    assert updated.status == 'timed_out'
    assert updated.reason == 'drain timeout_s exceeded'
    assert updated.terminal is True
    assert calls == 0


def test_drain_queue_full_rejects_without_appending() -> None:
    bounds = DrainBounds(max_pending=1, timeout_s=10.0, max_age_s=60.0)
    first = DrainQueue.empty(bounds).enqueue(_intent('agent1', created_at_s=0.0), now_s=0.0)
    second = first.queue.enqueue(_intent('agent2', created_at_s=1.0), now_s=1.0)

    assert first.accepted is True
    assert second.accepted is False
    assert second.record.status == 'rejected_queue_full'
    assert second.record.phase == 'rejected'
    assert len(second.queue.records) == 1


def test_drain_age_bound_rejects_stale_intent() -> None:
    bounds = DrainBounds(max_pending=2, timeout_s=10.0, max_age_s=5.0)
    result = DrainQueue.empty(bounds).enqueue(_intent('agent1', created_at_s=0.0), now_s=6.0)

    assert result.accepted is False
    assert result.record.status == 'timed_out'
    assert result.record.reason == 'intent age exceeds max_age_s'
    assert result.queue.records == ()


def test_drain_retired_terminal_state_is_stable() -> None:
    record = DrainQueue.empty().enqueue(_intent('agent1', created_at_s=0.0), now_s=0.0).record
    ready = plan_drain_transition(record, now_s=1.0, is_busy=lambda _record: False)
    retired = retire_record(ready, now_s=2.0)

    assert retired.phase == 'retired'
    assert retired.status == 'retired'
    assert retired.terminal is True
    assert plan_drain_transition(retired, now_s=3.0, is_busy=lambda _record: False) is retired
    assert retire_record(retired, now_s=4.0) is retired


def test_drain_retire_only_accepts_idle_ready_records() -> None:
    record = DrainQueue.empty().enqueue(_intent('agent1', created_at_s=0.0), now_s=0.0).record
    waiting = plan_drain_transition(record, now_s=1.0, is_busy=lambda _record: True)

    assert retire_record(record, now_s=2.0) is record
    assert retire_record(waiting, now_s=2.0) is waiting


def test_drain_replace_and_unload_intents_share_queue_bound() -> None:
    bounds = DrainBounds(max_pending=2, timeout_s=10.0, max_age_s=60.0)
    queue = DrainQueue.empty(bounds)
    first = queue.enqueue(_intent('agent1', kind='unload', created_at_s=0.0), now_s=0.0)
    second = first.queue.enqueue(_intent('agent1', kind='replace', created_at_s=1.0), now_s=1.0)
    third = second.queue.enqueue(_intent('agent2', kind='replace', created_at_s=2.0), now_s=2.0)

    assert first.accepted is True
    assert second.accepted is True
    assert third.accepted is False
    assert [record.intent.intent_kind for record in second.queue.records] == ['unload', 'replace']
    assert second.queue.blocks_new_work_for('agent1') is True
    assert second.queue.blocks_new_work_for('agent2') is False


def test_drain_replace_record_does_not_append_missing_records() -> None:
    bounds = DrainBounds(max_pending=1, timeout_s=10.0, max_age_s=60.0)
    queue = DrainQueue.empty(bounds)
    first = queue.enqueue(_intent('agent1', created_at_s=0.0), now_s=0.0)
    missing = DrainQueue.empty(bounds).enqueue(_intent('agent2', created_at_s=1.0), now_s=1.0).record

    updated = first.queue.replace_record(missing)

    assert updated is first.queue
    assert [record.intent.agent_name for record in updated.records] == ['agent1']


def test_drain_queue_store_round_trips_to_bounded_ccbd_state(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-store', BASE_CONFIG)
    app = CcbdApp(project_root, clock=lambda: '2026-05-29T00:00:00Z', pid=4242)
    store = DrainQueueStore(app.paths, bounds=DrainBounds(max_pending=2, timeout_s=10.0, max_age_s=60.0))
    queue = DrainQueue.empty(store.load().bounds)
    accepted = queue.enqueue(_intent('agent1', created_at_s=0.0), now_s=0.0)

    store.save(accepted.queue)
    loaded = store.load()

    assert app.paths.ccbd_reload_drain_path.name == 'reload-drain.json'
    assert app.paths.ccbd_reload_drain_path.exists()
    assert loaded.bounds == accepted.queue.bounds
    assert loaded.records[0].intent.agent_name == 'agent1'
    assert loaded.records[0].status == 'pending'


def test_drain_state_machine_uses_injectable_busy_predicate_and_no_app_mutation(
    tmp_path: Path,
    monkeypatch,
) -> None:
    project_root = _project(tmp_path / 'repo-no-mutation', BASE_CONFIG)
    app = CcbdApp(project_root, clock=lambda: '2026-05-29T00:00:00Z', pid=4242)
    before_graph = app.service_graph
    calls: list[str] = []
    record = DrainQueue.empty().enqueue(_intent('agent1', created_at_s=0.0), now_s=0.0).record

    def _busy(current_record):
        calls.append(current_record.intent.agent_name)
        return False

    def _fail(*_args, **_kwargs):
        raise AssertionError('drain state machine must not mutate app runtime, namespace, graph, or tmux')

    monkeypatch.setattr(app, 'publish_service_graph', _fail, raising=False)
    monkeypatch.setattr(app.runtime_service, 'mutate_runtime_authority', _fail, raising=False)
    monkeypatch.setattr(app.runtime_service, 'patch_runtime_state', _fail, raising=False)
    for method_name in ('ensure_started', 'destroy', 'recreate', 'patch_topology', 'refresh'):
        monkeypatch.setattr(app.project_namespace, method_name, _fail, raising=False)
    updated = plan_drain_transition(record, now_s=1.0, is_busy=_busy)
    retired = retire_record(updated, now_s=2.0)

    assert calls == ['agent1']
    assert retired.status == 'retired'
    assert app.service_graph is before_graph


def test_reload_plan_includes_dry_run_drain_intents_for_remove_and_replace(tmp_path: Path) -> None:
    current = _load_config(tmp_path / 'current', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new',
        """version = 2
entry_window = "main"

[windows]
main = "agent1:claude"
""",
    )

    plan = build_reload_dry_run_plan(current, new)

    assert plan['plan_class'] == 'replace_agent'
    assert {(item['intent_kind'], item['agent'], item['initial_phase']) for item in plan['drain_intents']} == {
        ('unload', 'agent2', 'pending_unload'),
        ('replace', 'agent1', 'pending_replace'),
    }
    assert {item['dry_run_only'] for item in plan['drain_intents']} == {True}
    assert plan['safe_to_apply'] is False
    assert plan['mutation_enabled'] is False


def test_reload_non_dry_run_no_change_noops_and_parser_accepts_explicit_apply(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-block-no-change', BASE_CONFIG)
    app = CcbdApp(project_root, clock=lambda: '2026-05-29T00:00:00Z', pid=4242)
    old_graph = app.service_graph

    payload = app.socket_server._handlers['project_reload_config']({'dry_run': False})

    assert payload['status'] == 'noop'
    assert payload['stage'] == 'no_op'
    assert payload['plan_class'] == 'no_change'
    assert payload['diagnostics']['reason'] == 'no_change'
    assert payload['diagnostics']['graph_published'] is False
    assert payload['diagnostics']['reason'] == 'no_change'
    assert app.service_graph is old_graph
    assert CliParser().parse(['reload']).dry_run is False


def test_drain_intent_suggestions_are_stable_and_do_not_cover_additive_ops() -> None:
    operations = [
        {'op': 'add_agent', 'agent': 'agent3'},
        {'op': 'remove_agent', 'agent': 'agent2', 'reason': 'removed'},
        {'op': 'replace_agent', 'agent': 'agent1', 'reason': 'provider changed'},
    ]

    first = drain_intent_suggestions_for_reload_operations(
        operations,
        old_config_signature='old',
        new_config_signature='new',
    )
    second = drain_intent_suggestions_for_reload_operations(
        operations,
        old_config_signature='old',
        new_config_signature='new',
    )

    assert first == second
    assert [item['intent_kind'] for item in first] == ['unload', 'replace']


def test_reload_render_includes_drain_intent_suggestions() -> None:
    lines = render_reload(
        {
            'status': 'ok',
            'dry_run': True,
            'mutation_enabled': False,
            'plan_class': 'remove_agent',
            'safe_to_apply': False,
            'future_safe_to_apply': False,
            'old_config_signature': 'old',
            'new_config_signature': 'new',
            'operations': [{'op': 'remove_agent', 'agent': 'agent2', 'reason': 'removed'}],
            'drain_intents': [
                {
                    'intent_kind': 'unload',
                    'agent': 'agent2',
                    'initial_phase': 'pending_unload',
                    'dry_run_only': True,
                    'reason': 'removed',
                }
            ],
            'reasons': [],
            'warnings': [],
            'errors': [],
        }
    )

    assert (
        'reload_drain_intent: intent_kind=unload agent=agent2 initial_phase=pending_unload '
        'dry_run_only=true reason=removed'
    ) in lines


def _intent(agent_name: str, *, kind: str = 'unload', created_at_s: float) -> DrainIntent:
    return DrainIntent(
        intent_id=f'{kind}-{agent_name}-{created_at_s}',
        intent_kind=kind,
        agent_name=agent_name,
        created_at_s=created_at_s,
        reason='test',
    )


def _project(project_root: Path, config_text: str) -> Path:
    project_root.mkdir(parents=True, exist_ok=True)
    config_path = project_root / '.ccb' / 'ccb.config'
    config_path.parent.mkdir(parents=True, exist_ok=True)
    config_path.write_text(config_text, encoding='utf-8')
    return project_root


def _load_config(project_root: Path, config_text: str):
    return load_project_config(_project(project_root, config_text)).config
