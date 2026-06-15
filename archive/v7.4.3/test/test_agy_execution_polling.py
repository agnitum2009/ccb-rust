from __future__ import annotations

from completion.models import CompletionSourceKind, CompletionStatus
from provider_backends.agy.comm import AgyPaneReader
from provider_backends.agy.execution_runtime.poll import poll_submission
from provider_execution.base import ProviderSubmission


class _Backend:
    def __init__(self, text: str) -> None:
        self._text = text

    def get_pane_content(self, pane_id: str, *, lines: int) -> str:
        del pane_id, lines
        return self._text


def _submission(text: str) -> ProviderSubmission:
    req_id = 'job_agyempty123'
    backend = _Backend(text)
    return ProviderSubmission(
        job_id=req_id,
        agent_name='agy1',
        provider='agy',
        accepted_at='2026-04-06T00:00:00Z',
        ready_at='2026-04-06T00:00:00Z',
        source_kind=CompletionSourceKind.TERMINAL_TEXT,
        reply='',
        runtime_state={
            'reader': AgyPaneReader(backend=backend, pane_id='%9', lines=200),
            'backend': backend,
            'pane_id': '%9',
            'req_id': req_id,
            'started_at': '2026-04-06T00:00:00Z',
            'last_change_at': '2026-04-06T00:00:00Z',
            'next_seq': 1,
        },
    )


def test_agy_poll_marks_done_marker_with_empty_reply_incomplete() -> None:
    text = (
        'CCB_REQ_ID: job_agyempty123\n'
        'IMPORTANT: when you finish answering\n'
        'CCB_DONE: job_agyempty123\n'
        'CCB_DONE: job_agyempty123\n'
    )

    result = poll_submission(_submission(text), now='2026-04-06T00:00:03Z')

    assert result is not None
    assert result.decision is not None
    assert result.decision.status is CompletionStatus.INCOMPLETE
    assert result.decision.reason == 'pane_done_empty_reply'
    assert result.decision.reply == ''
    assert result.decision.diagnostics['done_seen'] is True
    assert result.decision.diagnostics['reply_chars'] == 0
    assert result.decision.diagnostics['empty_reply'] is True
    assert result.decision.diagnostics['error_type'] == 'empty_provider_reply'
    assert 'without assistant reply text' in result.decision.diagnostics['diagnosis']


def test_agy_poll_completes_done_marker_with_reply() -> None:
    text = (
        'CCB_REQ_ID: job_agyempty123\n'
        'IMPORTANT: when you finish answering\n'
        'CCB_DONE: job_agyempty123\n'
        'final answer\n'
        'CCB_DONE: job_agyempty123\n'
    )

    result = poll_submission(_submission(text), now='2026-04-06T00:00:03Z')

    assert result is not None
    assert result.decision is not None
    assert result.decision.status is CompletionStatus.COMPLETED
    assert result.decision.reason == 'pane_done_marker'
    assert result.decision.reply == 'final answer'
