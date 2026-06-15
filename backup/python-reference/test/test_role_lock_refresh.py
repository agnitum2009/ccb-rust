from __future__ import annotations

from io import StringIO
import json
from pathlib import Path
import shutil
import sys
from types import SimpleNamespace

import pytest

import cli.phase2 as phase2_module
from cli.phase2 import maybe_handle_phase2
from cli.services.role_lock_refresh import confirm_project_role_lock_refresh, find_project_role_lock_updates
from rolepacks.manifest import load_role_manifest
from rolepacks.runtime_lookup import tree_digest


class _TtyInput(StringIO):
    def isatty(self) -> bool:  # pragma: no cover - trivial
        return True


def _is_tty(stream: object) -> bool:
    checker = getattr(stream, 'isatty', None)
    return bool(checker()) if callable(checker) else False


def _configure_role_store(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setenv('AGENT_ROLES_STORE', str(tmp_path / '.roles'))
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))


def _write_project_config(project: Path) -> None:
    (project / '.ccb').mkdir(parents=True)
    (project / '.ccb' / 'ccb.config').write_text(
        '\n'.join(
            [
                'version = 2',
                'entry_window = "main"',
                '',
                '[windows]',
                'main = "locked:codex"',
                '',
                '[agents.locked]',
                'provider = "codex"',
                'role = "test.locked"',
            ]
        )
        + '\n',
        encoding='utf-8',
    )


def _write_role_source(tmp_path: Path, *, version: str, memory_text: str) -> Path:
    source = tmp_path / f'role-source-{version}'
    source.mkdir(parents=True)
    (source / 'role.toml').write_text(
        '\n'.join(
            [
                'schema = "rolepack/v1"',
                'id = "test.locked"',
                'name = "Locked Role"',
                f'version = "{version}"',
                'description = "Role lock refresh fixture."',
                '',
                '[identity]',
                'default_agent_name = "locked"',
                '',
                '[compatibility]',
                'providers = ["codex"]',
                '',
                '[memory]',
                'files = ["memory.md"]',
            ]
        )
        + '\n',
        encoding='utf-8',
    )
    (source / 'memory.md').write_text(memory_text + '\n', encoding='utf-8')
    return source


def _install_role_source(tmp_path: Path, source: Path) -> dict[str, object]:
    role = load_role_manifest(source)
    digest = tree_digest(source)
    role_dir = tmp_path / '.roles' / 'installed' / role.id
    target = role_dir / 'versions' / role.version / digest
    if target.exists():
        shutil.rmtree(target)
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source, target, symlinks=True)
    current = role_dir / 'current'
    if current.exists() or current.is_symlink():
        if current.is_symlink() or current.is_file():
            current.unlink()
        else:
            shutil.rmtree(current)
    current.symlink_to(target, target_is_directory=True)
    metadata = {
        'schema': 'agent-roles-install/v1',
        'id': role.id,
        'version': role.version,
        'digest': f'sha256:{digest}',
        'source': 'agentroles',
        'source_path': str(source),
    }
    (role_dir / 'install.json').write_text(json.dumps(metadata, sort_keys=True, indent=2) + '\n', encoding='utf-8')
    return {'role_id': role.id, 'version': role.version, 'digest': metadata['digest'], 'path': str(target)}


def _write_project_lock(project: Path, install_payload: dict[str, object]) -> None:
    (project / '.ccb' / 'role-lock.json').write_text(
        json.dumps(
            {
                'schema': 'rolepack-lock/v1',
                'roles': {
                    str(install_payload['role_id']): {
                        'version': str(install_payload['version']),
                        'digest': str(install_payload['digest']),
                        'source': 'installed',
                        'default_agent_name': 'locked',
                    }
                },
            },
            sort_keys=True,
            indent=2,
        )
        + '\n',
        encoding='utf-8',
    )


