from __future__ import annotations

import shutil
import tarfile
from pathlib import Path
from types import SimpleNamespace
from io import StringIO

import pytest

from cli import entrypoint_runtime
from cli.management_runtime import install as install_runtime
from cli.management_runtime.commands_runtime import update as update_runtime


class _TtyOutput(StringIO):
    def isatty(self) -> bool:
        return True


class _PipeOutput(StringIO):
    def isatty(self) -> bool:
        return False


def _clear_post_update_env(monkeypatch) -> None:
    for name in (
        "CCB_INSTALL_ROLES",
        "CCB_INSTALL_NEOVIM",
        "CCB_POST_UPDATE_REQUIRED",
        "CCB_POST_UPDATE_TIMEOUT_SECONDS",
        "CCB_ENTRYPOINT_SMOKE_TIMEOUT_SECONDS",
    ):
        monkeypatch.delenv(name, raising=False)


def test_cmd_update_defaults_to_latest_release(monkeypatch, tmp_path: Path) -> None:
    install_dir = tmp_path / "install"
    install_dir.mkdir()
    tmp_base = tmp_path / "tmp-base"
    tmp_base.mkdir()
    captured: dict[str, object] = {}

    monkeypatch.setenv("CODEX_INSTALL_PREFIX", str(install_dir))
    monkeypatch.setattr(update_runtime.platform, "system", lambda: "Linux")
    monkeypatch.setattr(update_runtime, "pick_temp_base_dir", lambda _install_dir: tmp_base)
    monkeypatch.setattr(update_runtime, "get_available_versions", lambda: ["5.1.0", "5.3.0", "5.2.8"])

    def _fake_update_via_tarball(tmp_base_arg, *, install_dir, target_version, old_info):
        captured["tmp_base"] = tmp_base_arg
        captured["install_dir"] = install_dir
        captured["target_version"] = target_version
        captured["old_info"] = old_info
        return 0

    monkeypatch.setattr(update_runtime, "_update_via_tarball", _fake_update_via_tarball)

    code = update_runtime.cmd_update(SimpleNamespace(target=None), script_root=tmp_path / "script-root")

    assert code == 0
    assert captured["tmp_base"] == tmp_base
    assert captured["install_dir"] == install_dir
    assert captured["target_version"] == "5.3.0"


def test_cmd_update_errors_when_latest_release_cannot_be_resolved(monkeypatch, tmp_path: Path) -> None:
    install_dir = tmp_path / "install"
    install_dir.mkdir()
    tmp_base = tmp_path / "tmp-base"
    tmp_base.mkdir()

    monkeypatch.setenv("CODEX_INSTALL_PREFIX", str(install_dir))
    monkeypatch.setattr(update_runtime.platform, "system", lambda: "Linux")
    monkeypatch.setattr(update_runtime, "pick_temp_base_dir", lambda _install_dir: tmp_base)
    monkeypatch.setattr(update_runtime, "get_available_versions", lambda: [])

    code = update_runtime.cmd_update(SimpleNamespace(target=None), script_root=tmp_path / "script-root")

    assert code == 1


def test_cmd_update_rejects_non_unix_platform(monkeypatch, tmp_path: Path, capsys) -> None:
    monkeypatch.setattr(update_runtime.platform, "system", lambda: "Windows")

    code = update_runtime.cmd_update(SimpleNamespace(target=None), script_root=tmp_path / "script-root")

    assert code == 1
    captured = capsys.readouterr()
    assert "Linux, macOS, or WSL" in captured.out


def test_cmd_update_allows_source_dev_install_and_targets_managed_prefix(monkeypatch, tmp_path: Path, capsys) -> None:
    source_dir = tmp_path / "source-install"
    source_dir.mkdir()
    (source_dir / "install.sh").write_text("#!/usr/bin/env bash\n", encoding="utf-8")
    (source_dir / ".git").mkdir()
    managed_prefix = tmp_path / "managed-install"
    monkeypatch.setattr(update_runtime.platform, "system", lambda: "Linux")
    monkeypatch.setenv("CODEX_INSTALL_PREFIX", str(managed_prefix))
    monkeypatch.setattr(update_runtime, "pick_temp_base_dir", lambda _install_dir: tmp_path / "tmp-base")
    monkeypatch.setattr(update_runtime, "_resolve_latest_release_version", lambda: "6.0.12")
    calls: dict[str, object] = {}

    def _fake_update_via_tarball(tmp_base_arg, *, install_dir, target_version, old_info):
        calls["tmp_base"] = tmp_base_arg
        calls["install_dir"] = install_dir
        calls["target_version"] = target_version
        calls["old_info"] = old_info
        return 0

    monkeypatch.setattr(update_runtime, "_update_via_tarball", _fake_update_via_tarball)
    monkeypatch.setattr(
        update_runtime,
        "get_version_info",
        lambda install_dir: {
            "install_mode": "source" if install_dir == source_dir else "release",
            "source_kind": "source" if install_dir == source_dir else "release",
            "version": "6.0.11",
        },
    )

    code = update_runtime.cmd_update(SimpleNamespace(target=None), script_root=source_dir)

    assert code == 0
    captured = capsys.readouterr()
    assert "source/dev checkout" in captured.out
    assert "Global `ccb` links now target the release install" in captured.out
    assert calls["install_dir"] == managed_prefix
    assert calls["target_version"] == "6.0.12"
    assert calls["old_info"]["install_mode"] == "source"


