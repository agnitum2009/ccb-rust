from __future__ import annotations

from collections.abc import Mapping

from .common import render_tmux_cleanup_summaries
from .ops_views_common import binding_line


def render_config_validate(summary) -> tuple[str, ...]:
    lines = [
        'config_status: valid',
        f'project: {summary.project_root}',
        f'project_id: {summary.project_id}',
        f'config_source_kind: {summary.source_kind}',
        f'config_source: {summary.source or "<builtin>"}',
        f'used_builtin_default: {str(summary.used_builtin_default).lower()}',
        f'default_agents: {", ".join(summary.default_agents)}',
        f'agents: {", ".join(summary.agent_names)}',
        f'cmd_enabled: {str(summary.cmd_enabled).lower()}',
        f'layout: {summary.layout_spec}',
    ]
    lines.extend(f'config_warning: {warning}' for warning in getattr(summary, 'style_warnings', ()) or ())
    return tuple(lines)


def render_start(summary) -> tuple[str, ...]:
    lines = [
        'start_status: ok',
        f'project: {summary.project_root}',
        f'project_id: {summary.project_id}',
        f'ccbd_started: {str(summary.daemon_started).lower()}',
        f'socket_path: {summary.socket_path}',
        f'agents: {", ".join(summary.started)}',
    ]
    heartbeat = getattr(summary, 'maintenance_heartbeat', None)
    if isinstance(heartbeat, Mapping):
        details = [
            f'status={heartbeat.get("maintenance_status")}',
            f'action={heartbeat.get("action")}',
        ]
        if heartbeat.get('runner_status') is not None:
            details.append(f'runner_status={heartbeat.get("runner_status")}')
        if heartbeat.get('tick_status') is not None:
            details.append(f'tick_status={heartbeat.get("tick_status")}')
        lines.append(
            'maintenance_heartbeat: '
            + ' '.join(details)
        )
        reason = str(heartbeat.get('reason') or '').strip()
        if reason:
            lines.append(f'maintenance_heartbeat_reason: {reason}')
    lines.extend(render_tmux_cleanup_summaries(getattr(summary, 'cleanup_summaries', ()) or ()))
    return tuple(lines)


def render_logs(summary) -> tuple[str, ...]:
    lines = [
        'logs_status: ok',
        f'project_id: {summary.project_id}',
        f'agent_name: {summary.agent_name}',
        f'provider: {summary.provider}',
        f'runtime_ref: {summary.runtime_ref}',
        f'session_ref: {summary.session_ref}',
        f'log_count: {len(summary.entries)}',
    ]
    if not summary.entries:
        lines.append('log: <none>')
        return tuple(lines)
    for entry in summary.entries:
        lines.append(f'log: {entry.source} {entry.path}')
        for line in entry.lines:
            lines.append(f'log_line: {line}')
    return tuple(lines)


def render_doctor_bundle(summary) -> tuple[str, ...]:
    return (
        'doctor_bundle_status: ok',
        f'project: {summary.project_root}',
        f'project_id: {summary.project_id}',
        f'bundle_id: {summary.bundle_id}',
        f'bundle_path: {summary.bundle_path}',
        f'file_count: {summary.file_count}',
        f'included_count: {summary.included_count}',
        f'missing_count: {summary.missing_count}',
        f'truncated_count: {summary.truncated_count}',
        f'doctor_error: {summary.doctor_error}',
    )


def render_cleanup(summary) -> tuple[str, ...]:
    lines = [
        f'cleanup_status: {summary.status}',
        f'project_root: {summary.project_root}',
        f'project_id: {summary.project_id}',
        f'cleanup_deleted_bytes: {summary.deleted_bytes}',
        f'cleanup_deleted_count: {summary.deleted_count}',
        f'cleanup_skipped_count: {summary.skipped_count}',
    ]
    for action in getattr(summary, 'actions', ()) or ():
        lines.append(
            'cleanup_action: '
            f'provider={action.provider} '
            f'kind={action.kind} '
            f'bytes={action.bytes_removed} '
            f'reason={action.reason} '
            f'path={action.path}'
        )
    for skipped in getattr(summary, 'skipped', ()) or ():
        lines.append(
            'cleanup_skipped: '
            f'provider={skipped.provider} '
            f'reason={skipped.reason} '
            f'path={skipped.path}'
        )
    return tuple(lines)


