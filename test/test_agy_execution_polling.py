from __future__ import annotations

import json
from pathlib import Path

from completion.models import CompletionItemKind, CompletionSourceKind
from provider_backends.agy.execution_runtime.poll import poll_submission
from provider_execution.base import ProviderSubmission


class _Backend:
    pass


def _write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        ''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in rows),
        encoding='utf-8',
    )


def _submission(work_dir: Path, home: Path) -> ProviderSubmission:
    req_id = 'job_agynative123'
    return ProviderSubmission(
        job_id=req_id,
        agent_name='agy1',
        provider='agy',
        accepted_at='2026-06-13T00:00:00Z',
        ready_at='2026-06-13T00:00:00Z',
        source_kind=CompletionSourceKind.SESSION_EVENT_LOG,
        reply='',
        runtime_state={
            'mode': 'native_transcript_log',
            'backend': _Backend(),
            'pane_id': '%9',
            'req_id': req_id,
            'request_anchor': req_id,
            'work_dir': str(work_dir),
            'agy_home': str(home),
            'started_at': '2026-06-13T00:00:00Z',
            'last_poll_at': '2026-06-13T00:00:00Z',
            'next_seq': 1,
            'anchor_emitted': False,
            'reply_buffer': '',
            'last_reply_signature': '',
            'turn_boundary_ref': '',
            'session_path': '',
        },
    )


def test_agy_poll_completes_from_native_transcript(tmp_path: Path) -> None:
    home = tmp_path / 'home'
    work_dir = tmp_path / 'project'
    work_dir.mkdir()
    transcript = (
        home
        / '.gemini'
        / 'antigravity-cli'
        / 'brain'
        / 'conv-1'
        / '.system_generated'
        / 'logs'
        / 'transcript.jsonl'
    )
    _write_jsonl(
        transcript,
        [
            {
                'step_index': 1,
                'source': 'USER_EXPLICIT',
                'type': 'USER_INPUT',
                'status': 'DONE',
                'created_at': '2026-06-13T00:00:01Z',
                'content': 'CCB_REQ_ID: job_agynative123\nhello',
            },
            {
                'step_index': 2,
                'source': 'MODEL',
                'type': 'PLANNER_RESPONSE',
                'status': 'DONE',
                'created_at': '2026-06-13T00:00:03Z',
                'content': 'native agy reply',
            },
        ],
    )

    result = poll_submission(_submission(work_dir, home), now='2026-06-13T00:00:05Z')

    assert result is not None
    assert result.decision is None
    assert result.submission.reply == 'native agy reply'
    assert [item.kind for item in result.items] == [
        CompletionItemKind.SESSION_ROTATE,
        CompletionItemKind.ANCHOR_SEEN,
        CompletionItemKind.ASSISTANT_FINAL,
        CompletionItemKind.TURN_BOUNDARY,
    ]
    assert result.items[-1].payload['reason'] == 'agy_transcript_response_done'