def test_release_artifact_name_uses_linux_arch_aliases(monkeypatch) -> None:
    monkeypatch.setattr(update_runtime.platform, "system", lambda: "Linux")
    monkeypatch.setattr(update_runtime.platform, "machine", lambda: "amd64")
    assert update_runtime._release_artifact_name() == "ccb-linux-x86_64.tar.gz"

    monkeypatch.setattr(update_runtime.platform, "machine", lambda: "arm64")
    assert update_runtime._release_artifact_name() == "ccb-linux-aarch64.tar.gz"


def test_release_artifact_name_uses_macos_universal_bundle(monkeypatch) -> None:
    monkeypatch.setattr(update_runtime.platform, "system", lambda: "Darwin")
    monkeypatch.setattr(update_runtime.platform, "machine", lambda: "arm64")

    assert update_runtime._release_artifact_name() == "ccb-macos-universal.tar.gz"


def test_release_artifact_url_points_to_release_download() -> None:
    url = update_runtime._release_artifact_url("6.0.0", artifact_name="ccb-linux-x86_64.tar.gz")

    assert url == "https://github.com/bfly123/claude_code_bridge/releases/download/v6.0.0/ccb-linux-x86_64.tar.gz"


def test_release_extract_dir_name_strips_tar_suffixes() -> None:
    assert update_runtime._release_extract_dir_name("ccb-linux-x86_64.tar.gz") == "ccb-linux-x86_64"
    assert update_runtime._release_extract_dir_name("ccb-linux-aarch64.tgz") == "ccb-linux-aarch64"
    assert update_runtime._release_extract_dir_name("ccb-preview.zip") == "ccb-preview"


def test_update_via_tarball_uses_staged_unix_installer(monkeypatch, tmp_path: Path) -> None:
    tmp_base = tmp_path / "tmp-base"
    tmp_base.mkdir()
    install_dir = tmp_path / "install"
    install_dir.mkdir()
    calls: dict[str, object] = {}
    monkeypatch.setattr(update_runtime.platform, "system", lambda: "Linux")
    monkeypatch.setattr(update_runtime.platform, "machine", lambda: "x86_64")

    def _fake_download(_url: str, destination: Path) -> bool:
        extracted_dir = tmp_base / "payload-src"
        extracted_dir.mkdir(parents=True, exist_ok=True)
        (extracted_dir / "install.sh").write_text("#!/usr/bin/env bash\r\nexit 0\r\n", encoding="utf-8")
        with tarfile.open(destination, "w:gz") as archive:
            archive.add(extracted_dir, arcname="ccb-linux-x86_64")
        return True

    monkeypatch.setattr(update_runtime, "download_tarball", _fake_download)
    monkeypatch.setattr(update_runtime, "get_version_info", lambda _install_dir: {"version": "6.0.8"})
    monkeypatch.setattr(update_runtime, "_print_update_outcome", lambda old_info, new_info: None)
    post_update_calls: list[dict[str, object]] = []

    def _fake_run_staged(action: str, *, source_dir: Path, install_dir: Path, extra_env: dict[str, str] | None = None) -> int:
        calls["action"] = action
        calls["source_dir"] = source_dir
        calls["install_dir"] = install_dir
        calls["extra_env"] = dict(extra_env or {})
        return 0

    monkeypatch.setattr(update_runtime, "run_staged_unix_installer", _fake_run_staged)
    monkeypatch.setattr(
        update_runtime,
        "_update_builtin_roles_after_update",
        lambda **_kwargs: (_ for _ in ()).throw(AssertionError("old updater must not update roles directly")),
    )
    monkeypatch.setattr(
        update_runtime,
        "_provision_neovim_after_update",
        lambda: (_ for _ in ()).throw(AssertionError("old updater must not provision neovim directly")),
    )
    monkeypatch.setattr(
        update_runtime,
        "_run_post_update_with_new_entrypoint",
        lambda **kwargs: post_update_calls.append(dict(kwargs)) or True,
    )

    code = update_runtime._update_via_tarball(
        tmp_base,
        install_dir=install_dir,
        target_version="6.0.8",
        old_info={"version": "6.0.7"},
    )

    assert code == 0
    assert calls["action"] == "install"
    assert calls["source_dir"] == tmp_base / "ccb_update" / update_runtime._release_extract_dir_name(update_runtime._release_artifact_name())
    assert calls["install_dir"] == install_dir
    assert calls["extra_env"] == {
        "CODEX_INSTALL_PREFIX": str(install_dir),
        "CCB_CLEAN_INSTALL": "1",
        "CCB_INSTALL_NEOVIM": "0",
        "CCB_INSTALL_ROLES": "0",
    }
    assert post_update_calls == [
        {
            "install_dir": install_dir,
            "old_info": {"version": "6.0.7"},
            "new_info": {"version": "6.0.8"},
        }
    ]