def _stale_role_lock_project(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> tuple[Path, dict[str, object], dict[str, object]]:
    _configure_role_store(monkeypatch, tmp_path)
    project = tmp_path / 'project'
    _write_project_config(project)
    installed_v1 = _install_role_source(tmp_path, _write_role_source(tmp_path, version='1.0.0', memory_text='memory v1'))
    _write_project_lock(project, installed_v1)
    installed_v2 = _install_role_source(tmp_path, _write_role_source(tmp_path, version='2.0.0', memory_text='memory v2'))
    return project, installed_v1, installed_v2


def test_find_project_role_lock_updates_detects_installed_current_drift(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    project, installed_v1, installed_v2 = _stale_role_lock_project(monkeypatch, tmp_path)

    updates = find_project_role_lock_updates(project)

    assert len(updates) == 1
    assert updates[0].role_id == 'test.locked'
    assert updates[0].locked_version == installed_v1['version']
    assert updates[0].locked_digest == installed_v1['digest']
    assert updates[0].current_version == installed_v2['version']
    assert updates[0].current_digest == installed_v2['digest']


def test_confirm_project_role_lock_refresh_updates_lock_when_accepted(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    project, _installed_v1, installed_v2 = _stale_role_lock_project(monkeypatch, tmp_path)
    stdout = StringIO()

    confirm_project_role_lock_refresh(
        project,
        out=stdout,
        stdin=_TtyInput('y\n'),
        stream_is_tty_fn=_is_tty,
    )

    lock_payload = json.loads((project / '.ccb' / 'role-lock.json').read_text(encoding='utf-8'))
    lock_entry = lock_payload['roles']['test.locked']
    assert lock_entry['version'] == installed_v2['version']
    assert lock_entry['digest'] == installed_v2['digest']
    assert 'Role Pack updates are available for this project:' in stdout.getvalue()
    assert 'role_lock_refreshed: test.locked version=2.0.0' in stdout.getvalue()


def test_confirm_project_role_lock_refresh_warns_without_mutating_noninteractive(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    project, installed_v1, _installed_v2 = _stale_role_lock_project(monkeypatch, tmp_path)
    stdout = StringIO()

    confirm_project_role_lock_refresh(
        project,
        out=stdout,
        stdin=StringIO('y\n'),
        stream_is_tty_fn=_is_tty,
    )

    lock_payload = json.loads((project / '.ccb' / 'role-lock.json').read_text(encoding='utf-8'))
    lock_entry = lock_payload['roles']['test.locked']
    assert lock_entry['version'] == installed_v1['version']
    assert lock_entry['digest'] == installed_v1['digest']
    assert 'role_lock_update_available: test.locked' in stdout.getvalue()
    assert 'role_lock_refresh: skipped_noninteractive' in stdout.getvalue()


def test_phase2_start_refreshes_role_lock_before_start_service(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    project, _installed_v1, installed_v2 = _stale_role_lock_project(monkeypatch, tmp_path)
    seen: dict[str, object] = {}

    def _fake_start(context, command):
        del command
        lock_payload = json.loads((context.project.project_root / '.ccb' / 'role-lock.json').read_text(encoding='utf-8'))
        seen['lock_version'] = lock_payload['roles']['test.locked']['version']
        seen['lock_digest'] = lock_payload['roles']['test.locked']['digest']
        return SimpleNamespace(
            project_root=str(context.project.project_root),
            project_id=context.project.project_id,
            started=('locked',),
            daemon_started=False,
            socket_path=str(context.paths.ccbd_socket_path),
        )

    monkeypatch.setattr(phase2_module, 'start_agents', _fake_start)
    monkeypatch.setattr(sys, 'stdin', _TtyInput('y\n'))
    stdout = StringIO()
    stderr = StringIO()

    code = maybe_handle_phase2([], cwd=project, stdout=stdout, stderr=stderr)

    assert code == 0, stderr.getvalue()
    assert seen == {'lock_version': installed_v2['version'], 'lock_digest': installed_v2['digest']}
    assert 'role_lock_refreshed: test.locked version=2.0.0' in stdout.getvalue()
    assert 'start_status: ok' in stdout.getvalue()
