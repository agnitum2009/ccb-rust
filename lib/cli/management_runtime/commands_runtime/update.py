from __future__ import annotations

from pathlib import Path
import os
import platform
import re
import shutil
import sys
import tarfile

from release_artifacts import release_artifact_name
from cli.roles_runtime.commands import cmd_roles
from cli.tools_runtime.neovim import provision_neovim

from ..install import (
    download_tarball,
    is_source_repo_root,
    pick_temp_base_dir,
    resolve_managed_install_dir,
    run_staged_unix_installer,
    safe_extract_tar,
)
from ..versioning import REPO_URL, format_version_info, get_available_versions, get_version_info
from .matching import find_matching_version, latest_version


def cmd_update(args, *, script_root: Path) -> int:
    supported, reason = _supported_update_platform()
    if not supported:
        print(reason)
        return 1
    source_repo_install = is_source_repo_root(script_root)
    install_dir = resolve_managed_install_dir(script_root=script_root)

    target_version = _resolve_target_version(args)
    if target_version is False:
        return 1

    current_install_root = script_root if source_repo_install else install_dir
    old_info = get_version_info(current_install_root)
    if target_version:
        if source_repo_install:
            print(f"🔄 Installing release v{target_version} from source/dev checkout...")
        else:
            print(f"🔄 Updating to v{target_version}...")
    else:
        if source_repo_install:
            print("🔄 Checking latest stable release for source/dev checkout...")
        else:
            print("🔄 Checking for release updates...")

    try:
        tmp_base = pick_temp_base_dir(install_dir)
    except Exception as exc:
        print(str(exc))
        return 1
    resolved_target = target_version or _resolve_latest_release_version()
    if not resolved_target:
        print("❌ Could not determine latest release version")
        return 1
    code = _update_via_tarball(tmp_base, install_dir=install_dir, target_version=resolved_target, old_info=old_info)
    if code != 0:
        return code
    if source_repo_install:
        print(f"ℹ️  Global `ccb` links now target the release install at: {install_dir}")
        print("   `./ccb` inside the source checkout still runs the live source tree.")
    return 0


def _resolve_target_version(args) -> str | bool | None:
    if not hasattr(args, "target") or not args.target:
        return None
    target_spec = args.target.lstrip("v")
    if not re.match(r"^\d+(\.\d+)*$", target_spec):
        print(f"❌ Invalid version format: {args.target}")
        print("   Examples: ccb update 4, ccb update 4.1, ccb update 4.1.3")
        return False
    print(f"🔍 Looking for version matching: {target_spec}")
    versions = get_available_versions()
    if not versions:
        print("❌ Could not fetch available versions")
        return False
    target_version = find_matching_version(target_spec, versions)
    if not target_version:
        ordered = sorted(versions, key=lambda item: [int(x) for x in item.split(".")], reverse=True)[:10]
        print(f"❌ No version found matching '{target_spec}'")
        print(f"   Available: {', '.join(ordered)}")
        return False
    print(f"📌 Target version: v{target_version}")
    return target_version


def _supported_update_platform() -> tuple[bool, str | None]:
    system_name = platform.system()
    if system_name in {"Linux", "Darwin"}:
        return True, None
    return (
        False,
        "❌ `ccb update` is currently supported only on Linux/macOS/WSL.\n"
        "   Please use a Linux, macOS, or WSL runtime, or reinstall manually on this platform.",
    )


def _resolve_latest_release_version() -> str | None:
    versions = get_available_versions()
    return latest_version(versions)


def _update_via_tarball(tmp_base: Path, *, install_dir: Path, target_version: str | None, old_info: dict[str, object]) -> int:
    if not target_version:
        print("❌ Update failed: no release version selected")
        return 1
    artifact_name = _release_artifact_name()
    if not artifact_name:
        print(
            "❌ Update failed: unsupported release artifact target "
            f"for platform '{platform.system()}' architecture '{platform.machine()}'"
        )
        return 1
    tarball_url = _release_artifact_url(target_version, artifact_name=artifact_name)
    extracted_name = artifact_name

    tmp_dir = tmp_base / "ccb_update"
    try:
        print(f"📥 Downloading v{target_version}...")
        if tmp_dir.exists():
            shutil.rmtree(tmp_dir)
        tmp_dir.mkdir(parents=True, exist_ok=True)
        tarball_path = tmp_dir / artifact_name
        if not download_tarball(tarball_url, tarball_path):
            print("❌ Update failed: unable to download release tarball")
            return 1

        print("📂 Extracting...")
        with tarfile.open(tarball_path, "r:gz") as tar:
            safe_extract_tar(tar, tmp_dir)
        extracted_dir = tmp_dir / _release_extract_dir_name(extracted_name)

        print("🔧 Installing...")
        returncode = run_staged_unix_installer(
            "install",
            source_dir=extracted_dir,
            install_dir=install_dir,
            extra_env={
                "CODEX_INSTALL_PREFIX": str(install_dir),
                "CCB_CLEAN_INSTALL": "1",
                "CCB_INSTALL_NEOVIM": "0",
                "CCB_INSTALL_ROLES": "0",
            },
        )
        if returncode != 0:
            print(f"❌ Update failed: installer exited with code {returncode}")
            return returncode

        new_info = get_version_info(install_dir)
        _print_update_outcome(old_info, new_info)
        _update_builtin_roles_after_update(install_dir=install_dir)
        _provision_neovim_after_update()
        return 0
    except Exception as exc:
        print(f"❌ Update failed: {exc}")
        return 1
    finally:
        if tmp_dir.exists():
            shutil.rmtree(tmp_dir, ignore_errors=True)


