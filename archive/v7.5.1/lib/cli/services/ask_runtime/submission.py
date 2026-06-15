from __future__ import annotations

from collections.abc import Callable, Collection

from agents.config_loader_runtime.role_lookup import looks_like_role_id, normalize_role_id
from agents.models import AgentValidationError
from ccbd.api_models import DeliveryScope, MessageEnvelope
from mailbox_runtime.targets import NON_AGENT_ACTORS, normalize_actor_name
from storage.text_artifacts import artifact_stub, maybe_spill_text, write_text_artifact

from .models import AskSummary

_DEFAULT_REPLY_GUIDANCE = """CCB reply guidance:
- Answer directly and concisely.
- Include only relevant conclusions, blockers, risks, evidence, and next actions.
- Avoid raw logs and background unless explicitly requested."""

_COMPACT_REPLY_GUIDANCE = """CCB reply guidance:
- Distill aggressively and lead with the answer.
- Keep only details needed for this ask.
- Omit empty sections, raw logs, repeated context, and background unless essential."""

_SILENT_REPLY_GUIDANCE = """CCB reply guidance:
- Silent-on-success requested.
- Reply with the shortest useful status.
- Include details only for failures, blockers, or required next actions."""

_GUIDANCE_MARKER = 'CCB reply guidance:'
_EXPLICIT_OUTPUT_HINTS = (
    'output requirements',
    'reply format',
    'response format',
    'format:',
    'only reply',
    'reply only',
    'full report',
    'full output',
    'detailed report',
    'complete output',
    'include everything',
    'all details',
    'leave nothing out',
    'verbatim',
    'do not summarize',
    'do not abbreviate',
    '\u5b8c\u6574\u8f93\u51fa',
    '\u4e0d\u8981\u603b\u7ed3',
    '\u4e0d\u8981\u538b\u7f29',
    '\u4e0d\u8981\u7cbe\u7b80',
    '\u4e0d\u8981\u7701\u7565',
    '\u9010\u5b57\u8fd4\u56de',
    '\u9010\u5b57',
    '\u539f\u6837\u8fd4\u56de',
    '\u4fdd\u7559\u539f\u6587',
    '\u5b8c\u6574\u65e5\u5fd7',
    '\u5b8c\u6574\u62a5\u544a',
    '\u8be6\u7ec6\u62a5\u544a',
    '\u5168\u6587',
)


def submit_ask(
    context,
    command,
    *,
    load_project_config_fn: Callable,
    resolve_ask_sender_fn: Callable,
    invoke_mounted_daemon_fn: Callable,
) -> AskSummary:
    config = load_project_config_fn(context.project.project_root).config
    normalized_target = _resolve_target(command.target, config.agents)
    _validate_target(normalized_target, config.agents)
    sender = resolve_ask_sender_fn(context, command.sender)
    normalized_sender = _normalize_sender(sender)
    _validate_sender(normalized_sender, config.agents)
    message_body = message_with_reply_guidance(
        command.message,
        message_type=command.mode or 'ask',
        compact=bool(getattr(command, 'compact', False)),
        silence_on_success=command.silence,
    )
    message_body, body_artifact = _artifact_request_body(
        context.paths,
        message_body,
        owner_id=f'{normalized_sender}-to-{normalized_target}',
        force=bool(getattr(command, 'artifact_request', False)),
    )
    payload = invoke_mounted_daemon_fn(
        context,
        allow_restart_stale=True,
        request_fn=lambda client: client.submit(
            MessageEnvelope(
                project_id=context.project.project_id,
                to_agent=normalized_target,
                from_actor=normalized_sender,
                body=message_body,
                task_id=command.task_id,
                reply_to=command.reply_to,
                message_type=command.mode or 'ask',
                delivery_scope=_delivery_scope(normalized_target),
                silence_on_success=command.silence,
                route_options=_route_options(command),
                body_artifact=body_artifact,
            )
        )
    )
    return _summary_from_payload(context.project.project_id, payload)


def _route_options(command) -> dict[str, object]:
    options: dict[str, object] = {}
    if bool(getattr(command, 'callback', False)):
        options['mode'] = 'callback'
    if bool(getattr(command, 'artifact_request', False)):
        options['artifact_request'] = True
    if bool(getattr(command, 'artifact_reply', False)):
        options['artifact_reply'] = True
    return options