def test_post_update_delegation_runs_installed_entrypoint(monkeypatch, tmp_path: Path) -> None:
    _clear_post_update_env(monkeypatch)
    monkeypatch.delenv("CODEX_BIN_DIR", raising=False)
    install_dir = tmp_path / "install"
    install_dir.mkdir()
    ccb_entry = install_dir / "ccb"
    ccb_entry.write_text("#!/usr/bin/env python3\n", encoding="utf-8")
    calls: list[dict[str, object]] = []

    def _fake_run(command, **kwargs):
        calls.append({"command": list(command), "kwargs": dict(kwargs)})
        return update_runtime.subprocess.CompletedProcess(command, 0)

    monkeypatch.setattr(update_runtime.subprocess, "run", _fake_run)

    ok = update_runtime._run_post_update_with_new_entrypoint(
        install_dir=install_dir,
        old_info={"version": "6.0.7"},
        new_info={"version": "6.0.8"},
    )

    assert ok is True
    assert calls[0]["command"] == [str(ccb_entry), "--print-version"]
    assert calls[1]["command"] == [
        str(ccb_entry),
        update_runtime.POST_UPDATE_COMMAND,
        "--from-version",
        "6.0.7",
        "--to-version",
        "6.0.8",
    ]
    assert calls[1]["kwargs"]["cwd"] == Path.cwd()
    assert calls[1]["kwargs"]["env"]["CODEX_INSTALL_PREFIX"] == str(install_dir)
    assert calls[1]["kwargs"]["env"]["CCB_SKIP_STARTUP_UPDATE_CHECK"] == "1"


def test_post_update_delegation_prefers_current_bin_wrapper(monkeypatch, tmp_path: Path) -> None:
    _clear_post_update_env(monkeypatch)
    monkeypatch.delenv("CODEX_BIN_DIR", raising=False)
    install_dir = tmp_path / "install"
    bin_dir = tmp_path / "bin"
    install_dir.mkdir()
    bin_dir.mkdir()
    (install_dir / "ccb").write_text("#!/usr/bin/env python3\n", encoding="utf-8")
    ccb_entry = bin_dir / "ccb"
    ccb_entry.write_text(f'#!/usr/bin/env bash\nexec /venv/bin/python "{install_dir / "ccb"}" "$@"\n', encoding="utf-8")
    calls: list[dict[str, object]] = []

    def _fake_run(command, **kwargs):
        calls.append({"command": list(command), "kwargs": dict(kwargs)})
        return update_runtime.subprocess.CompletedProcess(command, 0)

    monkeypatch.setattr(update_runtime.sys, "argv", [str(ccb_entry), "update"])
    monkeypatch.setattr(update_runtime.subprocess, "run", _fake_run)

    ok = update_runtime._run_post_update_with_new_entrypoint(
        install_dir=install_dir,
        old_info={"version": "6.0.7"},
        new_info={"version": "6.0.8"},
    )

    assert ok is True
    assert calls[0]["command"] == [str(ccb_entry), "--print-version"]
    assert calls[1]["command"][0] == str(ccb_entry)


def test_post_update_delegation_honors_codex_bin_dir(monkeypatch, tmp_path: Path) -> None:
    _clear_post_update_env(monkeypatch)
    install_dir = tmp_path / "install"
    bin_dir = tmp_path / "custom-bin"
    install_dir.mkdir()
    bin_dir.mkdir()
    (install_dir / "ccb").write_text("#!/usr/bin/env python3\n", encoding="utf-8")
    ccb_entry = bin_dir / "ccb"
    # Explicit CODEX_BIN_DIR is authoritative and intentionally bypasses
    # install_dir wrapper target detection.
    ccb_entry.write_text("#!/usr/bin/env bash\n", encoding="utf-8")
    calls: list[list[str]] = []

    def _fake_run(command, **kwargs):
        calls.append(list(command))
        return update_runtime.subprocess.CompletedProcess(command, 0)

    monkeypatch.setenv("CODEX_BIN_DIR", str(bin_dir))
    monkeypatch.setattr(update_runtime.subprocess, "run", _fake_run)

    ok = update_runtime._run_post_update_with_new_entrypoint(
        install_dir=install_dir,
        old_info={"version": "6.0.7"},
        new_info={"version": "6.0.8"},
    )

    assert ok is True
    assert calls[0] == [str(ccb_entry), "--print-version"]
    assert calls[1][0] == str(ccb_entry)


def test_post_update_delegation_warns_without_failing_core_update(monkeypatch, tmp_path: Path, capsys) -> None:
    _clear_post_update_env(monkeypatch)
    monkeypatch.delenv("CODEX_BIN_DIR", raising=False)
    install_dir = tmp_path / "install"
    install_dir.mkdir()
    ccb_entry = install_dir / "ccb"
    ccb_entry.write_text("#!/usr/bin/env python3\n", encoding="utf-8")

    def _fake_run(command, **kwargs):
        if list(command)[-1] == "--print-version":
            return update_runtime.subprocess.CompletedProcess(command, 0)
        return update_runtime.subprocess.CompletedProcess(command, 17)

    monkeypatch.setattr(update_runtime.subprocess, "run", _fake_run)

    ok = update_runtime._run_post_update_with_new_entrypoint(
        install_dir=install_dir,
        old_info={"version": "6.0.7"},
        new_info={"version": "6.0.8"},
    )

    captured = capsys.readouterr()
    assert ok is True
    assert "Post-update provisioning exited with code 17" in captured.out
    assert "Core update completed" in captured.out