def _print_update_outcome(old_info: dict[str, object], new_info: dict[str, object]) -> None:
    old_str = format_version_info(old_info)
    new_str = format_version_info(new_info)
    if old_info.get("commit") != new_info.get("commit") or old_info.get("version") != new_info.get("version"):
        print(f"✅ Updated: {old_str} → {new_str}")
    else:
        print(f"✅ Already up to date: {new_str}")


def _provision_neovim_after_update() -> None:
    choice = _neovim_install_choice()
    if choice == 'env-skip':
        print('ℹ️  Neovim tool provisioning skipped by CCB_INSTALL_NEOVIM=0')
        return
    if choice == 'declined':
        print('ℹ️  Neovim/LazyVim provisioning skipped.')
        print('   Run `ccb tools install neovim` later to enable the default neovim window.')
        return
    if choice == 'noninteractive-skip':
        print('ℹ️  Neovim/LazyVim provisioning skipped in non-interactive update.')
        print('   Run `ccb tools install neovim` later to enable the default neovim window.')
        return
    required = choice == 'required'
    try:
        result = provision_neovim(required=required)
    except Exception as exc:
        if required:
            raise
        print(f"⚠️  Neovim tool not ready: {type(exc).__name__}: {exc}")
        return
    status = str(result.get('status') or '')
    if status == 'ok':
        print(f"✅ Neovim tool ready: {result.get('wrapper')}")
    elif required:
        raise RuntimeError(str(result.get('reason') or 'Neovim tool provisioning failed'))
    else:
        print(f"⚠️  Neovim tool not ready: {result.get('reason') or status}")


def _update_builtin_roles_after_update(*, install_dir: Path) -> None:
    choice = _roles_update_choice()
    if choice == 'declined':
        print('ℹ️  Role Pack update skipped.')
        print('   Run `ccb roles update ccb.archi` later to refresh roles and dependencies.')
        return
    if choice == 'noninteractive-skip':
        print('ℹ️  Role Pack update skipped in non-interactive update.')
        print('   Run `ccb roles update ccb.archi` later to refresh roles and dependencies.')
        return
    stdout = sys.stdout
    stderr = sys.stderr
    code = cmd_roles(['update', 'ccb.archi'], script_root=install_dir, cwd=Path.cwd(), stdout=stdout, stderr=stderr)
    if code == 0:
        print('✅ Role Pack ready: ccb.archi')
    else:
        print('⚠️  Role Pack update failed: ccb.archi')


def _roles_update_choice() -> str:
    if not _stream_is_tty(sys.stdin) or not _stream_is_tty(sys.stdout):
        return 'noninteractive-skip'
    print('Install/refresh bundled Role Packs and dependencies now? [Y/n] ', end='', flush=True)
    try:
        answer = sys.stdin.readline()
    except Exception:
        return 'noninteractive-skip'
    if str(answer or '').strip().lower() in {'n', 'no'}:
        return 'declined'
    return 'accepted'


def _neovim_install_choice() -> str:
    requested = str(os.environ.get('CCB_INSTALL_NEOVIM') or '').strip().lower()
    if requested in {'0', 'false', 'off', 'no'}:
        return 'env-skip'
    if requested in {'1', 'true', 'on', 'yes'}:
        return 'required'
    if not _stream_is_tty(sys.stdin) or not _stream_is_tty(sys.stdout):
        return 'noninteractive-skip'
    print('Install/refresh the default Neovim + LazyVim tool window now? [y/N] ', end='', flush=True)
    try:
        answer = sys.stdin.readline()
    except Exception:
        return 'noninteractive-skip'
    if str(answer or '').strip().lower() in {'y', 'yes'}:
        return 'optional'
    return 'declined'


def _stream_is_tty(stream) -> bool:
    checker = getattr(stream, 'isatty', None)
    if not callable(checker):
        return False
    try:
        return bool(checker())
    except Exception:
        return False


def _release_artifact_url(version: str, *, artifact_name: str) -> str:
    return f"{REPO_URL}/releases/download/v{version}/{artifact_name}"


def _release_artifact_name() -> str | None:
    return release_artifact_name(platform.system(), machine=platform.machine())


def _release_extract_dir_name(artifact_name: str) -> str:
    text = str(artifact_name or "").strip()
    if text.endswith(".tar.gz"):
        return text[:-7]
    if text.endswith(".tgz"):
        return text[:-4]
    return Path(text).stem


__all__ = ['cmd_update']
