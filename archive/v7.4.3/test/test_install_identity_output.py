from __future__ import annotations

import os
import json
import shlex
import subprocess
import textwrap
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
INSTALL_SH = REPO_ROOT / "install.sh"


def _render_install_identity(*, source_kind: str, channel: str, version: str = "9.9.9") -> str:
    env = os.environ.copy()
    env.update(
        {
            "CCB_LANG": "en",
            "CCB_SOURCE_KIND": source_kind,
            "CCB_BUILD_CHANNEL": channel,
            "CCB_BUILD_VERSION": version,
        }
    )
    command = textwrap.dedent(
        f"""
        set -euo pipefail
        source {shlex.quote(str(INSTALL_SH))}
        print_install_identity_summary
        print_install_identity_notice
        """
    )
    completed = subprocess.run(
        ["bash", "-lc", command],
        check=True,
        capture_output=True,
        text=True,
        env=env,
    )
    return completed.stdout


def test_source_install_identity_output_is_explicit() -> None:
    output = _render_install_identity(source_kind="source", channel="dev", version="1.2.3")

    assert "install_mode=source" in output
    assert "source_kind=source" in output
    assert "channel=dev" in output
    assert "version=1.2.3" in output
    assert "Development/source install detected" in output
    assert "This is a development install, not an official release package." in output


def test_release_install_identity_output_is_explicit() -> None:
    output = _render_install_identity(source_kind="release", channel="stable", version="2.0.0")

    assert "install_mode=release" in output
    assert "source_kind=release" in output
    assert "channel=stable" in output
    assert "version=2.0.0" in output
    assert "Official release package install detected" in output


def test_preview_release_install_identity_is_not_misreported_as_source() -> None:
    output = _render_install_identity(source_kind="preview", channel="preview", version="2.0.0-preview")

    assert "install_mode=release" in output
    assert "source_kind=preview" in output
    assert "channel=preview" in output
    assert "version=2.0.0-preview" in output
    assert "Preview release package install detected" in output
    assert "not an official stable release" in output


def test_write_install_metadata_avoids_bash4_parameter_expansion(tmp_path: Path) -> None:
    install_prefix = tmp_path / "install"
    install_prefix.mkdir()
    (install_prefix / "ccb").write_text(
        'VERSION = "0.0.0"\nGIT_COMMIT = "abc123"\nGIT_DATE = "2026-05-04"\n',
        encoding="utf-8",
    )
    env = os.environ.copy()
    env.update(
        {
            "CODEX_INSTALL_PREFIX": str(install_prefix),
            "CCB_BUILD_VERSION": '6.0.26-"quoted"',
            "CCB_BUILD_CHANNEL": "stable",
            "CCB_SOURCE_KIND": "release",
            "CCB_BUILD_TIME": "2026-05-04T00:00:00Z",
            "CCB_TEST_EUID": "1000",
            "CCB_TEST_USER_NAME": "runner",
        }
    )
    command = textwrap.dedent(
        f"""
        set -euo pipefail
        source {shlex.quote(str(INSTALL_SH))}
        write_install_metadata
        """
    )

    completed = subprocess.run(
        ["bash", "-lc", command],
        capture_output=True,
        text=True,
        env=env,
    )

    assert completed.returncode == 0, completed.stderr or completed.stdout
    assert "${version@Q}" not in INSTALL_SH.read_text(encoding="utf-8")
    payload = json.loads((install_prefix / "BUILD_INFO.json").read_text(encoding="utf-8"))
    assert payload["version"] == '6.0.26-"quoted"'
    assert payload["source_kind"] == "release"
    assert payload["install_mode"] == "release"
    assert payload["root_install"] is False
    assert "install_user_id" in payload
    assert "install_user_name" in payload


def test_write_install_metadata_records_root_profile(tmp_path: Path) -> None:
    install_prefix = tmp_path / "install"
    install_prefix.mkdir()
    (install_prefix / "ccb").write_text(
        'VERSION = "0.0.0"\nGIT_COMMIT = "abc123"\nGIT_DATE = "2026-05-04"\n',
        encoding="utf-8",
    )
    env = os.environ.copy()
    env.update(
        {
            "CODEX_INSTALL_PREFIX": str(install_prefix),
            "CCB_BUILD_VERSION": "6.0.26",
            "CCB_BUILD_CHANNEL": "stable",
            "CCB_SOURCE_KIND": "release",
            "CCB_BUILD_TIME": "2026-05-04T00:00:00Z",
            "CCB_TEST_EUID": "0",
            "CCB_TEST_USER_NAME": "root",
            "SUDO_USER": "demo",
        }
    )
    command = textwrap.dedent(
        f"""
        set -euo pipefail
        source {shlex.quote(str(INSTALL_SH))}
        write_install_metadata
        """
    )

    completed = subprocess.run(
        ["bash", "-lc", command],
        capture_output=True,
        text=True,
        env=env,
    )

    assert completed.returncode == 0, completed.stderr or completed.stdout
    payload = json.loads((install_prefix / "BUILD_INFO.json").read_text(encoding="utf-8"))
    assert payload["install_user_id"] == 0
    assert payload["install_user_name"] == "root"
    assert payload["root_install"] is True
    assert payload["sudo_user"] == "demo"