def test_post_update_delegation_timeout_warns_without_failing_core_update(monkeypatch, tmp_path: Path, capsys) -> None:
    _clear_post_update_env(monkeypatch)
    monkeypatch.delenv("CODEX_BIN_DIR", raising=False)
    monkeypatch.setenv("CCB_POST_UPDATE_TIMEOUT_SECONDS", "1")
    install_dir = tmp_path / "install"
    install_dir.mkdir()
    ccb_entry = install_dir / "ccb"
    ccb_entry.write_text("#!/usr/bin/env python3\n", encoding="utf-8")

    def _fake_run(command, **kwargs):
        if list(command)[-1] == "--print-version":
            return update_runtime.subprocess.CompletedProcess(command, 0)
        raise update_runtime.subprocess.TimeoutExpired(command, kwargs.get("timeout"))

    monkeypatch.setattr(update_runtime.subprocess, "run", _fake_run)

    ok = update_runtime._run_post_update_with_new_entrypoint(
        install_dir=install_dir,
        old_info={"version": "6.0.7"},
        new_info={"version": "6.0.8"},
    )

    captured = capsys.readouterr()
    assert ok is True
    assert "Post-update provisioning timed out after 1s" in captured.out
    assert "Core update completed" in captured.out


def test_post_update_required_failure_fails_update(monkeypatch, tmp_path: Path, capsys) -> None:
    _clear_post_update_env(monkeypatch)
    monkeypatch.delenv("CODEX_BIN_DIR", raising=False)
    monkeypatch.setenv("CCB_INSTALL_NEOVIM", "1")
    install_dir = tmp_path / "install"
    install_dir.mkdir()
    ccb_entry = install_dir / "ccb"
    ccb_entry.write_text("#!/usr/bin/env python3\n", encoding="utf-8")

    def _fake_run(command, **kwargs):
        if list(command)[-1] == "--print-version":
            return update_runtime.subprocess.CompletedProcess(command, 0)
        return update_runtime.subprocess.CompletedProcess(command, 17)

    monkeypatch.setattr(update_runtime.subprocess, "run", _fake_run)

    ok = update_runtime._run_post_update_with_new_entrypoint(
        install_dir=install_dir,
        old_info={"version": "6.0.7"},
        new_info={"version": "6.0.8"},
    )

    captured = capsys.readouterr()
    assert ok is False
    assert "Required post-update provisioning exited with code 17" in captured.out


@pytest.mark.parametrize("required_env", ["CCB_INSTALL_ROLES", "CCB_POST_UPDATE_REQUIRED"])
def test_post_update_required_roles_catalog_unavailable_returns_failure(
    monkeypatch,
    tmp_path: Path,
    capsys,
    required_env: str,
) -> None:
    _clear_post_update_env(monkeypatch)
    monkeypatch.setenv(required_env, "1")
    monkeypatch.setattr(
        update_runtime,
        "role_catalog_status",
        lambda **_kwargs: (_ for _ in ()).throw(RuntimeError("catalog down")),
    )
    monkeypatch.setattr(update_runtime, "_provision_neovim_after_update", lambda: None)
    monkeypatch.setattr(update_runtime, "set_tmux_ui_active", lambda active: None)

    code = update_runtime._run_post_update_provisioning(install_dir=tmp_path / "install")

    captured = capsys.readouterr()
    assert code == 1
    assert "Agent Roles catalog unavailable" in captured.out


def test_post_update_refreshes_tmux_ui_without_affecting_provisioning(monkeypatch, tmp_path: Path, capsys) -> None:
    _clear_post_update_env(monkeypatch)
    calls: list[bool] = []
    monkeypatch.setenv("CCB_INSTALL_ROLES", "0")
    monkeypatch.setenv("CCB_INSTALL_NEOVIM", "0")
    monkeypatch.setattr(update_runtime, "set_tmux_ui_active", lambda active: calls.append(active))

    code = update_runtime._run_post_update_provisioning(install_dir=tmp_path / "install")

    captured = capsys.readouterr()
    assert code == 0
    assert calls == [True]
    assert "Tmux UI post-update refresh skipped" not in captured.out


def test_post_update_tmux_ui_refresh_failure_is_non_blocking(monkeypatch, tmp_path: Path, capsys) -> None:
    _clear_post_update_env(monkeypatch)
    monkeypatch.setenv("CCB_INSTALL_ROLES", "0")
    monkeypatch.setenv("CCB_INSTALL_NEOVIM", "0")
    monkeypatch.setattr(
        update_runtime,
        "set_tmux_ui_active",
        lambda active: (_ for _ in ()).throw(RuntimeError("tmux unavailable")),
    )

    code = update_runtime._run_post_update_provisioning(install_dir=tmp_path / "install")

    captured = capsys.readouterr()
    assert code == 0
    assert "Tmux UI post-update refresh skipped: RuntimeError: tmux unavailable" in captured.out


def test_post_update_required_installed_role_update_failure_returns_failure(monkeypatch, tmp_path: Path, capsys) -> None:
    _clear_post_update_env(monkeypatch)
    monkeypatch.setenv("CCB_INSTALL_ROLES", "1")
    rows = (
        {
            "role_id": "agentroles.archi",
            "status": "update_available",
            "version": "0.2.0",
            "installed_version": "0.1.0",
        },
    )
    monkeypatch.setattr(update_runtime, "role_catalog_status", lambda **_kwargs: rows)
    monkeypatch.setattr(update_runtime, "cmd_roles", lambda *_args, **_kwargs: 42)
    monkeypatch.setattr(update_runtime, "_provision_neovim_after_update", lambda: None)

    code = update_runtime._run_post_update_provisioning(install_dir=tmp_path / "install")

    captured = capsys.readouterr()
    assert code == 1
    assert "Role Pack update failed: agentroles.archi" in captured.out
    assert "Role Pack updates had 1 failure" in captured.out


