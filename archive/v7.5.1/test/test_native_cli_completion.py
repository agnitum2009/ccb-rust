from __future__ import annotations

import json
from pathlib import Path

from completion.models import CompletionItemKind, CompletionSourceKind, CompletionStatus
from provider_backends.agy.execution_runtime.poll import poll_submission as poll_agy_submission
from provider_backends.agy.native_log import agy_home_from_start_cmd, observe_agy_transcript
from provider_backends.agy.protocol import wrap_agy_prompt
from provider_backends.deepseek.execution import DeepSeekProviderAdapter
from provider_backends.deepseek.native_log import (
    deepseek_project_code,
    deepseek_project_root,
    observe_deepseek_session,
)
from provider_backends.kimi.execution import KimiProviderAdapter
from provider_backends.kimi.native_log import kimi_project_hash, kimi_sessions_root, observe_kimi_turn
from provider_backends.native_cli_support import wrap_native_prompt
from provider_execution.base import ProviderSubmission


class _Backend:
    pass


def _submission(
    *,
    provider: str,
    source_kind: CompletionSourceKind,
    work_dir: Path,
    req_id: str = "job_native123",
    extra_state: dict[str, object] | None = None,
) -> ProviderSubmission:
    return ProviderSubmission(
        job_id=req_id,
        agent_name=f"{provider}1",
        provider=provider,
        accepted_at="2026-06-13T00:00:00Z",
        ready_at="2026-06-13T00:00:00Z",
        source_kind=source_kind,
        reply="",
        runtime_state={
            "mode": "native",
            "backend": _Backend(),
            "pane_id": "%9",
            "req_id": req_id,
            "request_anchor": req_id,
            "work_dir": str(work_dir),
            "started_at": "2026-06-13T00:00:00Z",
            "last_poll_at": "2026-06-13T00:00:00Z",
            "prompt_sent": True,
            "next_seq": 1,
            "anchor_emitted": False,
            "reply_buffer": "",
            "last_reply_signature": "",
            "turn_boundary_ref": "",
            "session_path": "",
            **(extra_state or {}),
        },
    )


def _write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(row, ensure_ascii=False) + "\n" for row in rows),
        encoding="utf-8",
    )


def test_native_prompts_do_not_request_ccb_done() -> None:
    assert "CCB_DONE" not in wrap_native_prompt("answer", "job_native123")
    assert "CCB_DONE" not in wrap_agy_prompt("answer", "job_native123")
    assert "CCB_REQ_ID" in wrap_agy_prompt("answer", "job_native123")


def test_kimi_observes_wire_turn_end_and_poll_emits_boundary(monkeypatch, tmp_path: Path) -> None:
    home = tmp_path / "home"
    work_dir = tmp_path / "project"
    work_dir.mkdir()
    monkeypatch.setenv("HOME", str(home))
    wire = kimi_sessions_root(work_dir, home=home) / "session-1" / "wire.jsonl"
    _write_jsonl(
        wire,
        [
            {
                "timestamp": "2026-06-13T00:00:01Z",
                "message": {
                    "type": "TurnBegin",
                    "payload": {"user_input": [{"type": "text", "text": "CCB_REQ_ID: job_native123\nhello"}]},
                },
            },
            {
                "timestamp": "2026-06-13T00:00:02Z",
                "message": {"type": "ContentPart", "payload": {"type": "text", "text": "• native kimi reply"}},
            },
            {
                "timestamp": "2026-06-13T00:00:03Z",
                "message": {"type": "StatusUpdate", "payload": {"message_id": "msg-1"}},
            },
            {"timestamp": "2026-06-13T00:00:04Z", "message": {"type": "TurnEnd", "payload": {}}},
        ],
    )

    observed = observe_kimi_turn(work_dir, "job_native123", home_candidates=[home])

    assert kimi_project_hash(work_dir)
    assert observed is not None
    assert observed.completed is True
    assert observed.reply == "native kimi reply"

    result = KimiProviderAdapter().poll(
        _submission(provider="kimi", source_kind=CompletionSourceKind.SESSION_EVENT_LOG, work_dir=work_dir),
        now="2026-06-13T00:00:05Z",
    )

    assert result is not None
    assert result.decision is None
    assert result.submission.reply == "native kimi reply"
    assert [item.kind for item in result.items] == [
        CompletionItemKind.SESSION_ROTATE,
        CompletionItemKind.ANCHOR_SEEN,
        CompletionItemKind.ASSISTANT_FINAL,
        CompletionItemKind.TURN_BOUNDARY,
    ]


