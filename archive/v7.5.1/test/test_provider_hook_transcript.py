from __future__ import annotations

import json
from pathlib import Path

from provider_hooks.artifacts_runtime.transcript import (
    current_turn_req_id_from_transcript_text,
    latest_last_prompt_req_id_from_transcript_text,
    latest_req_id_from_transcript,
    latest_user_req_id_from_transcript_text,
)


def _jsonl(*records: dict) -> str:
    return "\n".join(json.dumps(record) for record in records) + "\n"


def test_latest_user_req_id_uses_outer_marker_when_body_contains_req_ids() -> None:
    content = _jsonl(
        {
            "type": "user",
            "message": {
                "role": "user",
                "content": (
                    "CCB_REQ_ID: job_current123\n\n"
                    "Review this transcript:\n"
                    "CCB_REQ_ID: job_old456\n\n"
                    "```text\n"
                    "CCB_REQ_ID: job_code789\n"
                    "```\n"
                ),
            },
        }
    )

    assert latest_user_req_id_from_transcript_text(content) == "job_current123"


def test_latest_user_req_id_ignores_body_only_req_id_mentions() -> None:
    content = _jsonl(
        {
            "type": "user",
            "message": {
                "role": "user",
                "content": "Please inspect why CCB_REQ_ID: job_old456 did not return.",
            },
        }
    )

    assert latest_user_req_id_from_transcript_text(content) is None


def test_latest_user_req_id_ignores_tool_result_req_id_after_outer_marker() -> None:
    content = _jsonl(
        {
            "type": "user",
            "message": {
                "role": "user",
                "content": "CCB_REQ_ID: job_current123\n\nRun the check.",
            },
        },
        {
            "type": "user",
            "message": {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "content": "Command output mentioned CCB_REQ_ID: job_tool999",
                    }
                ],
            },
        },
    )

    assert latest_user_req_id_from_transcript_text(content) == "job_current123"


def test_latest_req_id_prefers_latest_outer_user_marker(tmp_path: Path) -> None:
    transcript = tmp_path / "transcript.jsonl"
    transcript.write_text(
        _jsonl(
            {
                "type": "user",
                "message": {
                    "role": "user",
                    "content": "CCB_REQ_ID: job_first111\n\nInitial request.",
                },
            },
            {
                "type": "assistant",
                "message": {"role": "assistant", "content": "Working."},
            },
            {
                "type": "user",
                "message": {
                    "role": "user",
                    "content": (
                        "CCB_REQ_ID: job_second222\n\n"
                        "Forwarded text contains CCB_REQ_ID: job_old333."
                    ),
                },
            },
        ),
        encoding="utf-8",
    )

    assert latest_req_id_from_transcript(transcript) == "job_second222"


def test_last_prompt_req_id_uses_outer_marker_not_body_marker() -> None:
    content = _jsonl(
        {
            "type": "last-prompt",
            "lastPrompt": (
                "CCB_REQ_ID: job_prompt123\n\n"
                "The request body includes CCB_REQ_ID: job_body456."
            ),
        }
    )

    assert latest_last_prompt_req_id_from_transcript_text(content) == "job_prompt123"


def test_current_turn_req_id_follows_tool_result_parent_chain_to_ccb_prompt() -> None:
    content = _jsonl(
        {
            "uuid": "u1",
            "type": "user",
            "message": {
                "role": "user",
                "content": "CCB_REQ_ID: job_current123\n\nRun a tool.",
            },
        },
        {
            "uuid": "a1",
            "parentUuid": "u1",
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{"type": "tool_use", "name": "Read"}],
            },
        },
        {
            "uuid": "u2",
            "parentUuid": "a1",
            "type": "user",
            "message": {
                "role": "user",
                "content": [{"type": "tool_result", "content": "ok"}],
            },
            "toolUseResult": {"type": "text"},
        },
        {
            "uuid": "a2",
            "parentUuid": "u2",
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{"type": "text", "text": "done"}],
            },
        },
    )

    assert current_turn_req_id_from_transcript_text(content, assistant_reply="done") == "job_current123"


def test_current_turn_req_id_ignores_scheduled_task_after_interrupted_ccb_prompt() -> None:
    content = _jsonl(
        {
            "uuid": "u1",
            "type": "user",
            "message": {
                "role": "user",
                "content": "CCB_REQ_ID: job_stale123\n\nRun a long task.",
            },
        },
        {
            "uuid": "u2",
            "parentUuid": "u1",
            "type": "user",
            "message": {
                "role": "user",
                "content": [{"type": "text", "text": "[Request interrupted by user]"}],
            },
        },
        {
            "uuid": "s1",
            "parentUuid": "u2",
            "type": "system",
            "subtype": "scheduled_task_fire",
            "content": "Running scheduled task",
        },
        {
            "uuid": "u3",
            "parentUuid": "s1",
            "type": "user",
            "message": {
                "role": "user",
                "content": "循环计数，共50次",
            },
            "isMeta": True,
        },
        {
            "uuid": "a1",
            "parentUuid": "u3",
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{"type": "text", "text": "当前进度：已完成第9次。"}],
            },
        },
        {
            "type": "last-prompt",
            "lastPrompt": "CCB_REQ_ID: job_stale123\n\nRun a long task.",
        },
    )

    assert current_turn_req_id_from_transcript_text(content, assistant_reply="当前进度：已完成第9次。") is None


def test_current_turn_req_id_for_empty_reply_uses_latest_ccb_prompt_after_previous_assistant() -> None:
    content = _jsonl(
        {
            "uuid": "u1",
            "type": "user",
            "message": {
                "role": "user",
                "content": "CCB_REQ_ID: job_old111\n\nOld task.",
            },
        },
        {
            "uuid": "a1",
            "parentUuid": "u1",
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{"type": "text", "text": "old done"}],
            },
        },
        {
            "uuid": "u2",
            "type": "user",
            "message": {
                "role": "user",
                "content": "CCB_REQ_ID: job_current222\n\nCurrent task.",
            },
        },
    )

    assert current_turn_req_id_from_transcript_text(content, assistant_reply="") == "job_current222"


def test_current_turn_req_id_for_empty_reply_does_not_reuse_previous_assistant_req_id() -> None:
    content = _jsonl(
        {
            "uuid": "u1",
            "type": "user",
            "message": {
                "role": "user",
                "content": "CCB_REQ_ID: job_old111\n\nOld task.",
            },
        },
        {
            "uuid": "a1",
            "parentUuid": "u1",
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{"type": "text", "text": "old done"}],
            },
        },
    )

    assert current_turn_req_id_from_transcript_text(content, assistant_reply="") is None
