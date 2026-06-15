from __future__ import annotations

from copy import deepcopy
from typing import Any

from agents.config_identity import project_config_identity_payload
from ccbd.reload_drain import drain_intent_suggestions_for_reload_operations
from ccbd.reload_patch import build_invalid_namespace_patch_plan, build_namespace_patch_plan


_PLAN_PRIORITY = {
    'no_change': 0,
    'view_only_change': 10,
    'maintenance_change': 20,
    'add_tool_window': 30,
    'add_agent': 40,
    'add_window': 50,
    'remove_tool_window': 55,
    'change_tool_window': 65,
    'layout_change': 60,
    'move_agent': 70,
    'remove_agent': 80,
    'replace_agent': 90,
}


def build_reload_dry_run_plan(
    current_config,
    new_config,
    *,
    current_config_identity: dict[str, object] | None = None,
    project_id: str | None = None,
    current_namespace=None,
) -> dict[str, object]:
    old_identity = dict(current_config_identity or project_config_identity_payload(current_config))
    new_identity = project_config_identity_payload(new_config)
    warnings: list[str] = []

    if old_identity.get('config_signature') == new_identity.get('config_signature'):
        return _identity_preserving_plan(
            current_config,
            new_config,
            old_identity=old_identity,
            new_identity=new_identity,
            warnings=warnings,
            project_id=project_id,
            current_namespace=current_namespace,
        )

    operations = _build_operations(current_config, new_config)
    if not operations:
        operations.append(
            {
                'op': 'layout_change',
                'change': 'unclassified_identity_change',
                'reason': 'config identity changed but no narrower Phase 3 operation was detected',
            }
        )
        warnings.append('Config identity changed; Phase 3 degraded this diff to layout_change.')

    if any(item.get('op') == 'replace_agent' for item in operations):
        warnings.append(
            'Existing agent spec changes are conservatively classified as replace_agent; '
            'runtime-only and metadata-only fields are not split in Phase 3.'
        )

    plan_class = _select_plan_class(operations)
    drain_intents = drain_intent_suggestions_for_reload_operations(
        operations,
        old_config_signature=old_identity.get('config_signature'),
        new_config_signature=new_identity.get('config_signature'),
    )
    namespace_patch_plan = build_namespace_patch_plan(
        current_config,
        new_config,
        operations,
        project_id=project_id,
        current_namespace=current_namespace,
    )
    return _plan_payload(
        status='ok',
        plan_class=plan_class,
        old_identity=old_identity,
        new_identity=new_identity,
        operations=operations,
        drain_intents=drain_intents,
        namespace_patch_plan=namespace_patch_plan,
        reasons=_operation_reasons(operations),
        warnings=warnings,
        errors=[],
        future_safe_to_apply=_future_safe_to_apply(plan_class, operations),
    )


def build_invalid_reload_dry_run_plan(
    current_config,
    error: object,
    *,
    current_config_identity: dict[str, object] | None = None,
) -> dict[str, object]:
    old_identity = dict(current_config_identity or project_config_identity_payload(current_config))
    return _plan_payload(
        status='invalid_config',
        plan_class='invalid_config',
        old_identity=old_identity,
        new_identity={'known_agents': (), 'config_signature': None},
        operations=[],
        drain_intents=[],
        namespace_patch_plan=build_invalid_namespace_patch_plan(error),
        reasons=['new config could not be loaded or validated'],
        warnings=[],
        errors=[str(error)],
        future_safe_to_apply=False,
    )


