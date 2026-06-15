from __future__ import annotations

from completion.models import CompletionItemKind
from provider_execution.base import ProviderSubmission
from provider_execution.common import build_item

from ...protocol import extract_reply_for_req, is_done_text, strip_done_text
from .models import ClaudePollState


def handle_assistant_event(
    submission: ProviderSubmission,
    poll: ClaudePollState,
    event: dict[str, object],
    *,
    now: str,
) -> None:
    text = assistant_text(event)
    subagent_id, subagent_name, is_subagent = assistant_identity(event)
    event_assistant_uuid = assistant_uuid(event)
    poll.raw_buffer = append_buffer(poll.raw_buffer, text)

    cleaned = cleaned_assistant_text(text, request_anchor=poll.request_anchor)
    if has_visible_text(cleaned):
        append_chunk_item(
            submission,
            poll,
            event=event,
            cleaned=cleaned,
            subagent_id=subagent_id,
            subagent_name=subagent_name,
            is_subagent=is_subagent,
            event_assistant_uuid=event_assistant_uuid,
            now=now,
        )

    maybe_append_turn_boundary(submission, poll, now=now)
    maybe_append_assistant_end_turn_boundary(
        submission,
        poll,
        event=event,
        cleaned=cleaned,
        is_subagent=is_subagent,
        now=now,
    )


def assistant_text(event: dict[str, object]) -> str:
    return str(event.get("text") or "")


def assistant_identity(event: dict[str, object]) -> tuple[str, str, bool]:
    subagent_id = str(event.get("subagent_id") or "").strip()
    subagent_name = str(event.get("subagent_name") or "").strip()
    return subagent_id, subagent_name, bool(subagent_id or subagent_name)


def assistant_uuid(event: dict[str, object]) -> str:
    return str(event.get("uuid") or "").strip()


def assistant_stop_reason(event: dict[str, object]) -> str:
    return str(event.get("stop_reason") or "").strip().lower()


def append_buffer(buffer: str, text: str) -> str:
    return f"{buffer}\n{text}".strip() if buffer else text


def cleaned_assistant_text(text: str, *, request_anchor: str) -> str:
    if not request_anchor:
        return text
    return strip_done_text(text, request_anchor)


def has_visible_text(text: str) -> bool:
    return bool(text.strip())


def append_chunk_item(
    submission: ProviderSubmission,
    poll: ClaudePollState,
    *,
    event: dict[str, object],
    cleaned: str,
    subagent_id: str,
    subagent_name: str,
    is_subagent: bool,
    event_assistant_uuid: str,
    now: str,
) -> None:
    poll.reply_buffer = append_buffer(poll.reply_buffer, cleaned)
    if not is_subagent:
        poll.last_assistant_uuid = str(event_assistant_uuid or poll.last_assistant_uuid or "")
    current_assistant_uuid = event_assistant_uuid or poll.last_assistant_uuid or None
    poll.items.append(
        build_item(
            submission,
            kind=CompletionItemKind.ASSISTANT_CHUNK,
            timestamp=now,
            seq=poll.next_seq,
            payload=chunk_payload(
                poll=poll,
                event=event,
                cleaned=cleaned,
                assistant_uuid=current_assistant_uuid,
                subagent_id=subagent_id,
                subagent_name=subagent_name,
            ),
        )
    )
    poll.next_seq += 1


def chunk_payload(
    *,
    poll: ClaudePollState,
    event: dict[str, object],
    cleaned: str,
    assistant_uuid: str | None,
    subagent_id: str,
    subagent_name: str,
) -> dict[str, object]:
    return {
        "text": cleaned,
        "merged_text": poll.reply_buffer,
        "turn_id": poll.request_anchor,
        "session_path": poll.session_path or None,
        "assistant_uuid": assistant_uuid,
        "subagent_id": subagent_id or None,
        "subagent_name": subagent_name or None,
        "stop_reason": event.get("stop_reason"),
    }


def maybe_append_turn_boundary(
    submission: ProviderSubmission,
    poll: ClaudePollState,
    *,
    now: str,
) -> None:
    if poll.reached_turn_boundary:
        return
    if not poll.request_anchor or not is_done_text(poll.raw_buffer, poll.request_anchor):
        return
    reply = extract_reply_for_req(poll.raw_buffer, poll.request_anchor) or poll.reply_buffer
    poll.items.append(
        build_item(
            submission,
            kind=CompletionItemKind.TURN_BOUNDARY,
            timestamp=now,
            seq=poll.next_seq,
            payload=turn_boundary_payload(poll=poll, reply=reply),
        )
    )
    poll.next_seq += 1
    poll.reached_turn_boundary = True


def maybe_append_assistant_end_turn_boundary(
    submission: ProviderSubmission,
    poll: ClaudePollState,
    *,
    event: dict[str, object],
    cleaned: str,
    is_subagent: bool,
    now: str,
) -> None:
    if poll.reached_turn_boundary:
        return
    if is_subagent or not poll.anchor_seen or not poll.request_anchor:
        return
    if assistant_stop_reason(event) != "end_turn":
        return
    reply = poll.reply_buffer if has_visible_text(poll.reply_buffer) else cleaned
    if not has_visible_text(reply):
        return
    event_assistant_uuid = assistant_uuid(event)
    if event_assistant_uuid:
        poll.last_assistant_uuid = event_assistant_uuid
    poll.items.append(
        build_item(
            submission,
            kind=CompletionItemKind.TURN_BOUNDARY,
            timestamp=now,
            seq=poll.next_seq,
            payload=turn_boundary_payload(
                poll=poll,
                reply=reply,
                reason="assistant_end_turn",
                stop_reason="end_turn",
            ),
        )
    )
    poll.next_seq += 1
    poll.reached_turn_boundary = True


def turn_boundary_payload(
    *,
    poll: ClaudePollState,
    reply: str,
    reason: str = "task_complete",
    stop_reason: str | None = None,
) -> dict[str, object]:
    payload = {
        "reason": reason,
        "last_agent_message": reply,
        "turn_id": poll.request_anchor,
        "session_path": poll.session_path or None,
        "assistant_uuid": poll.last_assistant_uuid or None,
    }
    if stop_reason:
        payload["stop_reason"] = stop_reason
    return payload


__all__ = ["handle_assistant_event"]
