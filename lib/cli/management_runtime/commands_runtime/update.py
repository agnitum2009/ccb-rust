from __future__ import annotations

from pathlib import Path
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile

from release_artifacts import release_artifact_name
from cli.roles_runtime.commands import cmd_roles
from cli.tools_runtime.neovim import provision_neovim
from rolepacks.sources import role_catalog_status

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


POST_UPDATE_COMMAND = "__post-update"
POST_UPDATE_TIMEOUT_SECONDS = 300.0
ENTRYPOINT_SMOKE_TIMEOUT_SECONDS = 30.0
DEFAULT_CATALOG_ROLE_IDS = ('agentroles.archi', 'agentroles.ccb_self')


def set_tmux_ui_active(active: bool) -> None:
    from cli.services.tmux_ui import set_tmux_ui_active as _set_tmux_ui_active

    _set_tmux_ui_active(active)


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
        if not _run_post_update_with_new_entrypoint(install_dir=install_dir, old_info=old_info, new_info=new_info):
            return 1
        return 0
    except Exception as exc:
        print(f"❌ Update failed: {exc}")
        return 1
    finally:
        if tmp_dir.exists():
            shutil.rmtree(tmp_dir, ignore_errors=True)


def maybe_handle_post_update_command(tokens: list[str], *, script_root: Path) -> int | None:
    if list(tokens[:1]) != [POST_UPDATE_COMMAND]:
        return None
    return _run_post_update_provisioning(install_dir=Path(script_root).expanduser())


def _run_post_update_with_new_entrypoint(
    *,
    install_dir: Path,
    old_info: dict[str, object],
    new_info: dict[str, object],
) -> bool:
    ccb_entry = _installed_ccb_entrypoint(install_dir)
    if not _verify_installed_ccb_entrypoint(ccb_entry):
        return False
    command = [
        str(ccb_entry),
        POST_UPDATE_COMMAND,
        "--from-version",
        _post_update_version_label(old_info),
        "--to-version",
        _post_update_version_label(new_info),
    ]
    env = dict(os.environ)
    env["CODEX_INSTALL_PREFIX"] = str(install_dir)
    env["CCB_SKIP_STARTUP_UPDATE_CHECK"] = "1"
    try:
        result = subprocess.run(command, cwd=Path.cwd(), env=env, timeout=_post_update_timeout_seconds())
    except subprocess.TimeoutExpired:
        if _post_update_failure_is_required():
            print(f"❌ Required post-update provisioning timed out after {_post_update_timeout_seconds():g}s.")
            return False
        print(f"⚠️  Post-update provisioning timed out after {_post_update_timeout_seconds():g}s.")
        print("   Core update completed; retry optional provisioning with `ccb roles list` or `ccb tools install neovim`.")
        return True
    except Exception as exc:
        if _post_update_failure_is_required():
            print(f"❌ Required post-update provisioning failed to run: {type(exc).__name__}: {exc}")
            return False
        print(f"⚠️  Post-update provisioning skipped: {type(exc).__name__}: {exc}")
        return True
    if result.returncode != 0:
        if _post_update_failure_is_required():
            print(f"❌ Required post-update provisioning exited with code {result.returncode}.")
            return False
        print(f"⚠️  Post-update provisioning exited with code {result.returncode}.")
        print("   Core update completed; retry optional provisioning with `ccb roles list` or `ccb tools install neovim`.")
    return True


def _installed_ccb_entrypoint(install_dir: Path) -> Path:
    bin_dir = str(os.environ.get("CODEX_BIN_DIR") or "").strip()
    if bin_dir:
        return Path(bin_dir).expanduser() / "ccb"
    install_root = Path(install_dir).expanduser()
    candidates: list[Path] = []
    argv0 = str(sys.argv[0] if sys.argv else "").strip()
    if argv0:
        current_entry = Path(argv0).expanduser()
        if current_entry.name == "ccb":
            candidates.append(current_entry)
    candidates.append(Path.home() / ".local" / "bin" / "ccb")
    candidates.append(install_root / "ccb")
    for candidate in candidates:
        if _entrypoint_targets_install_dir(candidate, install_root):
            return candidate
    return install_root / "ccb"