def test_kimi_completed_empty_reply_is_incomplete(monkeypatch, tmp_path: Path) -> None:
    home = tmp_path / "home"
    work_dir = tmp_path / "project"
    work_dir.mkdir()
    monkeypatch.setenv("HOME", str(home))
    wire = kimi_sessions_root(work_dir, home=home) / "session-1" / "wire.jsonl"
    _write_jsonl(
        wire,
        [
            {
                "timestamp": "2026-06-13T00:00:01Z",
                "message": {
                    "type": "TurnBegin",
                    "payload": {"user_input": [{"type": "text", "text": "CCB_REQ_ID: job_native123"}]},
                },
            },
            {"timestamp": "2026-06-13T00:00:04Z", "message": {"type": "TurnEnd", "payload": {}}},
        ],
    )

    result = KimiProviderAdapter().poll(
        _submission(provider="kimi", source_kind=CompletionSourceKind.SESSION_EVENT_LOG, work_dir=work_dir),
        now="2026-06-13T00:00:05Z",
    )

    assert result is not None
    assert result.decision is not None
    assert result.decision.status is CompletionStatus.INCOMPLETE
    assert result.decision.reason == "kimi_native_empty_reply"
    assert result.decision.diagnostics["empty_reply"] is True


def test_kimi_observes_source_style_turn_events(monkeypatch, tmp_path: Path) -> None:
    home = tmp_path / "home"
    work_dir = tmp_path / "project"
    work_dir.mkdir()
    monkeypatch.setenv("HOME", str(home))
    wire = kimi_sessions_root(work_dir, home=home) / "session-1" / "wire.jsonl"
    _write_jsonl(
        wire,
        [
            {
                "time": 1,
                "type": "turn.prompt",
                "input": [{"type": "text", "text": "CCB_REQ_ID: job_native123\nhello"}],
            },
            {"time": 2, "type": "assistant.delta", "turnId": 1, "delta": "source-style "},
            {"time": 3, "type": "assistant.delta", "turnId": 1, "delta": "kimi reply"},
            {"time": 4, "type": "turn.ended", "turnId": 1, "reason": "completed"},
        ],
    )

    observed = observe_kimi_turn(work_dir, "job_native123", home_candidates=[home])

    assert observed is not None
    assert observed.completed is True
    assert observed.reply == "source-style kimi reply"


def test_deepseek_observes_session_store_and_poll_emits_boundary(monkeypatch, tmp_path: Path) -> None:
    home = tmp_path / "home"
    work_dir = tmp_path / "project"
    work_dir.mkdir()
    monkeypatch.setenv("HOME", str(home))
    project_root = deepseek_project_root(work_dir, home=home)
    project_root.mkdir(parents=True)
    (project_root / "sessions-index.json").write_text(
        json.dumps({"sessions": [{"id": "sess-1", "status": "completed", "assistantReply": "native deep reply"}]}),
        encoding="utf-8",
    )
    _write_jsonl(
        project_root / "sess-1.jsonl",
        [
            {"id": "u1", "role": "user", "content": "CCB_REQ_ID: job_native123\nhello"},
            {"id": "a1", "role": "assistant", "content": "native deep reply"},
        ],
    )

    observed = observe_deepseek_session(work_dir, "job_native123", home_candidates=[home])

    assert deepseek_project_code(work_dir)
    assert observed is not None
    assert observed.completed is True
    assert observed.reply == "native deep reply"

    result = DeepSeekProviderAdapter().poll(
        _submission(provider="deepseek", source_kind=CompletionSourceKind.SESSION_SNAPSHOT, work_dir=work_dir),
        now="2026-06-13T00:00:05Z",
    )

    assert result is not None
    assert result.decision is None
    assert result.submission.reply == "native deep reply"
    assert result.items[-1].kind is CompletionItemKind.TURN_BOUNDARY
    assert result.items[-1].payload["reason"] == "deepseek_session_completed"