def test_post_update_required_selected_new_role_install_failure_returns_failure(
    monkeypatch,
    tmp_path: Path,
    capsys,
) -> None:
    _clear_post_update_env(monkeypatch)
    monkeypatch.setenv("CCB_INSTALL_ROLES", "1")
    rows = (
        {
            "role_id": "agentroles.ccb_self",
            "status": "available",
            "version": "0.1.0",
            "name": "CCB Self",
            "description": "Maintains CCB projects.",
        },
        {
            "role_id": "agentroles.new",
            "status": "available",
            "version": "0.1.0",
            "name": "New Role",
            "description": "New catalog role.",
        },
    )

    class _TtyInput:
        def isatty(self) -> bool:
            return True

        def readline(self) -> str:
            return "1\n"

    stdout = _TtyOutput()
    monkeypatch.setattr(update_runtime.sys, "stdin", _TtyInput())
    monkeypatch.setattr(update_runtime.sys, "stdout", stdout)
    monkeypatch.setattr(update_runtime, "role_catalog_status", lambda **_kwargs: rows)
    monkeypatch.setattr(update_runtime, "cmd_roles", lambda *_args, **_kwargs: 42)
    monkeypatch.setattr(update_runtime, "_provision_neovim_after_update", lambda: None)

    code = update_runtime._run_post_update_provisioning(install_dir=tmp_path / "install")

    captured = capsys.readouterr()
    assert code == 1
    output = stdout.getvalue() + captured.out
    assert "Role Pack install failed: agentroles.new" in output
    assert "Role Pack installs had 1 failure" in output


def test_post_update_required_default_role_install_failure_returns_failure(
    monkeypatch,
    tmp_path: Path,
    capsys,
) -> None:
    _clear_post_update_env(monkeypatch)
    monkeypatch.setenv("CCB_INSTALL_ROLES", "1")
    rows = (
        {
            "role_id": "agentroles.ccb_self",
            "status": "available",
            "version": "0.1.0",
            "name": "CCB Self",
            "description": "Maintains CCB projects.",
        },
    )
    monkeypatch.setattr(update_runtime, "role_catalog_status", lambda **_kwargs: rows)
    monkeypatch.setattr(update_runtime, "cmd_roles", lambda *_args, **_kwargs: 42)
    monkeypatch.setattr(update_runtime, "_provision_neovim_after_update", lambda: None)

    code = update_runtime._run_post_update_provisioning(install_dir=tmp_path / "install")

    captured = capsys.readouterr()
    assert code == 1
    assert "Default Role Pack install failed: agentroles.ccb_self" in captured.out
    assert "Default Role Pack installs had 1 failure" in captured.out


def test_post_update_internal_command_runs_new_process_provisioning(monkeypatch, tmp_path: Path) -> None:
    install_dir = tmp_path / "install"
    calls: list[dict[str, object]] = []

    monkeypatch.setattr(
        update_runtime,
        "_update_builtin_roles_after_update",
        lambda **kwargs: calls.append({"roles": dict(kwargs)}),
    )
    monkeypatch.setattr(
        update_runtime,
        "_provision_neovim_after_update",
        lambda: calls.append({"neovim": True}),
    )

    code = update_runtime.maybe_handle_post_update_command(
        [update_runtime.POST_UPDATE_COMMAND, "--from-version", "6.0.7", "--to-version", "6.0.8"],
        script_root=install_dir,
    )

    assert code == 0
    assert calls == [{"roles": {"install_dir": install_dir}}, {"neovim": True}]


def test_entrypoint_routes_internal_post_update_command(monkeypatch, tmp_path: Path) -> None:
    calls: list[dict[str, object]] = []

    monkeypatch.setattr(
        entrypoint_runtime,
        "maybe_handle_post_update_command",
        lambda tokens, *, script_root: calls.append({"tokens": list(tokens), "script_root": script_root}) or 0,
    )

    code = entrypoint_runtime.run_cli_entrypoint(
        [update_runtime.POST_UPDATE_COMMAND, "--from-version", "6.0.7"],
        version="6.0.8",
        script_root=tmp_path / "install",
        cwd=tmp_path,
        stdout=StringIO(),
        stderr=StringIO(),
    )

    assert code == 0
    assert calls == [
        {
            "tokens": [update_runtime.POST_UPDATE_COMMAND, "--from-version", "6.0.7"],
            "script_root": tmp_path / "install",
        }
    ]


def test_post_update_required_env_accepts_roles_without_prompt(monkeypatch, tmp_path: Path) -> None:
    _clear_post_update_env(monkeypatch)
    monkeypatch.setenv("CCB_POST_UPDATE_REQUIRED", "1")
    rows = (
        {
            "role_id": "agentroles.archi",
            "status": "current",
            "version": "0.2.0",
            "installed_version": "0.2.0",
        },
    )

    class _TtyInput:
        def isatty(self) -> bool:
            return True

        def readline(self) -> str:
            raise AssertionError("required post-update roles should not prompt")

    stdout = _TtyOutput()
    monkeypatch.setattr(update_runtime.sys, "stdin", _TtyInput())
    monkeypatch.setattr(update_runtime.sys, "stdout", stdout)
    monkeypatch.setattr(update_runtime, "role_catalog_status", lambda **_kwargs: rows)
    monkeypatch.setattr(
        update_runtime,
        "cmd_roles",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError("current roles should not update")),
    )

    code = update_runtime._update_builtin_roles_after_update(install_dir=tmp_path / "install")

    assert code == 0
    assert "Refresh installed Agent Roles" not in stdout.getvalue()
    assert "Installed Role Packs already match the catalog" in stdout.getvalue()


