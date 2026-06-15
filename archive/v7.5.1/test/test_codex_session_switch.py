from __future__ import annotations

import json
import os
from pathlib import Path

from provider_backends.codex.bridge_runtime.binding_runtime import CodexBindingTracker


OLD_ID = "11111111-1111-1111-1111-111111111111"
NEW_ID = "22222222-2222-2222-2222-222222222222"
ALT_ID = "33333333-3333-3333-3333-333333333333"


def test_bridge_tracker_auto_rebinds_unique_managed_candidate(tmp_path: Path, monkeypatch) -> None:
    work_dir, session_file, runtime_dir, old_log = _project(tmp_path)
    new_log = _log(tmp_path, session_id=NEW_ID, work_dir=work_dir, mtime=200)

    monkeypatch.setenv("CCB_SESSION_FILE", str(session_file))
    tracker = CodexBindingTracker(runtime_dir)

    assert tracker.refresh_once() is True

    data = json.loads(session_file.read_text(encoding="utf-8"))
    assert data["codex_session_id"] == NEW_ID
    assert data["codex_session_path"] == str(new_log)
    assert data["old_codex_session_id"] == OLD_ID
    assert data["old_codex_session_path"] == str(old_log)
    assert data["start_cmd"].endswith(f"resume {NEW_ID}")

    switch = json.loads((runtime_dir / "session-switch.json").read_text(encoding="utf-8"))
    assert switch["state"] == "auto_rebound"
    assert switch["committed"] is True


def test_bridge_tracker_rejects_ambiguous_managed_candidates(tmp_path: Path, monkeypatch) -> None:
    work_dir, session_file, runtime_dir, old_log = _project(tmp_path)
    _log(tmp_path, session_id=NEW_ID, work_dir=work_dir, mtime=200)
    _log(tmp_path, session_id=ALT_ID, work_dir=work_dir, mtime=201)

    monkeypatch.setenv("CCB_SESSION_FILE", str(session_file))
    tracker = CodexBindingTracker(runtime_dir)

    assert tracker.refresh_once() is False

    data = json.loads(session_file.read_text(encoding="utf-8"))
    assert data["codex_session_id"] == OLD_ID
    assert data["codex_session_path"] == str(old_log)

    switch = json.loads((runtime_dir / "session-switch.json").read_text(encoding="utf-8"))
    assert switch["state"] == "switched_unbound"
    assert switch["reason"] == "ambiguous_session_candidates"
    assert switch["committed"] is False


def test_bridge_tracker_requires_running_job_anchor_before_rebind(tmp_path: Path, monkeypatch) -> None:
    work_dir, session_file, runtime_dir, old_log = _project(tmp_path)
    jobs_path = tmp_path / "repo" / ".ccb" / "agents" / "agent1" / "jobs.jsonl"
    jobs_path.parent.mkdir(parents=True, exist_ok=True)
    jobs_path.write_text(
        json.dumps({"schema_version": 2, "record_type": "job_record", "job_id": "job_live", "status": "running"})
        + "\n",
        encoding="utf-8",
    )
    new_log = _log(tmp_path, session_id=NEW_ID, work_dir=work_dir, mtime=200)

    monkeypatch.setenv("CCB_SESSION_FILE", str(session_file))
    tracker = CodexBindingTracker(runtime_dir)

    assert tracker.refresh_once() is False
    data = json.loads(session_file.read_text(encoding="utf-8"))
    assert data["codex_session_id"] == OLD_ID
    switch = json.loads((runtime_dir / "session-switch.json").read_text(encoding="utf-8"))
    assert switch["reason"] == "running_job_anchor_not_seen"

    with new_log.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps({"type": "message", "payload": {"text": "CCB_REQ_ID: job_live"}}) + "\n")
    os.utime(new_log, (300, 300))

    assert tracker.refresh_once() is True
    data = json.loads(session_file.read_text(encoding="utf-8"))
    assert data["codex_session_id"] == NEW_ID
    assert data["codex_session_path"] == str(new_log)
    assert data["old_codex_session_path"] == str(old_log)


def _project(tmp_path: Path) -> tuple[Path, Path, Path, Path]:
    work_dir = tmp_path / "repo"
    ccb_dir = work_dir / ".ccb"
    runtime_dir = ccb_dir / "agents" / "agent1" / "provider-runtime" / "codex"
    codex_home = ccb_dir / "agents" / "agent1" / "provider-state" / "codex" / "home"
    runtime_dir.mkdir(parents=True, exist_ok=True)
    (codex_home / "sessions").mkdir(parents=True, exist_ok=True)
    old_log = _log(tmp_path, session_id=OLD_ID, work_dir=work_dir, mtime=100)
    session_file = ccb_dir / ".codex-agent1-session"
    session_file.write_text(
        json.dumps(
            {
                "active": True,
                "agent_name": "agent1",
                "work_dir": str(work_dir),
                "runtime_dir": str(runtime_dir),
                "codex_home": str(codex_home),
                "codex_session_path": str(old_log),
                "codex_session_id": OLD_ID,
                "codex_provider_authority_fingerprint": "fp-1",
                "codex_session_authority_fingerprint": "fp-1",
                "start_cmd": "codex resume " + OLD_ID,
                "codex_start_cmd": "codex resume " + OLD_ID,
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    return work_dir, session_file, runtime_dir, old_log


def _log(tmp_path: Path, *, session_id: str, work_dir: Path, mtime: int) -> Path:
    path = (
        tmp_path
        / "repo"
        / ".ccb"
        / "agents"
        / "agent1"
        / "provider-state"
        / "codex"
        / "home"
        / "sessions"
        / "2026"
        / "04"
        / "29"
        / f"rollout-{session_id}.jsonl"
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps({"type": "session_meta", "payload": {"cwd": str(work_dir)}}) + "\n",
        encoding="utf-8",
    )
    os.utime(path, (mtime, mtime))
    return path
