from __future__ import annotations

import os
import shlex
import subprocess
import textwrap
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
INSTALL_SH = REPO_ROOT / "install.sh"


def _run_root_gate(
    tmp_path: Path,
    *,
    euid: int,
    stdin_tty: bool = False,
    input_text: str = "",
    extra_env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env.update(
        {
            "CCB_LANG": "en",
            "CCB_TEST_EUID": str(euid),
            "HOME": str(tmp_path / "home"),
            "CODEX_INSTALL_PREFIX": str(tmp_path / "install"),
            "CODEX_BIN_DIR": str(tmp_path / "bin"),
        }
    )
    if stdin_tty:
        env["CCB_TEST_STDIN_TTY"] = "1"
    if extra_env:
        env.update(extra_env)
    command = textwrap.dedent(
        f"""
        set -euo pipefail
        source {shlex.quote(str(INSTALL_SH))}
        confirm_root_install_if_needed
        echo gate-passed
        """
    )
    return subprocess.run(
        ["bash", "-lc", command],
        input=input_text,
        capture_output=True,
        text=True,
        env=env,
    )


def _run_main_with_stubs(
    tmp_path: Path,
    action: str,
    *,
    euid: int,
    extra_env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env.update(
        {
            "CCB_LANG": "en",
            "CCB_TEST_EUID": str(euid),
            "HOME": str(tmp_path / "home"),
            "CODEX_INSTALL_PREFIX": str(tmp_path / "install"),
            "CODEX_BIN_DIR": str(tmp_path / "bin"),
        }
    )
    if extra_env:
        env.update(extra_env)
    command = textwrap.dedent(
        f"""
        set -euo pipefail
        source {shlex.quote(str(INSTALL_SH))}
        install_all() {{ echo install-called; }}
        uninstall_all() {{ echo uninstall-called; }}
        main {shlex.quote(action)}
        """
    )
    return subprocess.run(
        ["bash", "-lc", command],
        capture_output=True,
        text=True,
        env=env,
    )


def test_root_gate_allows_non_root_without_prompt(tmp_path: Path) -> None:
    completed = _run_root_gate(tmp_path, euid=1000)

    assert completed.returncode == 0, completed.stderr or completed.stdout
    assert "gate-passed" in completed.stdout
    assert "Root install is not recommended" not in completed.stderr


def test_root_gate_blocks_noninteractive_root_without_override(tmp_path: Path) -> None:
    completed = _run_root_gate(tmp_path, euid=0)

    assert completed.returncode != 0
    assert "Root install is not recommended" in completed.stderr
    assert "Root install requires explicit confirmation" in completed.stderr
    assert "gate-passed" not in completed.stdout


def test_root_gate_allows_noninteractive_root_with_explicit_override(tmp_path: Path) -> None:
    completed = _run_root_gate(tmp_path, euid=0, extra_env={"CCB_ALLOW_ROOT_INSTALL": "1"})

    assert completed.returncode == 0, completed.stderr or completed.stdout
    assert "Continuing root install because CCB_ALLOW_ROOT_INSTALL=1 is set" in completed.stderr
    assert "gate-passed" in completed.stdout


def test_root_gate_interactive_blank_defaults_to_cancel(tmp_path: Path) -> None:
    completed = _run_root_gate(tmp_path, euid=0, stdin_tty=True, input_text="\n")

    assert completed.returncode != 0
    assert "Root install is not recommended" in completed.stderr
    assert "Installation cancelled" in completed.stderr
    assert "gate-passed" not in completed.stdout


def test_root_gate_interactive_yes_continues(tmp_path: Path) -> None:
    completed = _run_root_gate(tmp_path, euid=0, stdin_tty=True, input_text="y\n")

    assert completed.returncode == 0, completed.stderr or completed.stdout
    assert "Root install is not recommended" in completed.stderr
    assert "gate-passed" in completed.stdout


def test_root_gate_reports_sudo_user_risk(tmp_path: Path) -> None:
    completed = _run_root_gate(
        tmp_path,
        euid=0,
        stdin_tty=True,
        input_text="n\n",
        extra_env={"SUDO_USER": "demo"},
    )

    assert completed.returncode != 0
    assert "Detected sudo user: demo" in completed.stderr
    assert "This will not install CCB for demo; it will install for root." in completed.stderr


def test_main_applies_root_confirmation_to_install_only(tmp_path: Path) -> None:
    install = _run_main_with_stubs(tmp_path, "install", euid=0)
    uninstall = _run_main_with_stubs(tmp_path, "uninstall", euid=0)

    assert install.returncode != 0
    assert "Root install is not recommended" in install.stderr
    assert "install-called" not in install.stdout
    assert uninstall.returncode == 0, uninstall.stderr or uninstall.stdout
    assert "Root install is not recommended" not in uninstall.stderr
    assert "uninstall-called" in uninstall.stdout