def _entrypoint_targets_install_dir(candidate: Path, install_dir: Path) -> bool:
    try:
        resolved_candidate = Path(candidate).expanduser().resolve()
        installed_entry = Path(install_dir).expanduser().resolve() / "ccb"
        if resolved_candidate == installed_entry:
            return True
    except Exception:
        installed_entry = Path(install_dir).expanduser() / "ccb"
    try:
        text = Path(candidate).expanduser().read_text(encoding="utf-8", errors="ignore")
    except Exception:
        return False
    return str(installed_entry) in text or str(Path(install_dir).expanduser() / "ccb") in text


def _verify_installed_ccb_entrypoint(ccb_entry: Path) -> bool:
    if not ccb_entry.exists():
        print(f"❌ Update failed: installed ccb entrypoint not found: {ccb_entry}")
        return False
    try:
        result = subprocess.run(
            [str(ccb_entry), "--print-version"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=_entrypoint_smoke_timeout_seconds(),
        )
    except subprocess.TimeoutExpired:
        print(f"❌ Update failed: installed ccb entrypoint smoke check timed out after {_entrypoint_smoke_timeout_seconds():g}s")
        return False
    except Exception as exc:
        print(f"❌ Update failed: installed ccb entrypoint smoke check could not run: {type(exc).__name__}: {exc}")
        return False
    if result.returncode == 0:
        return True
    print(f"❌ Update failed: installed ccb entrypoint failed runtime smoke check: {ccb_entry}")
    detail = (result.stderr or result.stdout or "").strip()
    if detail:
        print(f"   {detail.splitlines()[0]}")
    return False


def _post_update_version_label(info: dict[str, object]) -> str:
    value = info.get("version") or info.get("commit") or "unknown"
    return str(value)


def _post_update_timeout_seconds() -> float:
    return _positive_float_env("CCB_POST_UPDATE_TIMEOUT_SECONDS", POST_UPDATE_TIMEOUT_SECONDS)


def _entrypoint_smoke_timeout_seconds() -> float:
    return _positive_float_env("CCB_ENTRYPOINT_SMOKE_TIMEOUT_SECONDS", ENTRYPOINT_SMOKE_TIMEOUT_SECONDS)


def _positive_float_env(name: str, default: float) -> float:
    raw = str(os.environ.get(name) or "").strip()
    if not raw:
        return default
    try:
        value = float(raw)
    except ValueError:
        return default
    return value if value > 0 else default


def _post_update_failure_is_required() -> bool:
    # In post-update context, force env vars mean both "do not prompt" and
    # "treat provisioning failure as required".
    return (
        _truthy_env("CCB_POST_UPDATE_REQUIRED")
        or _truthy_env("CCB_INSTALL_ROLES")
        or _truthy_env("CCB_INSTALL_NEOVIM")
    )


def _truthy_env(name: str) -> bool:
    value = str(os.environ.get(name) or "").strip().lower()
    return value in {"1", "true", "on", "yes"}


def _run_post_update_provisioning(*, install_dir: Path) -> int:
    failures = 0
    try:
        set_tmux_ui_active(True)
    except Exception as exc:
        print(f"⚠️  Tmux UI post-update refresh skipped: {type(exc).__name__}: {exc}")
    try:
        failures += int(_update_builtin_roles_after_update(install_dir=install_dir) or 0)
    except Exception as exc:
        failures += 1
        print(f"⚠️  Role Pack post-update provisioning failed: {type(exc).__name__}: {exc}")
    try:
        _provision_neovim_after_update()
    except Exception as exc:
        failures += 1
        print(f"⚠️  Neovim post-update provisioning failed: {type(exc).__name__}: {exc}")
    return 1 if failures else 0


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


def _update_builtin_roles_after_update(*, install_dir: Path) -> int:
    return _update_catalog_roles_after_update(install_dir=install_dir)


def _update_catalog_roles_after_update(*, install_dir: Path) -> int:
    choice = _roles_update_choice()
    if choice == 'env-skip':
        print('ℹ️  Role Pack update skipped by CCB_INSTALL_ROLES=0')
        return 0
    try:
        rows = tuple(role_catalog_status(refresh_default=True))
    except Exception as exc:
        print(f'⚠️  Agent Roles catalog unavailable: {type(exc).__name__}: {exc}')
        return 1
    if choice == 'declined':
        print('ℹ️  Role Pack update skipped.')
        _print_catalog_followups(rows, include_default_roles=True)
        return 0
    if choice == 'noninteractive-skip':
        print('ℹ️  Role Pack update skipped in non-interactive update.')
        _print_catalog_followups(rows, include_default_roles=True)
        return 0
    failures = _refresh_installed_catalog_roles(rows, install_dir=install_dir)
    refreshed_rows = _refresh_catalog_rows(rows)
    failures += _install_default_catalog_roles(refreshed_rows, install_dir=install_dir)
    refreshed_rows = _refresh_catalog_rows(refreshed_rows)
    _print_catalog_followups(refreshed_rows)
    failures += _install_new_catalog_roles_interactive(refreshed_rows, install_dir=install_dir)
    return failures


def _refresh_catalog_rows(fallback: tuple[dict[str, object], ...]) -> tuple[dict[str, object], ...]:
    try:
        return tuple(role_catalog_status(refresh_default=True))
    except Exception:
        return fallback


def _refresh_installed_catalog_roles(rows: tuple[dict[str, object], ...], *, install_dir: Path) -> int:
    update_rows = [row for row in rows if row.get('status') == 'update_available']
    current_rows = [row for row in rows if row.get('status') == 'current']
    if not update_rows:
        if current_rows:
            print('✅ Installed Role Packs already match the catalog.')
        else:
            print('ℹ️  No installed catalog Role Packs to update.')
        return 0
    stdout = sys.stdout
    stderr = sys.stderr
    failures = 0
    for row in update_rows:
        role_id = str(row.get('role_id') or '').strip()
        if not role_id:
            continue
        code = cmd_roles(['update', role_id], script_root=install_dir, cwd=Path.cwd(), stdout=stdout, stderr=stderr)
        if code == 0:
            print(f'✅ Role Pack updated: {role_id}')
        else:
            failures += 1
            print(f'⚠️  Role Pack update failed: {role_id}')
    if failures:
        print(f'⚠️  Role Pack updates had {failures} failure(s).')
    return failures


def _install_default_catalog_roles(rows: tuple[dict[str, object], ...], *, install_dir: Path) -> int:
    default_ids = set(DEFAULT_CATALOG_ROLE_IDS)
    available = [
        row
        for row in rows
        if row.get('status') == 'available' and str(row.get('role_id') or '').strip() in default_ids
    ]
    if not available:
        return 0
    stdout = sys.stdout
    stderr = sys.stderr
    failures = 0
    for role_id in DEFAULT_CATALOG_ROLE_IDS:
        if not any(str(row.get('role_id') or '').strip() == role_id for row in available):
            continue
        code = cmd_roles(['install', role_id], script_root=install_dir, cwd=Path.cwd(), stdout=stdout, stderr=stderr)
        if code == 0:
            print(f'✅ Default Role Pack installed: {role_id}')
        else:
            failures += 1
            print(f'⚠️  Default Role Pack install failed: {role_id}')
    if failures:
        print(f'⚠️  Default Role Pack installs had {failures} failure(s).')
    return failures


def _print_catalog_followups(rows: tuple[dict[str, object], ...], *, include_default_roles: bool = False) -> None:
    default_ids = set(DEFAULT_CATALOG_ROLE_IDS)
    available_rows = [
        row
        for row in rows
        if row.get('status') == 'available'
    ]
    recommended = [
        row
        for row in available_rows
        if include_default_roles and str(row.get('role_id') or '').strip() in default_ids
    ]
    available = [row for row in available_rows if str(row.get('role_id') or '').strip() not in default_ids]
    missing = [row for row in rows if row.get('status') == 'installed_source_missing']
    if recommended:
        print('⭐ Recommended Agent Roles available:')
        _print_catalog_role_rows(recommended)
        print('   Install with `ccb roles install <role-id>`; bind with `ccb roles add <role-id>:<provider>`.')
    if available:
        print('🆕 New Agent Roles available:')
        _print_catalog_role_rows(available)
        print('   Install with `ccb roles install <role-id>`; bind with `ccb roles add <role-id>:<provider>`.')
    for row in missing:
        role_id = str(row.get('role_id') or '').strip()
        source_path = str(row.get('path') or '').strip()
        print(f'⚠️  Installed Role Pack source missing: {role_id}' + (f' ({source_path})' if source_path else ''))


def _print_catalog_role_rows(rows: list[dict[str, object]]) -> None:
    for index, row in enumerate(rows, start=1):
        role_id = str(row.get('role_id') or '').strip()
        version = str(row.get('version') or '').strip()
        name = str(row.get('name') or '').strip()
        description = _short_catalog_text(str(row.get('description') or '').strip())
        label = f'{role_id} v{version}' if version else role_id
        details = ' - '.join(item for item in (name, description) if item)
        print(f'   {index}. {label}' + (f': {details}' if details else ''))


def _install_new_catalog_roles_interactive(rows: tuple[dict[str, object], ...], *, install_dir: Path) -> int:
    available = [
        row
        for row in rows
        if row.get('status') == 'available'
        and str(row.get('role_id') or '').strip() not in DEFAULT_CATALOG_ROLE_IDS
    ]
    if not available or not _stream_is_tty(sys.stdin) or not _stream_is_tty(sys.stdout):
        return 0
    print('Install newly available Agent Roles now? Enter numbers, all, or blank to skip: ', end='', flush=True)
    try:
        answer = sys.stdin.readline()
    except Exception:
        return 0
    selected = _select_catalog_role_ids(answer, available)
    if not selected:
        print('ℹ️  New Role Pack installation skipped.')
        return 0
    stdout = sys.stdout
    stderr = sys.stderr
    failures = 0
    for role_id in selected:
        code = cmd_roles(['install', role_id], script_root=install_dir, cwd=Path.cwd(), stdout=stdout, stderr=stderr)
        if code == 0:
            print(f'✅ Role Pack installed: {role_id}')
        else:
            failures += 1
            print(f'⚠️  Role Pack install failed: {role_id}')
    if failures:
        print(f'⚠️  Role Pack installs had {failures} failure(s).')
    return failures


def _select_catalog_role_ids(answer: str, rows: list[dict[str, object]]) -> tuple[str, ...]:
    text = str(answer or '').strip().lower()
    if not text or text in {'n', 'no', 'skip'}:
        return ()
    if text in {'a', 'all', '*'}:
        return tuple(str(row.get('role_id') or '').strip() for row in rows if str(row.get('role_id') or '').strip())
    selected: list[str] = []
    by_id = {str(row.get('role_id') or '').strip().lower(): str(row.get('role_id') or '').strip() for row in rows}
    for item in text.replace(',', ' ').split():
        if item.isdigit():
            index = int(item) - 1
            if 0 <= index < len(rows):
                role_id = str(rows[index].get('role_id') or '').strip()
                if role_id:
                    selected.append(role_id)
            continue
        role_id = by_id.get(item)
        if role_id:
            selected.append(role_id)
    deduped: list[str] = []
    for role_id in selected:
        if role_id not in deduped:
            deduped.append(role_id)
    return tuple(deduped)


def _short_catalog_text(text: str, *, limit: int = 96) -> str:
    compact = ' '.join(str(text or '').split())
    if len(compact) <= limit:
        return compact
    return compact[: max(0, limit - 3)].rstrip() + '...'


def _roles_update_choice() -> str:
    requested = str(os.environ.get('CCB_INSTALL_ROLES') or '').strip().lower()
    if requested in {'0', 'false', 'off', 'no'}:
        return 'env-skip'
    if _truthy_env("CCB_POST_UPDATE_REQUIRED"):
        return 'accepted'
    if requested in {'1', 'true', 'on', 'yes'}:
        return 'accepted'
    if not _stream_is_tty(sys.stdin) or not _stream_is_tty(sys.stdout):
        return 'noninteractive-skip'
    print('Refresh installed and recommended Agent Roles from the catalog now? [Y/n] ', end='', flush=True)
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
    if _truthy_env("CCB_POST_UPDATE_REQUIRED"):
        return 'required'
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


__all__ = ['POST_UPDATE_COMMAND', 'cmd_update', 'maybe_handle_post_update_command']