def test_deepseek_completed_empty_reply_is_incomplete(monkeypatch, tmp_path: Path) -> None:
    home = tmp_path / "home"
    work_dir = tmp_path / "project"
    work_dir.mkdir()
    monkeypatch.setenv("HOME", str(home))
    project_root = deepseek_project_root(work_dir, home=home)
    project_root.mkdir(parents=True)
    (project_root / "sessions-index.json").write_text(
        json.dumps({"sessions": [{"id": "sess-1", "status": "completed"}]}),
        encoding="utf-8",
    )
    _write_jsonl(project_root / "sess-1.jsonl", [{"id": "u1", "role": "user", "content": "CCB_REQ_ID: job_native123"}])

    result = DeepSeekProviderAdapter().poll(
        _submission(provider="deepseek", source_kind=CompletionSourceKind.SESSION_SNAPSHOT, work_dir=work_dir),
        now="2026-06-13T00:00:05Z",
    )

    assert result is not None
    assert result.decision is not None
    assert result.decision.status is CompletionStatus.INCOMPLETE
    assert result.decision.reason == "deepseek_native_empty_reply"
    assert result.decision.diagnostics["empty_reply"] is True


def test_deepseek_permission_denied_is_incomplete(monkeypatch, tmp_path: Path) -> None:
    home = tmp_path / "home"
    work_dir = tmp_path / "project"
    work_dir.mkdir()
    monkeypatch.setenv("HOME", str(home))
    project_root = deepseek_project_root(work_dir, home=home)
    project_root.mkdir(parents=True)
    (project_root / "sessions-index.json").write_text(
        json.dumps(
            {
                "sessions": [
                    {
                        "id": "sess-1",
                        "status": "permission_denied",
                        "failReason": "Permission denied by user",
                    }
                ]
            }
        ),
        encoding="utf-8",
    )
    _write_jsonl(
        project_root / "sess-1.jsonl",
        [
            {"id": "u1", "role": "user", "content": "CCB_REQ_ID: job_native123\nhello"},
            {"id": "a1", "role": "assistant", "content": "needs permission"},
        ],
    )

    result = DeepSeekProviderAdapter().poll(
        _submission(provider="deepseek", source_kind=CompletionSourceKind.SESSION_SNAPSHOT, work_dir=work_dir),
        now="2026-06-13T00:00:05Z",
    )

    assert result is not None
    assert result.decision is not None
    assert result.decision.status is CompletionStatus.INCOMPLETE
    assert result.decision.reason == "deepseek_native_permission_denied"
    assert result.decision.diagnostics["native_status"] == "permission_denied"


def test_agy_observes_transcript_and_poll_emits_boundary(tmp_path: Path) -> None:
    home = tmp_path / "home"
    work_dir = tmp_path / "project"
    work_dir.mkdir()
    transcript = home / ".gemini" / "antigravity-cli" / "brain" / "conv-1" / ".system_generated" / "logs" / "transcript.jsonl"
    _write_jsonl(
        transcript,
        [
            {
                "step_index": 1,
                "source": "USER_EXPLICIT",
                "type": "USER_INPUT",
                "status": "DONE",
                "created_at": "2026-06-13T00:00:01Z",
                "content": "CCB_REQ_ID: job_native123\nhello",
            },
            {
                "step_index": 2,
                "source": "MODEL",
                "type": "PLANNER_RESPONSE",
                "status": "DONE",
                "created_at": "2026-06-13T00:00:03Z",
                "content": "native agy reply",
            },
        ],
    )

    observed = observe_agy_transcript(work_dir, "job_native123", home_candidates=[home])

    assert observed is not None
    assert observed.completed is True
    assert observed.reply == "native agy reply"
    assert agy_home_from_start_cmd(f"export HOME={home}; agy") == home

    result = poll_agy_submission(
        _submission(
            provider="agy",
            source_kind=CompletionSourceKind.SESSION_EVENT_LOG,
            work_dir=work_dir,
            extra_state={"agy_home": str(home)},
        ),
        now="2026-06-13T00:00:05Z",
    )

    assert result is not None
    assert result.decision is None
    assert result.submission.reply == "native agy reply"
    assert result.items[-1].kind is CompletionItemKind.TURN_BOUNDARY
    assert result.items[-1].payload["reason"] == "agy_transcript_response_done"