def _identity_preserving_plan(
    current_config,
    new_config,
    *,
    old_identity: dict[str, object],
    new_identity: dict[str, object],
    warnings: list[str],
    project_id: str | None = None,
    current_namespace=None,
) -> dict[str, object]:
    old_full = _canonical_config_record(current_config, include_sidebar_view=True)
    new_full = _canonical_config_record(new_config, include_sidebar_view=True)
    if old_full == new_full:
        return _plan_payload(
            status='ok',
            plan_class='no_change',
            old_identity=old_identity,
            new_identity=new_identity,
            operations=[],
            drain_intents=[],
            namespace_patch_plan=build_namespace_patch_plan(
                current_config,
                new_config,
                [],
                project_id=project_id,
                current_namespace=current_namespace,
            ),
            reasons=['config identity and presentation fields are unchanged'],
            warnings=warnings,
            errors=[],
            future_safe_to_apply=True,
        )

    old_without_view = _canonical_config_record(current_config, include_sidebar_view=False)
    new_without_view = _canonical_config_record(new_config, include_sidebar_view=False)
    operations = [
        {
            'op': 'view_only_change',
            'field': 'sidebar_view',
            'reason': 'config identity is unchanged; only presentation fields affect the diff',
        }
    ]
    if old_without_view != new_without_view:
        warnings.append(
            'Config identity is unchanged but non-sidebar presentation fields could not be split more narrowly.'
        )
    return _plan_payload(
        status='ok',
        plan_class='view_only_change',
        old_identity=old_identity,
        new_identity=new_identity,
        operations=operations,
        drain_intents=[],
        namespace_patch_plan=build_namespace_patch_plan(
            current_config,
            new_config,
            operations,
            project_id=project_id,
            current_namespace=current_namespace,
        ),
        reasons=['config identity is unchanged; presentation-only fields changed'],
        warnings=warnings,
        errors=[],
        future_safe_to_apply=True,
    )


def _build_operations(current_config, new_config) -> list[dict[str, object]]:
    operations: list[dict[str, object]] = []
    old_agents = set(getattr(current_config, 'agents', {}) or {})
    new_agents = set(getattr(new_config, 'agents', {}) or {})
    old_window_by_agent = _agent_window_map(current_config)
    new_window_by_agent = _agent_window_map(new_config)
    old_windows = _window_record_map(current_config)
    new_windows = _window_record_map(new_config)
    old_tools = _tool_window_record_map(current_config)
    new_tools = _tool_window_record_map(new_config)
    added_windows = set(new_windows) - set(old_windows)

    for window_name in _ordered_windows(new_config, added_windows):
        record = new_windows[window_name]
        operations.append(
            {
                'op': 'add_window',
                'window': window_name,
                'agents': list(record.get('agent_names') or ()),
                'reason': 'window exists only in new config',
            }
        )

    for agent_name in sorted(new_agents - old_agents):
        operations.append(
            {
                'op': 'add_agent',
                'agent': agent_name,
                'window': new_window_by_agent.get(agent_name),
                'reason': 'agent exists only in new config',
            }
        )

    for agent_name in sorted(old_agents - new_agents):
        operations.append(
            {
                'op': 'remove_agent',
                'agent': agent_name,
                'window': old_window_by_agent.get(agent_name),
                'reason': 'agent exists only in current published config',
            }
        )

    for agent_name in sorted(old_agents & new_agents):
        old_window = old_window_by_agent.get(agent_name)
        new_window = new_window_by_agent.get(agent_name)
        if old_window != new_window:
            operations.append(
                {
                    'op': 'move_agent',
                    'agent': agent_name,
                    'from_window': old_window,
                    'to_window': new_window,
                    'reason': 'existing agent window membership changed',
                }
            )

    for agent_name in sorted(old_agents & new_agents):
        old_record = _agent_record(current_config.agents[agent_name])
        new_record = _agent_record(new_config.agents[agent_name])
        if old_record != new_record:
            operations.append(
                {
                    'op': 'replace_agent',
                    'agent': agent_name,
                    'fields': _changed_fields(old_record, new_record),
                    'reason': 'existing agent spec changed',
                }
            )

    operations.extend(_topology_operations(current_config, new_config, old_windows=old_windows, new_windows=new_windows))
    operations.extend(_tool_window_operations(current_config, new_config, old_tools=old_tools, new_tools=new_tools))
    operations.extend(_maintenance_operations(current_config, new_config))
    return operations


