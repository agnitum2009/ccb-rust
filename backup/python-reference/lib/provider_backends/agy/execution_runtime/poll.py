from __future__ import annotations

from dataclasses import replace
from pathlib import Path

from completion.models import (
    CompletionConfidence,
    CompletionCursor,
    CompletionDecision,
    CompletionItemKind,
    CompletionStatus,
)
from provider_execution.base import ProviderPollResult, ProviderSubmission
from provider_execution.common import build_item

from ..native_log import observe_agy_transcript
from .helpers import hash_text, seconds_between, state_int, state_str


MAX_WAIT_SECS = 300.0
ANCHOR_WAIT_SECS = 120.0


def poll_submission(submission: ProviderSubmission, *, now: str) -> ProviderPollResult | None:
    state = dict(submission.runtime_state)

    send_error = state.get('send_error')
    if send_error:
        return _terminal(
            submission,
            state,
            now,
            status=CompletionStatus.FAILED,
            reason=f'send_failed:{send_error}',
            reply='',
            confidence=CompletionConfidence.DEGRADED,
        )

    pane_id = state_str(state, 'pane_id')
    req_id = state_str(state, 'request_anchor') or state_str(state, 'req_id')
    work_dir = state_str(state, 'work_dir')
    if not pane_id or not req_id or not work_dir:
        return _terminal(
            submission,
            state,
            now,
            status=CompletionStatus.FAILED,
            reason='runtime_state_invalid',
            reply='',
            confidence=CompletionConfidence.DEGRADED,
        )

    if state.get('backend') is None:
        return _terminal(
            submission,
            state,
            now,
            status=CompletionStatus.FAILED,
            reason='runtime_handle_lost',
            reply='',
            confidence=CompletionConfidence.DEGRADED,
        )

    started_at = state_str(state, 'started_at') or submission.accepted_at or now

    state['last_poll_at'] = now
    state['next_seq'] = state_int(state, 'next_seq', 1)
    total_secs = seconds_between(started_at, now)
    state['total_secs'] = total_secs

    observation = observe_agy_transcript(
        Path(work_dir),
        req_id,
        home_candidates=_home_candidates(state),
    )
    if observation is None:
        if total_secs >= ANCHOR_WAIT_SECS:
            return _terminal(
                submission,
                state,
                now,
                status=CompletionStatus.INCOMPLETE,
                reason='agy_native_anchor_missing',
                reply='',
                confidence=CompletionConfidence.DEGRADED,
                diagnostics_extra={
                    'anchor_seen': False,
                    'diagnosis': 'AGY transcript did not record the submitted CCB_REQ_ID.',
                },
            )
        return ProviderPollResult(submission=replace(submission, runtime_state=state), items=(), decision=None)

    items = []
    transcript_path = str(observation.transcript_path or '')
    if transcript_path and transcript_path != state_str(state, 'session_path'):
        items.append(
            build_item(
                submission,
                kind=CompletionItemKind.SESSION_ROTATE,
                timestamp=now,
                seq=_next_seq(state),
                payload={
                    'session_path': transcript_path,
                    'provider_session_id': observation.conversation_id,
                },
                cursor_kwargs={'session_path': transcript_path},
            )
        )
        state['session_path'] = transcript_path
        state['anchor_emitted'] = False
        state['reply_buffer'] = ''
        state['last_reply_signature'] = ''
        state['turn_boundary_ref'] = ''

    if observation.request_seen and not bool(state.get('anchor_emitted')):
        items.append(
            build_item(
                submission,
                kind=CompletionItemKind.ANCHOR_SEEN,
                timestamp=now,
                seq=_next_seq(state),
                payload={
                    'turn_id': req_id,
                    'session_path': transcript_path or None,
                    'provider_session_id': observation.conversation_id,
                    'native_started_at': observation.native_started_at,
                },
                cursor_kwargs={'session_path': transcript_path or None},
            )
        )
        state['anchor_emitted'] = True

    reply = observation.reply or ''
    reply_signature = hash_text(reply)
    if reply and reply_signature != state_str(state, 'last_reply_signature'):
        state['reply_buffer'] = reply
        state['last_reply_signature'] = reply_signature
        items.append(
            build_item(
                submission,
                kind=CompletionItemKind.ASSISTANT_FINAL,
                timestamp=now,
                seq=_next_seq(state),
                payload={
                    'text': reply,
                    'reply': reply,
                    'final_answer': reply,
                    'turn_id': req_id,
                    'session_path': transcript_path or None,
                    'provider_session_id': observation.conversation_id,
                    'provider_turn_ref': observation.provider_turn_ref,
                    'native_completed': observation.completed,
                },
                cursor_kwargs={'session_path': transcript_path or None},
            )
        )

    boundary_ref = str(observation.provider_turn_ref or observation.conversation_id or transcript_path or req_id)
    if observation.completed and boundary_ref != state_str(state, 'turn_boundary_ref'):
        items.append(
            build_item(
                submission,
                kind=CompletionItemKind.TURN_BOUNDARY,
                timestamp=now,
                seq=_next_seq(state),
                payload={
                    'reason': 'agy_transcript_response_done',
                    'last_agent_message': reply,
                    'turn_id': req_id,
                    'session_path': transcript_path or None,
                    'provider_session_id': observation.conversation_id,
                    'provider_turn_ref': observation.provider_turn_ref,
                    'native_completed_at': observation.native_completed_at,
                    'latest_status': observation.latest_status,
                },
                cursor_kwargs={'session_path': transcript_path or None},
            )
        )
        state['turn_boundary_ref'] = boundary_ref

    if total_secs >= MAX_WAIT_SECS and not observation.completed:
        return _terminal(
            submission,
            state,
            now,
            status=CompletionStatus.FAILED,
            reason='agy_native_turn_timeout',
            reply=str(state.get('reply_buffer') or ''),
            confidence=CompletionConfidence.DEGRADED,
        )

    progress = replace(submission, reply=str(state.get('reply_buffer') or ''), runtime_state=state)
    if items or progress != submission:
        return ProviderPollResult(submission=progress, items=tuple(items), decision=None)
    return None