def _artifact_request_body(layout, message_body: str, *, owner_id: str, force: bool):
    if force:
        artifact = write_text_artifact(
            layout,
            text=message_body,
            kind='ask-request',
            owner_id=owner_id,
        )
        return (
            artifact_stub(
                prefix='CCB ask request was stored as an artifact by --artifact-request.',
                artifact=artifact,
                include_preview=False,
            ),
            artifact,
        )
    return maybe_spill_text(
        layout,
        text=message_body,
        kind='ask-request',
        owner_id=owner_id,
        prefix='CCB ask request is larger than 4 KiB and was stored as an artifact.',
    )


def message_with_reply_guidance(
    message: str,
    *,
    message_type: str,
    compact: bool = False,
    silence_on_success: bool = False,
) -> str:
    if str(message_type or '').strip().lower() != 'ask':
        return message
    if _has_explicit_output_guidance(message):
        return message
    if silence_on_success:
        guidance = _SILENT_REPLY_GUIDANCE
    elif compact:
        guidance = _COMPACT_REPLY_GUIDANCE
    else:
        guidance = _DEFAULT_REPLY_GUIDANCE
    return f'{str(message).rstrip()}\n\n{guidance}'


def _has_explicit_output_guidance(message: str) -> bool:
    text = str(message or '')
    lowered = text.lower()
    if _GUIDANCE_MARKER.lower() in lowered:
        return True
    return any(hint in lowered for hint in _EXPLICIT_OUTPUT_HINTS)


def _normalize_sender(value: str | None) -> str:
    try:
        return normalize_actor_name(value)
    except AgentValidationError as exc:
        raise ValueError(str(exc)) from exc


def _normalize_target(value: str | None) -> str:
    normalized = str(value or '').strip().lower()
    if normalized == 'all':
        return normalized
    if looks_like_role_id(normalized):
        return normalize_role_id(normalized)
    return _normalize_sender(normalized)


def _validate_target(target: str, configured_agents: Collection[str]) -> None:
    if target != 'all' and target not in configured_agents:
        raise ValueError(f'unknown agent: {target}')


def _resolve_target(value: str | None, configured_agents: Collection[str]) -> str:
    normalized = _normalize_target(value)
    if normalized == 'all' or normalized in configured_agents:
        return normalized
    if looks_like_role_id(normalized):
        role_id = normalize_role_id(normalized)
        matches = sorted(
            name
            for name, spec in dict(configured_agents).items()
            if str(getattr(spec, 'role', '') or '').strip().lower() == role_id
        )
        if len(matches) == 1:
            return matches[0]
        if not matches:
            raise ValueError(
                f'role {role_id} is not bound to any configured agent; '
                'target the project-local agent name or add the role to config'
            )
        raise ValueError(
            f'role {role_id} is bound to multiple agents: {", ".join(matches)}; '
            'target one agent name explicitly'
        )
    return normalized


def _validate_sender(sender: str, configured_agents: Collection[str]) -> None:
    if sender in NON_AGENT_ACTORS:
        if sender == 'cmd':
            raise ValueError(f'unknown sender agent: {sender}')
        return
    if sender in configured_agents:
        return
    raise ValueError(f'unknown sender agent: {sender}')


def _delivery_scope(target: str | None) -> DeliveryScope:
    return DeliveryScope.BROADCAST if str(target or '').strip().lower() == 'all' else DeliveryScope.SINGLE


def _summary_from_payload(project_id: str, payload: dict) -> AskSummary:
    if 'job_id' in payload:
        jobs = (
            {
                'job_id': payload['job_id'],
                'agent_name': payload['agent_name'],
                'target_kind': payload.get('target_kind', 'agent'),
                'target_name': payload.get('target_name', payload['agent_name']),
                'provider_instance': payload.get('provider_instance'),
                'status': payload['status'],
            },
        )
        submission_id = None
    else:
        jobs = tuple(payload.get('jobs', ()))
        submission_id = payload.get('submission_id')
    return AskSummary(project_id=project_id, submission_id=submission_id, jobs=jobs)


__all__ = ['message_with_reply_guidance', 'submit_ask']