def _topology_operations(current_config, new_config, *, old_windows: dict[str, dict], new_windows: dict[str, dict]) -> list[dict[str, object]]:
    operations: list[dict[str, object]] = []
    for window_name in _ordered_windows(current_config, set(old_windows) - set(new_windows)):
        operations.append(
            {
                'op': 'layout_change',
                'window': window_name,
                'change': 'remove_window',
                'reason': 'window exists only in current published config',
            }
        )

    for window_name in _ordered_windows(new_config, set(old_windows) & set(new_windows)):
        old_record = old_windows[window_name]
        new_record = new_windows[window_name]
        old_agents = tuple(old_record.get('agent_names') or ())
        new_agents = tuple(new_record.get('agent_names') or ())
        if set(old_agents) != set(new_agents):
            continue
        changed = [
            field
            for field in ('order', 'layout_spec', 'agent_names')
            if old_record.get(field) != new_record.get(field)
        ]
        if changed:
            operations.append(
                {
                    'op': 'layout_change',
                    'window': window_name,
                    'fields': changed,
                    'reason': 'existing window layout changed without adding or removing agents',
                }
            )

    if getattr(current_config, 'entry_window', None) != getattr(new_config, 'entry_window', None):
        operations.append(
            {
                'op': 'layout_change',
                'field': 'entry_window',
                'old': getattr(current_config, 'entry_window', None),
                'new': getattr(new_config, 'entry_window', None),
                'reason': 'entry window changed',
            }
        )

    old_sidebar = _record_or_none(getattr(current_config, 'sidebar', None))
    new_sidebar = _record_or_none(getattr(new_config, 'sidebar', None))
    if old_sidebar != new_sidebar:
        operations.append(
            {
                'op': 'layout_change',
                'field': 'sidebar',
                'reason': 'sidebar topology changed',
            }
        )
    return operations


def _select_plan_class(operations: list[dict[str, object]]) -> str:
    if not operations:
        return 'no_change'
    return max((str(item.get('op') or 'layout_change') for item in operations), key=lambda op: _PLAN_PRIORITY.get(op, 60))


def _future_safe_to_apply(plan_class: str, operations: list[dict[str, object]]) -> bool:
    if plan_class in {'no_change', 'view_only_change', 'maintenance_change'}:
        return True
    if plan_class in {'add_tool_window', 'remove_tool_window'}:
        unsafe_tool_ops = {'change_tool_window'}
        unsafe_agent_ops = {'replace_agent', 'move_agent'}
        return not any(str(item.get('op') or '') in unsafe_tool_ops | unsafe_agent_ops for item in operations)
    if plan_class == 'remove_agent':
        return _remove_agent_operations_are_safe(operations)
    unsafe_ops = {'replace_agent', 'move_agent', 'layout_change', 'change_tool_window'}
    if any(str(item.get('op') or '') in unsafe_ops for item in operations):
        return False
    return plan_class in {'add_agent', 'add_window'}


def _remove_agent_operations_are_safe(operations: list[dict[str, object]]) -> bool:
    if not operations:
        return False
    for item in operations:
        op = str(item.get('op') or '')
        if op == 'remove_agent':
            continue
        if op == 'layout_change' and str(item.get('change') or '') == 'remove_window':
            continue
        return False
    return any(str(item.get('op') or '') == 'remove_agent' for item in operations)


def _tool_window_operations(
    current_config,
    new_config,
    *,
    old_tools: dict[str, dict[str, object]],
    new_tools: dict[str, dict[str, object]],
) -> list[dict[str, object]]:
    operations: list[dict[str, object]] = []
    for window_name in _ordered_tool_windows(new_config, set(new_tools) - set(old_tools)):
        record = new_tools[window_name]
        operations.append(
            {
                'op': 'add_tool_window',
                'window': window_name,
                'command': record.get('command'),
                'reason': 'tool window exists only in new config',
            }
        )
    for window_name in _ordered_tool_windows(current_config, set(old_tools) - set(new_tools)):
        operations.append(
            {
                'op': 'remove_tool_window',
                'window': window_name,
                'reason': 'tool window exists only in current published config',
            }
        )
    for window_name in _ordered_tool_windows(new_config, set(old_tools) & set(new_tools)):
        old_record = old_tools[window_name]
        new_record = new_tools[window_name]
        changed = [
            field
            for field in ('command',)
            if old_record.get(field) != new_record.get(field)
        ]
        if changed:
            operations.append(
                {
                    'op': 'change_tool_window',
                    'window': window_name,
                    'fields': changed,
                    'reason': 'existing tool window changed; explicit restart policy is not implemented',
                }
            )
    return operations


def _maintenance_operations(current_config, new_config) -> list[dict[str, object]]:
    old_record = _record_or_none(getattr(current_config, 'maintenance_heartbeat', None))
    new_record = _record_or_none(getattr(new_config, 'maintenance_heartbeat', None))
    if old_record == new_record:
        return []
    return [
        {
            'op': 'maintenance_change',
            'fields': _changed_fields(old_record or {}, new_record or {}),
            'reason': 'maintenance heartbeat policy changed',
        }
    ]