def render_clear(summary) -> tuple[str, ...]:
    results = tuple(summary.get('results', ()) or ()) if isinstance(summary, Mapping) else ()
    cleared_count = sum(1 for item in results if item.get('status') == 'cleared')
    skipped_count = sum(1 for item in results if item.get('status') == 'skipped')
    failed_count = sum(1 for item in results if item.get('status') == 'failed')
    lines = [
        f'clear_status: {summary.get("status", "unknown") if isinstance(summary, Mapping) else "unknown"}',
        f'cleared_count: {cleared_count}',
        f'skipped_count: {skipped_count}',
        f'failed_count: {failed_count}',
    ]
    for item in results:
        agent = str(item.get('agent') or '')
        status = str(item.get('status') or '')
        pane_id = str(item.get('pane_id') or '')
        reason = str(item.get('reason') or '')
        detail = f'agent={agent} status={status}'
        if pane_id:
            detail += f' pane_id={pane_id}'
        if reason:
            detail += f' reason={reason}'
        lines.append(f'clear_agent: {detail}')
    return tuple(lines)


def render_restart(summary) -> tuple[str, ...]:
    payload = summary if isinstance(summary, Mapping) else {}
    status = str(payload.get('restart_status') or payload.get('status') or 'unknown')
    lines = [
        f'restart_status: {status}',
        f'agent_name: {payload.get("agent_name", "")}',
    ]
    restartable = tuple(str(item) for item in (payload.get('restartable_agents') or ()) if str(item))
    if restartable:
        lines.append(f'restartable_agents: {", ".join(restartable)}')
    reason = str(payload.get('reason') or '').strip()
    if reason:
        lines.append(f'reason: {reason}')
    busy_gate = payload.get('busy_gate')
    if isinstance(busy_gate, Mapping):
        lines.append(_restart_busy_gate_line(busy_gate))
    blockers = tuple(payload.get('blockers') or ())
    for blocker in blockers:
        if isinstance(blocker, Mapping):
            reason_text = str(blocker.get('reason') or '').strip()
            detail = str(blocker.get('detail') or '').strip()
            line = f'blocker: reason={reason_text}'
            if detail:
                line += f' detail={detail}'
            lines.append(line)
        else:
            lines.append(f'blocker: {blocker}')
    old_runtime = payload.get('old_runtime')
    if isinstance(old_runtime, Mapping):
        lines.append(f'old_runtime: {_runtime_evidence_text(old_runtime)}')
    new_runtime = payload.get('new_runtime')
    if isinstance(new_runtime, Mapping):
        lines.append(f'new_runtime: {_runtime_evidence_text(new_runtime)}')
    result = payload.get('result')
    if isinstance(result, Mapping):
        lines.append(f'restart_result: {_flat_mapping_text(result)}')
    error = str(payload.get('error') or '').strip()
    if error:
        lines.append(f'error: {error}')
    return tuple(lines)


