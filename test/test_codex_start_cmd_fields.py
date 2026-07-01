from __future__ import annotations

from provider_backends.codex.start_cmd_runtime.fields import (
    effective_start_cmd,
    persist_resume_start_cmd_fields,
    resume_template_command,
)


def test_effective_start_cmd_rebuilds_resume_from_base_start_command() -> None:
    data = {
        "start_cmd": "export CODEX_RUNTIME_DIR=/tmp/demo; codex -c disable_paste_burst=true",
        "codex_start_cmd": "codex resume stale-session",
        "codex_session_id": "fresh-session",
    }

    assert effective_start_cmd(data) == (
        "export CODEX_RUNTIME_DIR=/tmp/demo; "
        "codex -c disable_paste_burst=true resume fresh-session"
    )


def test_persist_resume_start_cmd_fields_updates_both_stored_fields() -> None:
    data = {
        "start_cmd": "export CODEX_RUNTIME_DIR=/tmp/demo; codex -c disable_paste_burst=true",
    }

    updated = persist_resume_start_cmd_fields(data, "resume-session")

    assert updated == (
        "export CODEX_RUNTIME_DIR=/tmp/demo; "
        "codex -c disable_paste_burst=true resume resume-session"
    )
    assert data["codex_start_cmd"] == updated
    assert data["start_cmd"] == updated


def test_resume_template_command_prefers_non_resume_base_command() -> None:
    data = {
        "start_cmd": "export CODEX_RUNTIME_DIR=/tmp/demo; codex -c disable_paste_burst=true",
        "codex_start_cmd": "codex resume stale-session",
    }

    assert resume_template_command(data) == "export CODEX_RUNTIME_DIR=/tmp/demo; codex -c disable_paste_burst=true"



def test_effective_start_cmd_honors_fresh_restore_mode() -> None:
    """When the agent is configured for fresh starts, effective_start_cmd must not
    silently append a resume subcommand even if a session_id is present."""
    data = {
        "start_cmd": "export CODEX_RUNTIME_DIR=/tmp/demo; codex -c disable_paste_burst=true",
        "codex_start_cmd": "",
        "codex_session_id": "some-session",
        "codex_restore_mode": "fresh",
    }

    assert effective_start_cmd(data) == (
        "export CODEX_RUNTIME_DIR=/tmp/demo; "
        "codex -c disable_paste_burst=true"
    )


def test_persist_resume_start_cmd_fields_skips_resume_when_fresh() -> None:
    """Session switch binding must not rewrite a fresh agent's start command into
    a resume command."""
    data = {
        "start_cmd": "export CODEX_RUNTIME_DIR=/tmp/demo; codex -c disable_paste_burst=true",
        "codex_restore_mode": "fresh",
    }

    result = persist_resume_start_cmd_fields(data, "resume-session")

    assert result is None
    assert data["start_cmd"] == (
        "export CODEX_RUNTIME_DIR=/tmp/demo; "
        "codex -c disable_paste_burst=true"
    )
    assert "codex_start_cmd" not in data


def test_persist_resume_start_cmd_fields_updates_resume_when_provider_mode() -> None:
    """Provider-resume agents still get their resume session_id updated on switch."""
    data = {
        "start_cmd": "export CODEX_RUNTIME_DIR=/tmp/demo; codex -c disable_paste_burst=true",
        "codex_restore_mode": "provider",
    }

    result = persist_resume_start_cmd_fields(data, "resume-session")

    assert result == (
        "export CODEX_RUNTIME_DIR=/tmp/demo; "
        "codex -c disable_paste_burst=true resume resume-session"
    )
    assert data["codex_start_cmd"] == result
    assert data["start_cmd"] == result
