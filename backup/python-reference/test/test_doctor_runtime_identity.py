from __future__ import annotations

from pathlib import Path

from cli.render_runtime.ops_views_doctor import render_doctor
from cli.services.doctor_runtime import system


def test_runtime_identity_summary_reports_root_project_owner_warning(tmp_path: Path, monkeypatch) -> None:
    project_root = tmp_path / "project"
    ccb_dir = project_root / ".ccb"
    install_dir = tmp_path / "install"
    ccb_dir.mkdir(parents=True)
    install_dir.mkdir()

    monkeypatch.setattr(system, "_effective_uid", lambda: 0)
    monkeypatch.setattr(system, "_user_name", lambda uid: "root" if uid == 0 else "demo")

    def fake_path_owner(path: Path):
        if path == project_root or path == ccb_dir:
            return {"uid": 1000, "name": "demo"}
        if path == install_dir:
            return {"uid": 0, "name": "root"}
        return None

    monkeypatch.setattr(system, "_path_owner", fake_path_owner)

    payload = system.runtime_identity_summary(
        project_root,
        ccb_dir=ccb_dir,
        installation={
            "path": str(install_dir),
            "root_install": True,
            "install_user_id": "0",
            "install_user_name": "root",
            "sudo_user": "demo",
        },
    )

    assert payload["user_id"] == 0
    assert payload["user_name"] == "root"
    assert payload["root_runtime"] is True
    assert payload["install_root_owned"] is True
    assert payload["project_owner"] == "1000:demo"
    assert payload["ccb_dir_owner"] == "1000:demo"
    assert payload["install_owner"] == "0:root"
    assert payload["sudo_user"] == "demo"
    assert payload["warnings"] == (
        "Running CCB as root in a non-root-owned project can create root-owned .ccb files.",
    )


def test_render_doctor_includes_root_runtime_identity_lines() -> None:
    payload = {
        "project": "/tmp/repo",
        "project_id": "proj-1",
        "installation": {
            "path": "/tmp/install",
            "install_mode": "release",
            "source_kind": "release",
            "version": "7.2.1",
            "channel": "stable",
            "build_time": "2026-06-03T00:00:00Z",
            "platform": "linux",
            "arch": "x86_64",
        },
        "runtime": {
            "user_id": 0,
            "user_name": "root",
            "home": "/root",
            "root_runtime": True,
            "install_root_owned": True,
            "install_user_id": 0,
            "install_user_name": "root",
            "sudo_user": "demo",
            "project_owner": "1000:demo",
            "ccb_dir_owner": "1000:demo",
            "install_owner": "0:root",
            "warnings": (
                "Running CCB as root in a non-root-owned project can create root-owned .ccb files.",
            ),
        },
        "requirements": {
            "python_executable": "/usr/bin/python3",
            "python_version": "3.12.0",
            "tmux_available": True,
            "tmux_path": "/usr/bin/tmux",
            "provider_commands": (),
        },
        "ccbd": {
            "state": "unmounted",
            "health": "unknown",
            "generation": 0,
            "last_heartbeat_at": None,
            "pid_alive": False,
            "socket_connectable": False,
            "heartbeat_fresh": False,
            "takeover_allowed": True,
            "reason": "not_started",
            "active_execution_count": 0,
            "recoverable_execution_count": 0,
            "nonrecoverable_execution_count": 0,
            "pending_items_count": 0,
            "terminal_pending_count": 0,
            "recoverable_execution_providers": [],
            "nonrecoverable_execution_providers": [],
            "diagnostic_errors": (),
        },
        "agents": [],
    }

    lines = render_doctor(payload)

    assert "user_id: 0" in lines
    assert "user_name: root" in lines
    assert "home: /root" in lines
    assert "root_runtime: True" in lines
    assert "install_root_owned: True" in lines
    assert "sudo_user: demo" in lines
    assert "project_owner: 1000:demo" in lines
    assert "ccb_dir_owner: 1000:demo" in lines
    assert (
        "runtime_warning: Running CCB as root in a non-root-owned project can create root-owned .ccb files."
        in lines
    )