def _plan_payload(
    *,
    status: str,
    plan_class: str,
    old_identity: dict[str, object],
    new_identity: dict[str, object],
    operations: list[dict[str, object]],
    drain_intents: list[dict[str, object]],
    namespace_patch_plan: dict[str, object],
    reasons: list[str],
    warnings: list[str],
    errors: list[str],
    future_safe_to_apply: bool,
) -> dict[str, object]:
    return {
        'status': status,
        'dry_run': True,
        'mutation_enabled': False,
        'safe_to_apply': False,
        'future_safe_to_apply': bool(future_safe_to_apply),
        'plan_class': plan_class,
        'old_config_signature': old_identity.get('config_signature'),
        'new_config_signature': new_identity.get('config_signature'),
        'old_known_agents': list(old_identity.get('known_agents') or ()),
        'new_known_agents': list(new_identity.get('known_agents') or ()),
        'operations': operations,
        'drain_intents': drain_intents,
        'namespace_patch_plan': namespace_patch_plan,
        'reasons': reasons,
        'warnings': warnings,
        'errors': errors,
    }


def _operation_reasons(operations: list[dict[str, object]]) -> list[str]:
    reasons: list[str] = []
    for item in operations:
        reason = str(item.get('reason') or '').strip()
        op = str(item.get('op') or '').strip()
        target = str(item.get('agent') or item.get('window') or item.get('field') or '').strip()
        if reason and target:
            reasons.append(f'{op} {target}: {reason}')
        elif reason:
            reasons.append(f'{op}: {reason}' if op else reason)
    return reasons


def _canonical_config_record(config, *, include_sidebar_view: bool) -> dict[str, Any]:
    record = deepcopy(config.to_record())
    for key in ('schema_version', 'record_type', 'source_path'):
        record.pop(key, None)
    if not include_sidebar_view:
        record.pop('sidebar_view', None)
    return record


def _agent_record(spec) -> dict[str, Any]:
    record = deepcopy(spec.to_record())
    for key in ('schema_version', 'record_type'):
        record.pop(key, None)
    return record


def _changed_fields(old_record: dict[str, Any], new_record: dict[str, Any]) -> list[str]:
    return sorted(key for key in set(old_record) | set(new_record) if old_record.get(key) != new_record.get(key))


def _agent_window_map(config) -> dict[str, str]:
    mapping: dict[str, str] = {}
    for window in tuple(getattr(config, 'windows', ()) or ()):
        for agent_name in tuple(getattr(window, 'agent_names', ()) or ()):
            mapping[str(agent_name)] = str(getattr(window, 'name', '') or '')
    return mapping


def _window_record_map(config) -> dict[str, dict[str, object]]:
    return {
        str(getattr(window, 'name', '') or ''): dict(window.to_record())
        for window in tuple(getattr(config, 'windows', ()) or ())
    }


def _tool_window_record_map(config) -> dict[str, dict[str, object]]:
    return {
        str(getattr(tool, 'name', '') or ''): dict(tool.to_record())
        for tool in tuple(getattr(config, 'tool_windows', ()) or ())
    }


def _ordered_windows(config, names: set[str]) -> list[str]:
    order = {
        str(getattr(window, 'name', '') or ''): int(getattr(window, 'order', 0) or 0)
        for window in tuple(getattr(config, 'windows', ()) or ())
    }
    return sorted(names, key=lambda name: (order.get(name, 999999), name))


def _ordered_tool_windows(config, names: set[str]) -> list[str]:
    order = {
        str(getattr(tool, 'name', '') or ''): int(getattr(tool, 'order', 0) or 0)
        for tool in tuple(getattr(config, 'tool_windows', ()) or ())
    }
    return sorted(names, key=lambda name: (order.get(name, 999999), name))


def _record_or_none(value) -> dict[str, object] | None:
    if value is None:
        return None
    to_record = getattr(value, 'to_record', None)
    if callable(to_record):
        return dict(to_record())
    return dict(value)


__all__ = [
    'build_invalid_reload_dry_run_plan',
    'build_reload_dry_run_plan',
]
