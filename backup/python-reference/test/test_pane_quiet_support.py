from __future__ import annotations

from completion.models import CompletionSourceKind, CompletionStatus
from provider_backends.pane_quiet_support import PaneSnapshotReader, extract_reply_for_req, poll_submission
from provider_execution.base import ProviderSubmission


class _Backend:
    def __init__(self, text: str) -> None:
        self._text = text
        self.sent_texts: list[str] = []

    def get_pane_content(self, pane_id: str, *, lines: int) -> str:
        del pane_id, lines
        return self._text

    def send_text_to_pane(self, pane_id: str, text: str) -> None:
        del pane_id
        self.sent_texts.append(text)


def _submission(text: str, *, provider: str = "kimi", prompt_sent: bool = True) -> ProviderSubmission:
    req_id = "job_native123"
    backend = _Backend(text)
    return ProviderSubmission(
        job_id=req_id,
        agent_name=f"{provider}_agent",
        provider=provider,
        accepted_at="2026-06-13T00:00:00Z",
        ready_at="2026-06-13T00:00:00Z",
        source_kind=CompletionSourceKind.TERMINAL_TEXT,
        reply="",
        runtime_state={
            "mode": "pane_quiet",
            "provider": provider,
            "reader": PaneSnapshotReader(backend=backend, pane_id="%9", lines=200),
            "backend": backend,
            "pane_id": "%9",
            "req_id": req_id,
            "started_at": "2026-06-13T00:00:00Z",
            "last_change_at": "2026-06-13T00:00:00Z",
            "prompt_sent": prompt_sent,
            "pending_prompt": "pending prompt",
            "next_seq": 1,
        },
    )


def test_extract_reply_for_req_handles_echo_and_model_done_markers() -> None:
    text = (
        "CCB_REQ_ID: job_native123\n"
        "IMPORTANT: when you finish answering\n"
        "CCB_DONE: job_native123\n"
        "final answer\n"
        "CCB_DONE: job_native123\n"
    )

    reply, done_seen = extract_reply_for_req(text, "job_native123")

    assert done_seen is True
    assert reply == "final answer"


def test_extract_reply_for_req_strips_kimi_tui_assistant_bullet() -> None:
    text = (
        "CCB_REQ_ID: job_native123\n"
        "IMPORTANT: when you finish answering\n"
        "CCB_DONE: job_native123\n"
        "• final answer\n"
        "  CCB_DONE: job_native123\n"
    )

    reply, done_seen = extract_reply_for_req(text, "job_native123")

    assert done_seen is True
    assert reply == "final answer"


def test_extract_reply_for_req_handles_single_model_done_marker_when_prompt_echo_is_hidden() -> None:
    text = (
        "CCB_REQ_ID: job_native123\n"
        "final answer\n"
        "CCB_DONE: job_native123\n"
    )

    reply, done_seen = extract_reply_for_req(text, "job_native123")

    assert done_seen is True
    assert reply == "final answer"


def test_extract_reply_for_req_ignores_single_prompt_echo_done_marker() -> None:
    text = (
        "CCB_REQ_ID: job_native123\n"
        "please answer\n"
        "IMPORTANT: when you finish answering, write this exact line\n"
        "CCB_DONE: job_native123\n"
    )

    reply, done_seen = extract_reply_for_req(text, "job_native123")

    assert done_seen is False
    assert reply == ""


def test_pane_quiet_poll_marks_done_marker_with_reply_completed() -> None:
    text = (
        "CCB_REQ_ID: job_native123\n"
        "IMPORTANT: when you finish answering\n"
        "CCB_DONE: job_native123\n"
        "final answer\n"
        "CCB_DONE: job_native123\n"
    )

    result = poll_submission(_submission(text), now="2026-06-13T00:00:03Z")

    assert result is not None
    assert result.decision is not None
    assert result.decision.status is CompletionStatus.COMPLETED
    assert result.decision.reason == "pane_done_marker"
    assert result.decision.reply == "final answer"


def test_pane_quiet_poll_defers_kimi_prompt_until_input_ready() -> None:
    result = poll_submission(_submission("Kimi is booting\n", prompt_sent=False), now="2026-06-13T00:00:03Z")

    assert result is not None
    assert result.decision is None
    backend = result.submission.runtime_state["backend"]
    assert isinstance(backend, _Backend)
    assert backend.sent_texts == []
    assert result.submission.runtime_state["prompt_sent"] is False


def test_pane_quiet_poll_sends_deferred_kimi_prompt_when_input_ready() -> None:
    text = "Welcome to Kimi Code CLI!\n── input ─────────\nagent (kimi-for-coding ○)\n"
    result = poll_submission(_submission(text, prompt_sent=False), now="2026-06-13T00:00:03Z")

    assert result is not None
    assert result.decision is None
    backend = result.submission.runtime_state["backend"]
    assert isinstance(backend, _Backend)
    assert backend.sent_texts == ["pending prompt"]
    assert result.submission.runtime_state["prompt_sent"] is True
    assert result.submission.runtime_state["prompt_deferred_until_ready"] is False


def test_pane_quiet_poll_sends_deferred_kimi_prompt_with_k27_input_box() -> None:
    text = (
        "✦ K2.7 Code is ready higher end-to-end coding task success rates\n"
        "╭────────────────────────────────────────────────────────╮\n"
        "│ >                                                      │\n"
        "╰────────────────────────────────────────────────────────╯\n"
        "yolo  K2.7 Code thinking  /home/agnitum/o13  context: 0.0% (0/262.1k)\n"
    )
    result = poll_submission(_submission(text, prompt_sent=False), now="2026-06-13T00:00:03Z")

    assert result is not None
    assert result.decision is None
    backend = result.submission.runtime_state["backend"]
    assert isinstance(backend, _Backend)
    assert backend.sent_texts == ["pending prompt"]
    assert result.submission.runtime_state["prompt_sent"] is True
    assert result.submission.runtime_state["prompt_deferred_until_ready"] is False


def test_pane_quiet_poll_reports_kimi_input_not_ready_timeout() -> None:
    result = poll_submission(_submission("Kimi is booting\n", prompt_sent=False), now="2026-06-13T00:02:00Z")

    assert result is not None
    assert result.decision is not None
    assert result.decision.status is CompletionStatus.INCOMPLETE
    assert result.decision.reason == "kimi_input_not_ready"
    assert result.decision.diagnostics["input_not_ready"] is True


def test_pane_quiet_poll_marks_done_marker_with_empty_reply_incomplete() -> None:
    text = (
        "CCB_REQ_ID: job_native123\n"
        "IMPORTANT: when you finish answering\n"
        "CCB_DONE: job_native123\n"
        "CCB_DONE: job_native123\n"
    )

    result = poll_submission(_submission(text, provider="deepseek"), now="2026-06-13T00:00:03Z")

    assert result is not None
    assert result.decision is not None
    assert result.decision.status is CompletionStatus.INCOMPLETE
    assert result.decision.reason == "pane_done_empty_reply"
    assert result.decision.diagnostics["empty_reply"] is True
    assert result.decision.diagnostics["error_type"] == "empty_provider_reply"
    assert "deepseek pane showed" in result.decision.diagnostics["diagnosis"]


def test_pane_quiet_poll_reports_input_unresponsive_when_anchor_never_appears() -> None:
    result = poll_submission(_submission("provider prompt\n"), now="2026-06-13T00:03:00Z")

    assert result is not None
    assert result.decision is not None
    assert result.decision.status is CompletionStatus.INCOMPLETE
    assert result.decision.reason == "kimi_input_unresponsive"