def _home_candidates(state: dict[str, object]) -> list[Path]:
    candidates: list[Path] = []
    for key in ('agy_home', 'home', 'runtime_home'):
        value = state_str(state, key)
        if value:
            candidates.append(Path(value))
    runtime_dir = state_str(state, 'runtime_dir')
    if runtime_dir:
        candidates.append(Path(runtime_dir) / 'home')
    return candidates


def _terminal(
    submission: ProviderSubmission,
    state: dict[str, object],
    now: str,
    *,
    status: CompletionStatus,
    reason: str,
    reply: str,
    confidence: CompletionConfidence,
    diagnostics_extra: dict[str, object] | None = None,
) -> ProviderPollResult:
    cleaned_reply = reply or ''
    progress = replace(
        submission,
        runtime_state=state,
        status=status,
        reason=reason,
        reply=cleaned_reply,
        confidence=confidence,
    )
    cursor = CompletionCursor(
        source_kind=submission.source_kind,
        event_seq=state_int(state, 'next_seq', 1),
        updated_at=now,
    )
    diagnostics = {
        'mode': 'native_transcript_log',
        'total_secs': float(state.get('total_secs') or 0.0),
        'anchor_seen': bool(state.get('anchor_emitted')),
        'reply_chars': len(cleaned_reply),
    }
    diagnostics.update(diagnostics_extra or {})
    decision = CompletionDecision(
        terminal=True,
        status=status,
        reason=reason,
        confidence=confidence,
        reply=cleaned_reply,
        anchor_seen=bool(state.get('anchor_emitted')),
        reply_started=bool(cleaned_reply),
        reply_stable=bool(cleaned_reply) and status is CompletionStatus.COMPLETED,
        provider_turn_ref=state_str(state, 'req_id') or None,
        source_cursor=cursor,
        finished_at=now,
        diagnostics=diagnostics,
    )
    return ProviderPollResult(submission=progress, items=(), decision=decision)


def _next_seq(state: dict[str, object]) -> int:
    seq = state_int(state, 'next_seq', 1)
    state['next_seq'] = seq + 1
    return seq


__all__ = ['poll_submission']
