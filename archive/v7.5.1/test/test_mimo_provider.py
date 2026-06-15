from __future__ import annotations

import json
import sqlite3
from pathlib import Path
import sys
import time
from types import SimpleNamespace

from ccbd.api_models import DeliveryScope, JobRecord, JobStatus, MessageEnvelope
from completion.models import CompletionItemKind, CompletionSourceKind, CompletionStatus
from provider_backends.mimo.execution import MimoProviderAdapter
from provider_backends.mimo.launcher import materialize_mimo_memory_config
from provider_backends.mimo.runtime import MimoLogReader, default_mimo_storage_root
from provider_execution.base import ProviderRuntimeContext, ProviderSubmission


def _init_mimocode_db(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with sqlite3.connect(path) as conn:
        conn.executescript(
            """
            CREATE TABLE session (
                id TEXT PRIMARY KEY,
                directory TEXT NOT NULL,
                time_updated INTEGER NOT NULL
            );
            CREATE TABLE message (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL,
                data TEXT NOT NULL
            );
            CREATE TABLE part (
                id TEXT PRIMARY KEY,
                message_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL,
                data TEXT NOT NULL
            );
            """
        )


def test_mimo_log_reader_reads_completed_reply_from_mimocode_sqlite(tmp_path: Path) -> None:
    project_dir = tmp_path / "repo"
    project_dir.mkdir()
    root = tmp_path / "mimocode" / "data" / "storage"
    db_path = tmp_path / "mimocode" / "data" / "mimocode.db"
    _init_mimocode_db(db_path)

    with sqlite3.connect(db_path) as conn:
        conn.execute(
            "INSERT INTO session (id, directory, time_updated) VALUES (?, ?, ?)",
            ("ses_mimo", str(project_dir), 2000),
        )
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?)",
            (
                "msg_user",
                "ses_mimo",
                1000,
                1000,
                json.dumps({"id": "msg_user", "role": "user", "time": {"created": 1000}}, ensure_ascii=True),
            ),
        )
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
            (
                "prt_user",
                "msg_user",
                "ses_mimo",
                1000,
                1000,
                json.dumps({"type": "text", "text": "CCB_REQ_ID: job_mimo\n\nhello"}, ensure_ascii=True),
            ),
        )
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?)",
            (
                "msg_assistant",
                "ses_mimo",
                1001,
                1002,
                json.dumps(
                    {
                        "id": "msg_assistant",
                        "parentID": "msg_user",
                        "role": "assistant",
                        "time": {"created": 1001, "completed": 1002},
                        "finish": "stop",
                    },
                    ensure_ascii=True,
                ),
            ),
        )
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?, ?, ?, ?, ?, ?)",
            (
                "prt_assistant",
                "msg_assistant",
                "ses_mimo",
                1001,
                1002,
                json.dumps({"type": "text", "text": "MIMO_DONE", "time": {"start": 1001, "end": 1002}}, ensure_ascii=True),
            ),
        )
        conn.commit()

    reader = MimoLogReader(root=root, work_dir=project_dir, project_id="global")
    state = reader.capture_state()

    assert state["session_id"] == "ses_mimo"
    assert state["last_assistant_req_id"] == "job_mimo"
    assert state["last_assistant_completed"] == 1002
    reply, next_state = reader.try_get_message({"assistant_count": 0})
    assert reply == "MIMO_DONE"
    assert next_state["last_assistant_req_id"] == "job_mimo"


def test_mimo_default_storage_root_uses_mimocode_home(tmp_path: Path) -> None:
    home = tmp_path / "mimo-home"

    assert default_mimo_storage_root({"MIMOCODE_HOME": str(home)}) == home / "data" / "storage"