def render_maintenance(payload) -> tuple[str, ...]:
    data = payload if isinstance(payload, Mapping) else {}
    status = str(data.get('maintenance_status') or 'unknown')
    lines = [f'maintenance_status: {status}']
    action = str(data.get('action') or '').strip()
    if action:
        lines.append(f'action: {action}')
    reason = str(data.get('reason') or '').strip()
    if reason:
        lines.append(f'reason: {reason}')
    if status == 'not_implemented':
        return tuple(lines)
    runner_status = str(data.get('runner_status') or '').strip()
    if runner_status:
        lines.extend(
            [
                f'runner_status: {runner_status}',
                f'runner_started: {_render_optional(data.get("runner_started"))}',
                f'runner_id: {_render_optional(data.get("runner_id"))}',
                f'runner_pid: {_render_optional(data.get("runner_pid"))}',
                f'runner_exit_reason: {_render_optional(data.get("runner_exit_reason"))}',
                f'runner_iterations: {_render_optional(data.get("runner_iterations"))}',
            ]
        )
    tick_status = str(data.get('tick_status') or '').strip()
    if tick_status:
        lines.extend(
            [
                f'tick_status: {tick_status}',
                f'tick_source_kind: {data.get("tick_source_kind")}',
                f'tick_recommended_action: {data.get("tick_recommended_action")}',
                f'tick_needs_user: {_render_value(data.get("tick_needs_user"))}',
                f'tick_next_heartbeat_after_s: {_render_optional(data.get("tick_next_heartbeat_after_s"))}',
                f'status_written: {_render_value(data.get("status_written"))}',
                f'schedule_written: {_render_value(data.get("schedule_written"))}',
                f'activation_written: {_render_value(data.get("activation_written"))}',
                f'tick_activation_status: {_render_optional(data.get("tick_activation_status"))}',
                f'tick_activation_id: {_render_optional(data.get("tick_activation_id"))}',
                f'tick_activation_job_id: {_render_optional(data.get("tick_activation_job_id"))}',
            ]
        )
        summary = data.get('tick_summary')
        if isinstance(summary, Mapping):
            lines.extend(_maintenance_summary_lines('tick_summary', summary))
        evidence = data.get('tick_evidence')
        if isinstance(evidence, (list, tuple)):
            lines.append(f'tick_evidence_count: {len(evidence)}')
            for item in evidence[:5]:
                if isinstance(item, Mapping):
                    lines.append(_maintenance_evidence_line('tick_evidence', item))

    lines.extend(
        [
            f'project: {data.get("project")}',
            f'project_id: {data.get("project_id")}',
            f'config_source_kind: {data.get("config_source_kind")}',
            f'config_source: {data.get("config_source") or "<builtin>"}',
            f'heartbeat_enabled: {_render_value(data.get("enabled"))}',
            f'heartbeat_assessor: {data.get("assessor")}',
            f'heartbeat_assessor_present: {_render_value(data.get("assessor_present"))}',
            f'heartbeat_interval_s: {data.get("interval_s")}',
            f'heartbeat_min_interval_s: {data.get("min_interval_s")}',
            f'heartbeat_unknown_streak_cap: {data.get("unknown_streak_cap")}',
            f'heartbeat_escalation_policy: {data.get("escalation_policy")}',
            f'heartbeat_startup_ensure: {_render_value(data.get("startup_ensure"))}',
        ]
    )
    schedule = data.get('schedule')
    if isinstance(schedule, Mapping):
        lines.extend(_maintenance_record_lines('schedule', schedule))
    last_status = data.get('last_status')
    if isinstance(last_status, Mapping):
        lines.extend(_maintenance_record_lines('last_status', last_status))
    runner = data.get('runner')
    if isinstance(runner, Mapping):
        lines.extend(_maintenance_record_lines('runner', runner))
    last_activation = data.get('last_activation')
    if isinstance(last_activation, Mapping):
        lines.extend(_maintenance_record_lines('last_activation', last_activation))
    return tuple(lines)


def _maintenance_record_lines(prefix: str, payload: Mapping[str, object]) -> list[str]:
    lines = [
        f'{prefix}_state: {payload.get("state")}',
        f'{prefix}_path: {payload.get("path")}',
    ]
    error = str(payload.get('error') or '').strip()
    if error:
        lines.append(f'{prefix}_error: {error}')
    record = payload.get('record')
    if isinstance(record, Mapping):
        for key in (
            'next_run_at',
            'reason',
            'updated_at',
            'updated_by',
            'last_tick_status',
            'last_tick_at',
            'last_ok_at',
            'last_error',
            'unknown_streak',
            'source_kind',
            'recommended_action',
            'next_heartbeat_after_s',
            'needs_user',
            'last_activation_status',
            'last_activation_id',
            'last_activation_job_id',
            'last_activation_target',
            'last_activation_dedup_key',
            'runner_id',
            'pid',
            'state',
            'started_at',
            'last_seen_at',
            'last_wake_at',
            'last_tick_at',
            'last_tick_status',
            'observed_next_run_at',
            'sleep_until',
            'exit_reason',
            'activation_id',
            'status',
            'condition_kind',
            'trigger_kind',
            'source',
            'observed_at',
            'target_agent',
            'delivery_mode',
            'payload_kind',
            'dedup_key',
            'job_id',
            'submitted_at',
            'suppressed_reason',
            'repeat_count',
        ):
            if key in record:
                lines.append(f'{prefix}_{key}: {_render_value(record.get(key))}')
        summary = record.get('summary')
        if isinstance(summary, Mapping):
            lines.extend(_maintenance_summary_lines(f'{prefix}_summary', summary))
        evidence = record.get('evidence')
        if isinstance(evidence, (list, tuple)):
            lines.append(f'{prefix}_evidence_count: {len(evidence)}')
    return lines