def test_update_roles_prompt_accepts_interactive_default(monkeypatch, tmp_path: Path) -> None:
    calls: list[dict[str, object]] = []
    rows = (
        {
            "role_id": "agentroles.archi",
            "status": "update_available",
            "version": "0.2.0",
            "installed_version": "0.1.0",
            "name": "Architecture Reviewer",
            "description": "Reviews architecture drift.",
        },
        {
            "role_id": "agentroles.ccb_self",
            "status": "available",
            "version": "0.1.0",
            "name": "CCB Self",
            "description": "Maintains CCB projects.",
        },
        {
            "role_id": "agentroles.new",
            "status": "available",
            "version": "0.1.0",
            "name": "New Role",
            "description": "New catalog role.",
        },
    )

    class _TtyInput:
        def isatty(self) -> bool:
            return True

        def readline(self) -> str:
            return "\n"

    monkeypatch.setattr(update_runtime.sys, "stdin", _TtyInput())
    stdout = _TtyOutput()
    monkeypatch.setattr(update_runtime.sys, "stdout", stdout)
    monkeypatch.setattr(update_runtime, "role_catalog_status", lambda **_kwargs: rows)

    def _fake_cmd_roles(argv, *, script_root, cwd, stdout, stderr):
        calls.append({"argv": argv, "script_root": script_root, "cwd": cwd})
        print("role_status: updated", file=stdout)
        return 0

    monkeypatch.setattr(update_runtime, "cmd_roles", _fake_cmd_roles)

    update_runtime._update_builtin_roles_after_update(install_dir=tmp_path / "install")

    assert calls == [
        {"argv": ["update", "agentroles.archi"], "script_root": tmp_path / "install", "cwd": Path.cwd()},
        {"argv": ["install", "agentroles.ccb_self"], "script_root": tmp_path / "install", "cwd": Path.cwd()},
    ]
    assert "Refresh installed and recommended Agent Roles from the catalog now?" in stdout.getvalue()
    assert "Role Pack updated: agentroles.archi" in stdout.getvalue()
    assert "Default Role Pack installed: agentroles.ccb_self" in stdout.getvalue()
    assert "New Agent Roles available" in stdout.getvalue()
    assert "agentroles.new v0.1.0" in stdout.getvalue()


def test_update_roles_current_status_does_not_run_update_hooks(monkeypatch, tmp_path: Path) -> None:
    rows = (
        {
            "role_id": "agentroles.archi",
            "status": "current",
            "version": "0.2.0",
            "installed_version": "0.2.0",
            "name": "Architecture Reviewer",
            "description": "Reviews architecture drift.",
        },
    )

    class _TtyInput:
        def isatty(self) -> bool:
            return True

        def readline(self) -> str:
            return "\n"

    monkeypatch.setattr(update_runtime.sys, "stdin", _TtyInput())
    stdout = _TtyOutput()
    monkeypatch.setattr(update_runtime.sys, "stdout", stdout)
    monkeypatch.setattr(update_runtime, "role_catalog_status", lambda **_kwargs: rows)
    monkeypatch.setattr(
        update_runtime,
        "cmd_roles",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError("must not update current roles")),
    )

    update_runtime._update_builtin_roles_after_update(install_dir=tmp_path / "install")

    assert "Installed Role Packs already match the catalog" in stdout.getvalue()


def test_update_roles_prompt_declines_without_update(monkeypatch, tmp_path: Path) -> None:
    rows = (
        {
            "role_id": "agentroles.ccb_self",
            "status": "available",
            "version": "0.1.0",
            "name": "CCB Self",
            "description": "Maintains CCB projects.",
        },
        {
            "role_id": "agentroles.new",
            "status": "available",
            "version": "0.1.0",
            "name": "New Role",
            "description": "New catalog role.",
        },
    )

    class _TtyInput:
        def isatty(self) -> bool:
            return True

        def readline(self) -> str:
            return "n\n"

    monkeypatch.setattr(update_runtime.sys, "stdin", _TtyInput())
    stdout = _TtyOutput()
    monkeypatch.setattr(update_runtime.sys, "stdout", stdout)
    monkeypatch.setattr(update_runtime, "role_catalog_status", lambda **_kwargs: rows)
    monkeypatch.setattr(
        update_runtime,
        "cmd_roles",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError("must not update roles")),
    )

    update_runtime._update_builtin_roles_after_update(install_dir=tmp_path / "install")

    assert "Role Pack update skipped" in stdout.getvalue()
    assert "Recommended Agent Roles available" in stdout.getvalue()
    assert "agentroles.ccb_self v0.1.0" in stdout.getvalue()
    assert "agentroles.new v0.1.0" in stdout.getvalue()


