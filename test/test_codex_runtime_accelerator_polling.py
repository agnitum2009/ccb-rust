from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

from completion.models import CompletionItemKind, CompletionSourceKind
from provider_backends.codex.execution_runtime.accelerator import poll_with_accelerator
from provider_backends.codex.execution_runtime.polling import poll_submission
from provider_execution.base import ProviderSubmission
from runtime_accelerator.client import AcceleratorError


def _submission(tmp_path: Path) -> ProviderSubmission:
    session_path = str(tmp_path / "session.jsonl")
    return ProviderSubmission(
        job_id="job_1",
        agent_name="agent1",
        provider="codex",
        accepted_at="2026-04-06T00:00:00Z",
        ready_at="2026-04-06T00:00:00Z",
        source_kind=CompletionSourceKind.PROTOCOL_EVENT_STREAM,
        reply="",
        runtime_state={
            "mode": "active",
            "state": {"log_path": session_path, "offset": 9},
            "reader": object(),
            "backend": object(),
            "pane_id": "%1",
            "request_anchor": "job_1",
            "next_seq": 3,
            "anchor_seen": False,
            "bound_turn_id": "",
            "bound_task_id": "",
            "reply_buffer": "",
            "last_agent_message": "",
            "last_final_answer": "",
            "last_assistant_message": "",
            "last_assistant_signature": "",
            "session_path": session_path,
            "workspace_path": str(tmp_path),
            "delivery_state": "pending_anchor",
            "delivery_confirmed_at": "",
        },
    )


def test_codex_accelerator_can_be_disabled(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_CODEX", "0")
    monkeypatch.setattr(
        "provider_backends.codex.execution_runtime.accelerator.accelerator_client.call",
        lambda *args, **kwargs: (_ for _ in ()).throw(AssertionError("accelerator called")),
    )

    assert poll_with_accelerator(_submission(tmp_path), now="2026-04-06T00:01:00Z") is None


def test_codex_accelerator_failure_falls_back_to_python(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_CODEX", "1")
    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_SOCKET", str(tmp_path / "accelerator.sock"))
    monkeypatch.setattr(
        "provider_backends.codex.execution_runtime.accelerator.accelerator_client.call",
        lambda *args, **kwargs: (_ for _ in ()).throw(AcceleratorError("sidecar down")),
    )

    assert poll_with_accelerator(_submission(tmp_path), now="2026-04-06T00:01:00Z") is None


def test_codex_accelerator_builds_poll_result(monkeypatch, tmp_path: Path) -> None:
    captured: dict[str, object] = {}
    session_path = str(tmp_path / "session.jsonl")

    def fake_call(socket_path, method, params, *, timeout_s):
        captured["socket_path"] = str(socket_path)
        captured["method"] = method
        captured["params"] = params
        return {
            "schema_version": 1,
            "observations": [
                {
                    "job_id": "job_1",
                    "session_path": session_path,
                    "state": {
                        "offset": 42,
                        "next_seq": 5,
                        "anchor_seen": True,
                        "bound_turn_id": "turn-1",
                        "bound_task_id": "task-1",
                        "reply_buffer": "hello",
                        "last_agent_message": "",
                        "last_final_answer": "",
                        "last_assistant_message": "hello",
                        "last_assistant_signature": "sig-1",
                        "session_path": session_path,
                    },
                    "items": [
                        {
                            "job_id": "job_1",
                            "kind": "anchor_seen",
                            "seq": 3,
                            "payload": {"turn_id": "job_1", "session_path": session_path},
                        },
                        {
                            "job_id": "job_1",
                            "kind": "assistant_chunk",
                            "seq": 4,
                            "payload": {"text": "hello", "merged_text": "hello", "session_path": session_path},
                        },
                    ],
                    "reached_terminal": False,
                }
            ],
        }

    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_CODEX", "1")
    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_SOCKET", str(tmp_path / "accelerator.sock"))
    monkeypatch.setattr(
        "provider_backends.codex.execution_runtime.accelerator.accelerator_client.call",
        fake_call,
    )

    outcome = poll_with_accelerator(_submission(tmp_path), now="2026-04-06T00:01:00Z")

    assert outcome is not None
    assert outcome.used is True
    result = outcome.result
    assert result is not None
    assert captured["method"] == "codex_observe"
    assert captured["params"]["jobs"][0]["session_path"] == session_path
    assert captured["params"]["jobs"][0]["state"]["offset"] == 9
    assert [item.kind for item in result.items] == [
        CompletionItemKind.ANCHOR_SEEN,
        CompletionItemKind.ASSISTANT_CHUNK,
    ]
    assert result.submission.reply == "hello"
    assert result.submission.runtime_state["state"]["offset"] == 42
    assert result.submission.runtime_state["state"]["log_path"] == session_path
    assert result.submission.runtime_state["anchor_seen"] is True
    assert result.submission.runtime_state["delivery_state"] == "accepted"


def test_poll_submission_uses_accelerator_without_python_reader_when_no_changes(monkeypatch, tmp_path: Path) -> None:
    session_path = str(tmp_path / "session.jsonl")

    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_CODEX", "1")
    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_SOCKET", str(tmp_path / "accelerator.sock"))
    monkeypatch.setattr(
        "provider_backends.codex.execution_runtime.polling_runtime.prepare_active_poll",
        lambda submission, now: SimpleNamespace(reader=object(), backend=object(), pane_id="%1"),
    )
    monkeypatch.setattr(
        "provider_backends.codex.execution_runtime.accelerator.accelerator_client.call",
        lambda *args, **kwargs: {
            "schema_version": 1,
            "observations": [
                {
                    "job_id": "job_1",
                    "session_path": session_path,
                    "state": {
                        "offset": 9,
                        "next_seq": 3,
                        "anchor_seen": False,
                        "session_path": session_path,
                    },
                    "items": [],
                    "reached_terminal": False,
                }
            ],
        },
    )
    monkeypatch.setattr(
        "provider_backends.codex.execution_runtime.polling_runtime.read_entries",
        lambda *args, **kwargs: (_ for _ in ()).throw(AssertionError("python reader fallback called")),
    )

    assert poll_submission(_submission(tmp_path), now="2026-04-06T00:01:00Z") is None