def test_mimo_memory_config_materializes_memory_and_ask_instruction(tmp_path: Path) -> None:
    project_root = tmp_path / "repo"
    workspace = project_root
    (project_root / ".ccb").mkdir(parents=True)
    (project_root / ".ccb" / "ccb_memory.md").write_text("shared mimo memory\n", encoding="utf-8")
    config_path = project_root / ".ccb" / "agents" / "mimo1" / "provider-state" / "mimo" / "mimocode.json"
    event_path = project_root / ".ccb" / "agents" / "mimo1" / "events.jsonl"
    marker_path = project_root / ".ccb" / "agents" / "mimo1" / "provider-runtime" / "mimo" / "mimo-memory-projection.json"
    profile = SimpleNamespace(inherit_memory=True, inherit_skills=True)

    result = materialize_mimo_memory_config(
        project_root=project_root,
        agent_name="mimo1",
        workspace_path=workspace,
        config_path=config_path,
        profile=profile,
        event_path=event_path,
        marker_path=marker_path,
    )

    assert result.env == {"MIMOCODE_CONFIG": str(config_path)}
    config = json.loads(config_path.read_text(encoding="utf-8"))
    assert config["autoupdate"] is False
    assert config["instructions"] == [
        ".ccb/runtime/memory/mimo1.md",
        ".ccb/runtime/skills/mimo1/mimo/ask.md",
    ]
    assert (project_root / ".ccb" / "runtime" / "memory" / "mimo1.md").is_file()
    assert (project_root / ".ccb" / "runtime" / "skills" / "mimo1" / "mimo" / "ask.md").is_file()
    events = [json.loads(line) for line in event_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    assert events[-1]["event_type"] == "mimo_memory_projection_ok"


def _job(work_dir: Path) -> JobRecord:
    return JobRecord(
        job_id="job_mimo_run123",
        submission_id="sub_mimo",
        agent_name="mimo1",
        provider="mimo",
        request=MessageEnvelope(
            project_id="proj",
            to_agent="mimo1",
            from_actor="main",
            body="Reply exactly: MIMO_OK",
            task_id=None,
            reply_to=None,
            message_type="ask",
            delivery_scope=DeliveryScope.SINGLE,
        ),
        status=JobStatus.RUNNING,
        terminal_decision=None,
        cancel_requested_at=None,
        created_at="2026-06-13T00:00:00Z",
        updated_at="2026-06-13T00:00:00Z",
        workspace_path=str(work_dir),
    )


def _runtime_context(work_dir: Path) -> ProviderRuntimeContext:
    return ProviderRuntimeContext(
        agent_name="mimo1",
        workspace_path=str(work_dir),
        backend_type="pane-backed",
        runtime_ref="%1",
        session_ref=str(work_dir / ".mimo-mimo1-session"),
    )


def _write_mimo_session(work_dir: Path, *, home: Path, config: Path) -> None:
    session = {
        "active": True,
        "agent_name": "mimo1",
        "runtime_dir": str(work_dir / ".ccb" / "agents" / "mimo1" / "provider-runtime" / "mimo"),
        "completion_artifact_dir": str(work_dir / ".ccb" / "agents" / "mimo1" / "provider-runtime" / "mimo" / "completion"),
        "work_dir": str(work_dir),
        "mimo_home": str(home),
        "mimo_config_path": str(config),
        "pane_id": "%1",
    }
    (work_dir / ".mimo-mimo1-session").write_text(json.dumps(session), encoding="utf-8")


def test_mimo_adapter_completes_from_native_run_json(monkeypatch, tmp_path: Path) -> None:
    work_dir = tmp_path / "repo"
    work_dir.mkdir()
    home = tmp_path / "mimo-home"
    config = tmp_path / "mimocode.json"
    config.write_text(json.dumps({"autoupdate": False}), encoding="utf-8")
    _write_mimo_session(work_dir, home=home, config=config)
    monkeypatch.setenv("MIMO_START_CMD", f"{sys.executable} {Path('test/stubs/provider_stub.py').resolve()} --provider mimo")

    adapter = MimoProviderAdapter()
    submission = adapter.start(_job(work_dir), context=_runtime_context(work_dir), now="2026-06-13T00:00:00Z")

    assert submission.source_kind is CompletionSourceKind.STRUCTURED_RESULT_STREAM
    assert submission.runtime_state["mode"] == "mimo_run"
    assert submission.runtime_state["pure_mode"] is True
    terminal = None
    emitted: list[CompletionItemKind] = []
    current = submission
    for _ in range(150):
        result = adapter.poll(current, now="2026-06-13T00:00:01Z")
        if result is not None:
            current = result.submission
            emitted.extend(item.kind for item in result.items)
            if result.decision is not None:
                terminal = result
                break
        time.sleep(0.05)

    assert terminal is not None
    assert terminal.decision is not None
    assert terminal.decision.status is CompletionStatus.COMPLETED
    assert terminal.decision.reason == "mimo_run_stop"
    assert terminal.decision.reply == "stub reply for job_mimo_run123"
    assert emitted == [
        CompletionItemKind.ANCHOR_SEEN,
        CompletionItemKind.ASSISTANT_FINAL,
        CompletionItemKind.TURN_BOUNDARY,
    ]


def test_mimo_adapter_reports_empty_native_run_reply(tmp_path: Path) -> None:
    stdout = tmp_path / "run.jsonl"
    stdout.write_text(json.dumps({"type": "step_finish", "reason": "stop"}) + "\n", encoding="utf-8")
    submission = ProviderSubmission(
        job_id="job_empty",
        agent_name="mimo1",
        provider="mimo",
        accepted_at="2026-06-13T00:00:00Z",
        ready_at="2026-06-13T00:00:00Z",
        source_kind=CompletionSourceKind.STRUCTURED_RESULT_STREAM,
        reply="",
        runtime_state={
            "mode": "mimo_run",
            "request_anchor": "job_empty",
            "stdout_path": str(stdout),
            "stderr_path": str(tmp_path / "stderr.log"),
            "next_seq": 1,
            "anchor_emitted": False,
            "returncode": 0,
        },
    )

    result = MimoProviderAdapter().poll(submission, now="2026-06-13T00:00:01Z")

    assert result is not None
    assert result.decision is not None
    assert result.decision.status is CompletionStatus.INCOMPLETE
    assert result.decision.reason == "mimo_run_empty_reply"


def test_mimo_adapter_treats_tool_calls_finish_as_intermediate(tmp_path: Path) -> None:
    stdout = tmp_path / "run.jsonl"
    stdout.write_text(
        "\n".join(
            [
                json.dumps({"type": "step_finish", "part": {"type": "step-finish", "reason": "tool-calls"}}),
                json.dumps({"type": "step_start", "part": {"type": "step-start"}}),
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    submission = ProviderSubmission(
        job_id="job_tool_calls",
        agent_name="mimo1",
        provider="mimo",
        accepted_at="2026-06-13T00:00:00Z",
        ready_at="2026-06-13T00:00:00Z",
        source_kind=CompletionSourceKind.STRUCTURED_RESULT_STREAM,
        reply="",
        runtime_state={
            "mode": "mimo_run",
            "request_anchor": "job_tool_calls",
            "stdout_path": str(stdout),
            "stderr_path": str(tmp_path / "stderr.log"),
            "next_seq": 1,
            "anchor_emitted": False,
            "returncode": None,
        },
    )

    active = MimoProviderAdapter().poll(submission, now="2026-06-13T00:00:01Z")

    assert active is not None
    assert active.decision is None

    active.submission.runtime_state["returncode"] = 0
    terminal = MimoProviderAdapter().poll(active.submission, now="2026-06-13T00:00:02Z")

    assert terminal is not None
    assert terminal.decision is not None
    assert terminal.decision.status is CompletionStatus.INCOMPLETE
    assert terminal.decision.reason == "mimo_run_finished:tool-calls"