def test_update_roles_noninteractive_skips_without_prompt(monkeypatch, tmp_path: Path) -> None:
    rows = (
        {
            "role_id": "agentroles.ccb_self",
            "status": "available",
            "version": "0.1.0",
            "name": "CCB Self",
            "description": "Maintains CCB projects.",
        },
        {
            "role_id": "agentroles.new",
            "status": "available",
            "version": "0.1.0",
            "name": "New Role",
            "description": "New catalog role.",
        },
    )

    class _PipeInput:
        def isatty(self) -> bool:
            return False

    monkeypatch.setattr(update_runtime.sys, "stdin", _PipeInput())
    stdout = _PipeOutput()
    monkeypatch.setattr(update_runtime.sys, "stdout", stdout)
    monkeypatch.setattr(update_runtime, "role_catalog_status", lambda **_kwargs: rows)
    monkeypatch.setattr(
        update_runtime,
        "cmd_roles",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError("must not update roles")),
    )

    update_runtime._update_builtin_roles_after_update(install_dir=tmp_path / "install")

    assert "non-interactive update" in stdout.getvalue()
    assert "Recommended Agent Roles available" in stdout.getvalue()
    assert "agentroles.ccb_self v0.1.0" in stdout.getvalue()
    assert "agentroles.new v0.1.0" in stdout.getvalue()


def test_update_roles_new_catalog_role_selection_parser() -> None:
    rows = [
        {"role_id": "agentroles.one"},
        {"role_id": "agentroles.two"},
        {"role_id": "agentroles.three"},
    ]

    assert update_runtime._select_catalog_role_ids("", rows) == ()
    assert update_runtime._select_catalog_role_ids("2", rows) == ("agentroles.two",)
    assert update_runtime._select_catalog_role_ids("1, 3", rows) == ("agentroles.one", "agentroles.three")
    assert update_runtime._select_catalog_role_ids("all", rows) == (
        "agentroles.one",
        "agentroles.two",
        "agentroles.three",
    )
    assert update_runtime._select_catalog_role_ids("agentroles.two", rows) == ("agentroles.two",)


def test_update_neovim_prompt_accepts_interactive_yes(monkeypatch, capsys) -> None:
    calls: list[dict[str, object]] = []

    class _TtyInput:
        def isatty(self) -> bool:
            return True

        def readline(self) -> str:
            return "y\n"

    monkeypatch.delenv("CCB_INSTALL_NEOVIM", raising=False)
    monkeypatch.setattr(update_runtime.sys, "stdin", _TtyInput())
    stdout = _TtyOutput()
    monkeypatch.setattr(update_runtime.sys, "stdout", stdout)
    monkeypatch.setattr(
        update_runtime,
        "provision_neovim",
        lambda *, required=False: calls.append({"required": required}) or {"status": "ok", "wrapper": "/tmp/ccb-nvim"},
    )

    update_runtime._provision_neovim_after_update()

    assert calls == [{"required": False}]
    assert "Install/refresh the default Neovim + LazyVim tool window now?" in stdout.getvalue()
    assert "Neovim tool ready" in stdout.getvalue()


def test_post_update_required_env_makes_neovim_required_without_prompt(monkeypatch) -> None:
    _clear_post_update_env(monkeypatch)
    monkeypatch.setenv("CCB_POST_UPDATE_REQUIRED", "1")

    class _TtyInput:
        def isatty(self) -> bool:
            return True

        def readline(self) -> str:
            raise AssertionError("required post-update neovim should not prompt")

    monkeypatch.setattr(update_runtime.sys, "stdin", _TtyInput())
    monkeypatch.setattr(update_runtime.sys, "stdout", _TtyOutput())

    assert update_runtime._neovim_install_choice() == "required"


def test_update_neovim_prompt_declines_without_provisioning(monkeypatch, capsys) -> None:
    class _TtyInput:
        def isatty(self) -> bool:
            return True

        def readline(self) -> str:
            return "\n"

    monkeypatch.delenv("CCB_INSTALL_NEOVIM", raising=False)
    monkeypatch.setattr(update_runtime.sys, "stdin", _TtyInput())
    stdout = _TtyOutput()
    monkeypatch.setattr(update_runtime.sys, "stdout", stdout)
    monkeypatch.setattr(
        update_runtime,
        "provision_neovim",
        lambda **_kwargs: (_ for _ in ()).throw(AssertionError("must not provision")),
    )

    update_runtime._provision_neovim_after_update()

    assert "Run `ccb tools install neovim` later" in stdout.getvalue()


def test_update_neovim_noninteractive_skips_without_prompt(monkeypatch, capsys) -> None:
    class _PipeInput:
        def isatty(self) -> bool:
            return False

    monkeypatch.delenv("CCB_INSTALL_NEOVIM", raising=False)
    monkeypatch.setattr(update_runtime.sys, "stdin", _PipeInput())
    stdout = _PipeOutput()
    monkeypatch.setattr(update_runtime.sys, "stdout", stdout)
    monkeypatch.setattr(
        update_runtime,
        "provision_neovim",
        lambda **_kwargs: (_ for _ in ()).throw(AssertionError("must not provision")),
    )

    update_runtime._provision_neovim_after_update()

    assert "non-interactive update" in stdout.getvalue()


def test_update_neovim_env_forces_required_install(monkeypatch) -> None:
    calls: list[dict[str, object]] = []
    monkeypatch.setenv("CCB_INSTALL_NEOVIM", "1")
    monkeypatch.setattr(
        update_runtime,
        "provision_neovim",
        lambda *, required=False: calls.append({"required": required}) or {"status": "ok", "wrapper": "/tmp/ccb-nvim"},
    )

    update_runtime._provision_neovim_after_update()

    assert calls == [{"required": True}]