def _maintenance_summary_lines(prefix: str, payload: Mapping[str, object]) -> list[str]:
    lines: list[str] = []
    for key in (
        'source_kind',
        'ccbd_state',
        'agent_count',
        'active_agent_count',
        'pending_agent_count',
        'idle_agent_count',
        'offline_agent_count',
        'failed_agent_count',
        'concern_agent_count',
        'unknown_agent_count',
        'comms_count',
        'active_comms_count',
        'concern_comms_count',
        'failing_comms_count',
        'suspicion_count',
        'fallback_error',
    ):
        if key in payload:
            lines.append(f'{prefix}_{key}: {_render_value(payload.get(key))}')
    return lines


def _maintenance_evidence_line(prefix: str, payload: Mapping[str, object]) -> str:
    parts = [f'{prefix}:']
    for key in (
        'health',
        'kind',
        'condition_kind',
        'agent',
        'job_id',
        'target',
        'reason',
        'source',
        'status',
        'ccbd_state',
        'confidence',
    ):
        value = payload.get(key)
        if value is not None and value != '':
            parts.append(f'{key}={value}')
    return ' '.join(parts)


def _render_optional(value: object) -> str:
    if value is None:
        return '<none>'
    return _render_value(value)


def _restart_busy_gate_line(gate: Mapping[str, object]) -> str:
    fields = {
        'passed': str(bool(gate.get('passed'))).lower(),
        'runtime_state': gate.get('runtime_state'),
        'runtime_queue_depth': gate.get('runtime_queue_depth'),
        'queue_depth': gate.get('queue_depth'),
        'pending_reply_count': gate.get('pending_reply_count'),
        'active_job_id': gate.get('active_job_id'),
        'active_inbound_event_id': gate.get('active_inbound_event_id'),
        'pending_callback_count': gate.get('pending_callback_count'),
    }
    return 'restart_busy_gate: ' + _flat_mapping_text(fields)


def _runtime_evidence_text(evidence: Mapping[str, object]) -> str:
    fields = {
        'state': evidence.get('state'),
        'health': evidence.get('health'),
        'pane_id': evidence.get('pane_id'),
        'active_pane_id': evidence.get('active_pane_id'),
        'runtime_ref': evidence.get('runtime_ref'),
        'session_ref': evidence.get('session_ref'),
        'runtime_pid': evidence.get('runtime_pid'),
        'restart_count': evidence.get('restart_count'),
    }
    return _flat_mapping_text(fields)


def _flat_mapping_text(payload: Mapping[str, object]) -> str:
    return ' '.join(f'{key}={_render_value(value)}' for key, value in payload.items())


def _render_value(value: object) -> str:
    if value is None:
        return 'None'
    if isinstance(value, bool):
        return str(value).lower()
    if isinstance(value, (list, tuple)):
        return ','.join(str(item) for item in value)
    return str(value).replace('\n', '\\n')


def render_kill(summary) -> tuple[str, ...]:
    lines = [
        'kill_status: ok',
        f'project_id: {summary.project_id}',
        f'state: {summary.state}',
        f'socket_path: {summary.socket_path}',
        f'forced: {str(summary.forced).lower()}',
    ]
    lines.extend(render_tmux_cleanup_summaries(getattr(summary, 'cleanup_summaries', ()) or ()))
    return tuple(lines)


def render_ps(payload: Mapping[str, object]) -> tuple[str, ...]:
    lines = [
        f'project_id: {payload["project_id"]}',
        f'ccbd_state: {payload["ccbd_state"]}',
    ]
    for agent in payload['agents']:
        lines.append(
            f'agent: name={agent["agent_name"]} state={agent["state"]} provider={agent["provider"]} queue={agent["queue_depth"]}'
        )
        lines.append(binding_line(agent))
    return tuple(lines)


__all__ = [
    'render_clear',
    'render_cleanup',
    'render_config_validate',
    'render_doctor_bundle',
    'render_kill',
    'render_logs',
    'render_maintenance',
    'render_ps',
    'render_restart',
    'render_start',
]
