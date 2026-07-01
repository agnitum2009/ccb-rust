from __future__ import annotations

import json
from pathlib import Path

import pytest

from completion.models import CompletionSourceKind
from provider_backends.codex.comm_runtime.log_reader_facade import CodexLogReader
from provider_execution.base import ProviderSubmission


OLD_ID = "11111111-1111-1111-1111-111111111111"
NEW_ID = "22222222-2222-2222-2222-222222222222"
JOB_ID = "job_abc123"


def _make_session(tmp_path: Path, agent_name: str) -> object:
    from provider_backends.codex.session import CodexProjectSession

    work_dir = tmp_path / "repo"
    ccb_dir = work_dir / ".ccb"
    sessions = ccb_dir / "agents" / agent_name / "provider-state" / "codex" / "home" / "sessions"
    sessions.mkdir(parents=True, exist_ok=True)
    session_file = ccb_dir / f".codex-{agent_name}-session"
    data = {
        "agent_name": agent_name,
        "work_dir": str(work_dir),
        "work_dir_norm": str(work_dir),
        "workspace_path": str(work_dir),
        "codex_home": str(ccb_dir / "agents" / agent_name / "provider-state" / "codex" / "home"),
        "codex_session_root": str(sessions),
    }
    return CodexProjectSession(session_file=session_file, data=data)


def _write_log(path: Path, lines: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(line, ensure_ascii=False) + "\n" for line in lines), encoding="utf-8")


@pytest.fixture
def stale_binding(tmp_path: Path):
    from provider_backends.codex.session import CodexProjectSession

    agent_name = "agent1"
    session = _make_session(tmp_path, agent_name)
    assert isinstance(session, CodexProjectSession)
    sessions = Path(session.data["codex_session_root"])  # type: ignore[arg-type]

    old_log = sessions / f"rollout-{OLD_ID}.jsonl"
    _write_log(
        old_log,
        [
            {
                "type": "event_msg",
                "payload": {"type": "task_complete", "turn_id": OLD_ID, "last_agent_message": "done"},
            }
        ],
    )

    new_log = sessions / f"rollout-{NEW_ID}.jsonl"
    _write_log(
        new_log,
        [
            {
                "type": "session_meta",
                "payload": {"cwd": str(tmp_path / "repo"), "session_id": NEW_ID},
            },
            {
                "type": "event_msg",
                "payload": {
                    "type": "user_message",
                    "message": f"CCB_REQ_ID: {JOB_ID}\n\nhello",
                },
            },
        ],
    )

    session.data["codex_session_path"] = str(old_log)
    session.data["codex_session_id"] = OLD_ID
    return session, old_log, new_log


def test_refresh_reader_switches_to_anchor_fallback_log(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, stale_binding
) -> None:
    from provider_backends.codex import execution as execution_module
    from provider_backends.codex.execution import _refresh_reader_for_current_session_binding

    session, old_log, new_log = stale_binding
    work_dir = Path(session.data["work_dir"])  # type: ignore[arg-type]

    monkeypatch.setattr(execution_module, "_load_session", lambda _wd, _agent: session)

    reader = CodexLogReader(
        root=Path(session.data["codex_session_root"]),  # type: ignore[arg-type]
        log_path=old_log,
        session_id_filter=OLD_ID,
        work_dir=work_dir,
        follow_workspace_sessions=False,
    )
    runtime_state: dict[str, object] = {
        "mode": "active",
        "state": {"log_path": str(old_log), "offset": old_log.stat().st_size, "last_rescan": 0.0},
        "reader": reader,
        "request_anchor": JOB_ID,
        "anchor_seen": False,
        "delivery_state": "pending_anchor",
        "workspace_path": str(work_dir),
    }
    submission = ProviderSubmission(
        job_id=JOB_ID,
        agent_name="agent1",
        provider="codex",
        accepted_at="",
        ready_at="",
        source_kind=CompletionSourceKind.PROTOCOL_EVENT_STREAM,
        reply="",
        runtime_state=runtime_state,
    )

    updated = _refresh_reader_for_current_session_binding(submission)

    updated_state = updated.runtime_state["state"]
    assert Path(updated_state["log_path"]) == new_log  # type: ignore[arg-type]
    assert updated_state["offset"] == 0
    updated_reader = updated.runtime_state["reader"]
    assert updated_reader._session_id_filter == NEW_ID