def test_update_neovim_env_skip_does_not_provision(monkeypatch) -> None:
    monkeypatch.setenv("CCB_INSTALL_NEOVIM", "0")
    monkeypatch.setattr(
        update_runtime,
        "provision_neovim",
        lambda **_kwargs: (_ for _ in ()).throw(AssertionError("must not provision")),
    )

    update_runtime._provision_neovim_after_update()


def test_update_neovim_optional_failure_is_soft(monkeypatch) -> None:
    class _TtyInput:
        def isatty(self) -> bool:
            return True

        def readline(self) -> str:
            return "yes\n"

    monkeypatch.delenv("CCB_INSTALL_NEOVIM", raising=False)
    monkeypatch.setattr(update_runtime.sys, "stdin", _TtyInput())
    stdout = _TtyOutput()
    monkeypatch.setattr(update_runtime.sys, "stdout", stdout)
    monkeypatch.setattr(
        update_runtime,
        "provision_neovim",
        lambda **_kwargs: (_ for _ in ()).throw(RuntimeError("network unavailable")),
    )

    update_runtime._provision_neovim_after_update()

    assert "Neovim tool not ready" in stdout.getvalue()
    assert "network unavailable" in stdout.getvalue()


def test_update_neovim_required_failure_is_hard(monkeypatch) -> None:
    monkeypatch.setenv("CCB_INSTALL_NEOVIM", "1")
    monkeypatch.setattr(
        update_runtime,
        "provision_neovim",
        lambda **_kwargs: (_ for _ in ()).throw(RuntimeError("checksum mismatch")),
    )

    with pytest.raises(RuntimeError, match="checksum mismatch"):
        update_runtime._provision_neovim_after_update()


def test_update_via_tarball_uses_macos_release_artifact(monkeypatch, tmp_path: Path) -> None:
    tmp_base = tmp_path / "tmp-base"
    tmp_base.mkdir()
    install_dir = tmp_path / "install"
    install_dir.mkdir()
    calls: dict[str, object] = {}
    monkeypatch.setattr(update_runtime.platform, "system", lambda: "Darwin")
    monkeypatch.setattr(update_runtime.platform, "machine", lambda: "arm64")

    def _fake_download(_url: str, destination: Path) -> bool:
        extracted_dir = tmp_base / "payload-src"
        extracted_dir.mkdir(parents=True, exist_ok=True)
        (extracted_dir / "install.sh").write_text("#!/usr/bin/env bash\r\nexit 0\r\n", encoding="utf-8")
        with tarfile.open(destination, "w:gz") as archive:
            archive.add(extracted_dir, arcname="ccb-macos-universal")
        calls["downloaded_to"] = destination
        return True

    monkeypatch.setattr(update_runtime, "download_tarball", _fake_download)
    monkeypatch.setattr(update_runtime, "get_version_info", lambda _install_dir: {"version": "6.0.8"})
    monkeypatch.setattr(update_runtime, "_print_update_outcome", lambda old_info, new_info: None)
    monkeypatch.setattr(update_runtime, "_run_post_update_with_new_entrypoint", lambda **_kwargs: True)

    def _fake_run_staged(action: str, *, source_dir: Path, install_dir: Path, extra_env: dict[str, str] | None = None) -> int:
        calls["action"] = action
        calls["source_dir"] = source_dir
        calls["install_dir"] = install_dir
        return 0

    monkeypatch.setattr(update_runtime, "run_staged_unix_installer", _fake_run_staged)

    code = update_runtime._update_via_tarball(
        tmp_base,
        install_dir=install_dir,
        target_version="6.0.8",
        old_info={"version": "6.0.7"},
    )

    assert code == 0
    assert str(calls["downloaded_to"]).endswith("ccb-macos-universal.tar.gz")
    assert calls["action"] == "install"
    assert calls["source_dir"] == tmp_base / "ccb_update" / "ccb-macos-universal"
    assert calls["install_dir"] == install_dir


def test_staged_unix_installer_preserves_binary_sidebar_helper(tmp_path: Path) -> None:
    source_dir = tmp_path / "ccb-macos-universal"
    bin_dir = source_dir / "bin"
    bin_dir.mkdir(parents=True)
    (source_dir / "install.sh").write_bytes(b"#!/usr/bin/env bash\r\necho install\r\n")
    (bin_dir / "ask").write_bytes(b"#!/usr/bin/env bash\r\necho ask\r\n")
    sidebar_bytes = b"\xca\xfe\xba\xbe\x00\x00\x00\x02\r\nbinary\rpayload\x00"
    (bin_dir / "ccb-agent-sidebar").write_bytes(sidebar_bytes)

    staging_root, staged_source = install_runtime._stage_unix_installer_tree(source_dir, temp_base=tmp_path)
    try:
        assert (staged_source / "install.sh").read_bytes() == b"#!/usr/bin/env bash\necho install\n"
        assert (staged_source / "bin" / "ask").read_bytes() == b"#!/usr/bin/env bash\necho ask\n"
        assert (staged_source / "bin" / "ccb-agent-sidebar").read_bytes() == sidebar_bytes
    finally:
        shutil.rmtree(staging_root, ignore_errors=True)