def test_delivery_acceptance_guard_fails_when_fallback_candidate_never_switched(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, stale_binding
) -> None:
    from provider_backends.codex import execution as execution_module
    from provider_backends.codex.execution import _delivery_acceptance_guard

    session, old_log, _new_log = stale_binding
    work_dir = Path(session.data["work_dir"])  # type: ignore[arg-type]

    monkeypatch.setattr(execution_module, "_load_session", lambda _wd, _agent: session)

    started_at = "2026-07-01T00:00:00Z"
    now = "2026-07-01T00:05:00Z"
    runtime_state: dict[str, object] = {
        "mode": "active",
        "state": {"log_path": str(old_log), "offset": old_log.stat().st_size, "last_rescan": 0.0},
        "request_anchor": JOB_ID,
        "anchor_seen": False,
        "delivery_state": "pending_anchor",
        "delivery_target_pane_id": "%0",
        "delivery_started_at": started_at,
        "delivery_timeout_s": 120.0,
        "workspace_path": str(work_dir),
    }
    submission = ProviderSubmission(
        job_id=JOB_ID,
        agent_name="agent1",
        provider="codex",
        accepted_at=started_at,
        ready_at=started_at,
        source_kind=CompletionSourceKind.PROTOCOL_EVENT_STREAM,
        reply="",
        runtime_state=runtime_state,
    )

    result = _delivery_acceptance_guard(submission, now=now)

    assert result is not None
    assert result.submission.runtime_state["delivery_state"] == "failed"
    assert result.submission.diagnostics["delivery_failure_kind"] == "delivery_anchor_missing"


def test_delivery_acceptance_guard_suppressed_when_already_on_active_fallback(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, stale_binding
) -> None:
    from provider_backends.codex import execution as execution_module
    from provider_backends.codex.execution import _delivery_acceptance_guard

    session, old_log, new_log = stale_binding
    work_dir = Path(session.data["work_dir"])  # type: ignore[arg-type]

    monkeypatch.setattr(execution_module, "_load_session", lambda _wd, _agent: session)

    started_at = "2026-07-01T00:00:00Z"
    now = "2026-07-01T00:05:00Z"
    runtime_state: dict[str, object] = {
        "mode": "active",
        "state": {"log_path": str(new_log), "offset": 0, "last_rescan": 0.0},
        "request_anchor": JOB_ID,
        "anchor_seen": False,
        "delivery_state": "pending_anchor",
        "delivery_target_pane_id": "%0",
        "delivery_started_at": started_at,
        "delivery_timeout_s": 120.0,
        "workspace_path": str(work_dir),
        "codex_anchor_fallback_log": str(new_log),
        "codex_anchor_fallback_session_id": NEW_ID,
    }
    submission = ProviderSubmission(
        job_id=JOB_ID,
        agent_name="agent1",
        provider="codex",
        accepted_at=started_at,
        ready_at=started_at,
        source_kind=CompletionSourceKind.PROTOCOL_EVENT_STREAM,
        reply="",
        runtime_state=runtime_state,
    )

    result = _delivery_acceptance_guard(submission, now=now)

    assert result is None


def test_delivery_acceptance_guard_suppressed_when_current_log_not_drained(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, stale_binding
) -> None:
    from provider_backends.codex import execution as execution_module
    from provider_backends.codex.execution import _delivery_acceptance_guard

    session, old_log, _new_log = stale_binding
    work_dir = Path(session.data["work_dir"])  # type: ignore[arg-type]

    monkeypatch.setattr(execution_module, "_load_session", lambda _wd, _agent: session)

    started_at = "2026-07-01T00:00:00Z"
    now = "2026-07-01T00:05:00Z"
    runtime_state: dict[str, object] = {
        "mode": "active",
        "state": {"log_path": str(old_log), "offset": 0, "last_rescan": 0.0},
        "request_anchor": JOB_ID,
        "anchor_seen": False,
        "delivery_state": "pending_anchor",
        "delivery_target_pane_id": "%0",
        "delivery_started_at": started_at,
        "delivery_timeout_s": 120.0,
        "workspace_path": str(work_dir),
    }
    submission = ProviderSubmission(
        job_id=JOB_ID,
        agent_name="agent1",
        provider="codex",
        accepted_at=started_at,
        ready_at=started_at,
        source_kind=CompletionSourceKind.PROTOCOL_EVENT_STREAM,
        reply="",
        runtime_state=runtime_state,
    )

    result = _delivery_acceptance_guard(submission, now=now)

    assert result is None
