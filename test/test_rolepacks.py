from __future__ import annotations

from io import StringIO
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

import rolepacks.sources as role_sources
from agents.config_loader import load_project_config
from cli.entrypoint import run_cli_entrypoint
from project_memory import load_memory_sources
from provider_profiles.codex_home_config import materialize_codex_home_config
from rolepacks.manifest import RoleManifestError, load_role_manifest
from rolepacks.runtime_lookup import tree_digest
from rolepacks.service import builtin_role_root, install_role, load_installed_role, role_status, update_role
from rolepacks.sources import (
    DEFAULT_AGENT_ROLES_SPEC_GIT_URL,
    add_role_source,
    default_agent_roles_source,
    discover_source_roles,
    role_catalog_status,
)


REPO_ROOT = Path(__file__).resolve().parents[1]


def _agent_roles_archi() -> Path:
    return Path(os.environ['AGENT_ROLES_SPEC_HOME']) / 'roles' / 'archi'


@pytest.fixture(autouse=True)
def _agent_roles_catalog(monkeypatch, tmp_path: Path) -> None:
    agent_roles_spec = tmp_path / 'default-agent-roles-spec'
    _write_agent_roles_archi_fixture(agent_roles_spec)
    monkeypatch.setenv('HOME', str(tmp_path / 'home'))
    monkeypatch.setenv('AGENT_ROLES_SPEC_HOME', str(agent_roles_spec))
    monkeypatch.delenv('CCB_AGENT_ROLES_INCLUDE_REFERENCE', raising=False)


def _write_project_config(project: Path) -> None:
    ccb = project / '.ccb'
    ccb.mkdir()
    (ccb / 'ccb.config').write_text(
        '\n'.join(
            [
                'version = 2',
                'entry_window = "main"',
                '',
                '[windows]',
                'main = "agent1:codex"',
                '',
                '[agents.agent1]',
                'provider = "codex"',
            ]
        )
        + '\n',
        encoding='utf-8',
    )


def _write_project_config_text(project: Path, text: str) -> None:
    ccb = project / '.ccb'
    ccb.mkdir()
    (ccb / 'ccb.config').write_text(text, encoding='utf-8')


def _run_cli(argv: list[str], *, cwd: Path, script_root: Path = REPO_ROOT) -> tuple[int, str, str]:
    stdout = StringIO()
    stderr = StringIO()
    code = run_cli_entrypoint(
        argv,
        version='7.1.0',
        script_root=script_root,
        cwd=cwd,
        stdout=stdout,
        stderr=stderr,
    )
    return code, stdout.getvalue(), stderr.getvalue()


def _write_fake_tool_role(script_root: Path) -> None:
    role = script_root / 'roles' / 'test.fake'
    (role / 'tools').mkdir(parents=True)
    (role / 'role.toml').write_text(
        '\n'.join(
            [
                'schema = "rolepack/v1"',
                'id = "test.fake"',
                'name = "Fake Role"',
                'version = "1.0.0"',
                'description = "Fake role for tool hook tests."',
                '',
                '[tools.fake]',
                'install = "python tools/hook.py"',
                'update = "python tools/hook.py"',
                'doctor = "python tools/hook.py"',
                'required = true',
            ]
        )
        + '\n',
        encoding='utf-8',
    )
    (role / 'README.md').write_text('# Fake Role\n', encoding='utf-8')
    (role / 'tools' / 'hook.py').write_text(
        '\n'.join(
            [
                'from pathlib import Path',
                'import os',
                'import helper',
                'assert helper.VALUE == "ok"',
                'target = Path(os.environ["FAKE_ROLE_SENTINEL"])',
                'target.write_text(os.environ["CCB_ROLE_TOOL_ACTION"], encoding="utf-8")',
                'print("hook_action: " + os.environ["CCB_ROLE_TOOL_ACTION"])',
            ]
        )
        + '\n',
        encoding='utf-8',
    )
    (role / 'tools' / 'helper.py').write_text('VALUE = "ok"\n', encoding='utf-8')


def _write_agent_roles_archi_fixture(catalog: Path) -> Path:
    role = catalog / 'roles' / 'archi'
    (role / 'adapters' / 'ccb' / 'tools').mkdir(parents=True, exist_ok=True)
    (role / 'adapters' / 'ccb' / 'skills' / 'archi-tooling').mkdir(parents=True, exist_ok=True)
    for skill in ('archi-advice', 'archi-diff', 'archi-full', 'archi-goal'):
        (role / 'skills' / skill).mkdir(parents=True, exist_ok=True)
        (role / 'skills' / skill / 'SKILL.md').write_text(
            f'---\nname: {skill}\ndescription: Review architecture risk fixture.\n---\n\n# {skill}\n',
            encoding='utf-8',
        )
    (role / 'adapters' / 'ccb' / 'skills' / 'archi-tooling' / 'SKILL.md').write_text(
        '---\nname: archi-tooling\ndescription: CCB adapter tooling fixture.\n---\n\n# Archi Tooling\n',
        encoding='utf-8',
    )
    (role / 'role.toml').write_text(
        '\n'.join(
            [
                'schema = "agent-role/preview-0.1"',
                'id = "agentroles.archi"',
                'name = "Architecture Reviewer"',
                'version = "0.2.0"',
                'description = "Reviews architecture drift and structural risk."',
                '',
                '[identity]',
                'default_name = "archi"',
                '',
                '[contents]',
                'memory = ["memory.md"]',
                'skills = ["skills/archi-advice", "skills/archi-diff", "skills/archi-full", "skills/archi-goal"]',
            ]
        )
        + '\n',
        encoding='utf-8',
    )
    (role / 'adapters' / 'ccb' / 'adapter.toml').write_text(
        '\n'.join(
            [
                'schema = "agent-role-adapter/ccb-preview-0.1"',
                'host = "ccb"',
                'default_agent_name = "archi"',
                'supported_providers = ["codex", "claude"]',
                'memory = ["adapters/ccb/memory.md"]',
                'skills = ["adapters/ccb/skills/archi-tooling"]',
                '',
                '[tools.architec]',
                'install = "python -B adapters/ccb/tools/install.py"',
                'doctor = "python -B adapters/ccb/tools/doctor.py"',
                'update = "python -B adapters/ccb/tools/update.py"',
                'required = true',
            ]
        )
        + '\n',
        encoding='utf-8',
    )
    (role / 'memory.md').write_text(
        'Architecture Reviewer Memory\nArchitec is the architecture analysis CLI.\n',
        encoding='utf-8',
    )
    (role / 'adapters' / 'ccb' / 'memory.md').write_text(
        'CCB Adapter Memory\n'
        'Use the managed Architec wrapper named `ccb-archi`.\n'
        'Do not copy llmgateway secrets into role memory.\n',
        encoding='utf-8',
    )
    for action in ('install', 'update'):
        (role / 'adapters' / 'ccb' / 'tools' / f'{action}.py').write_text(
            f'print("architec_status: ok\\naction: {action}")\n',
            encoding='utf-8',
        )
    (role / 'adapters' / 'ccb' / 'tools' / 'doctor.py').write_text(
        '\n'.join(
            [
                'from __future__ import annotations',
                '',
                'import os',
                'from pathlib import Path',
                'import shutil',
                '',
                '',
                'def _is_executable(path: Path) -> bool:',
                '    return path.is_file() and os.access(path, os.X_OK)',
                '',
                '',
                'def main() -> int:',
                "    data_home = Path(os.environ.get('XDG_DATA_HOME') or Path.home() / '.local' / 'share')",
                "    root = data_home / 'ccb' / 'tools' / 'architec'",
                "    wrapper = root / 'bin' / 'ccb-archi'",
                "    archi_binary = root / 'venv' / 'bin' / 'archi'",
                '    managed_wrapper_exists = _is_executable(wrapper)',
                '    managed_archi_binary_exists = _is_executable(archi_binary)',
                "    path_archi = shutil.which('archi')",
                "    llmgateway = Path.home() / '.llmgateway' / 'config.yaml'",
                "    selected_kind = 'managed_wrapper' if managed_wrapper_exists else ('path_archi' if path_archi else 'none')",
                "    status = 'ok' if managed_wrapper_exists and managed_archi_binary_exists else ('degraded' if path_archi else 'missing')",
                "    print(f'architec_status: {status}')",
                "    print(f'managed_wrapper_exists: {managed_wrapper_exists}')",
                "    print(f'managed_archi_binary_exists: {managed_archi_binary_exists}')",
                "    print(f'selected_kind: {selected_kind}')",
                "    print('llmgateway_config: ' + ('present' if llmgateway.is_file() else 'missing'))",
                "    return 0 if status == 'ok' else 1",
                '',
                '',
                "raise SystemExit(main())",
            ]
        )
        + '\n',
        encoding='utf-8',
    )
    return role


def _write_catalog_role(catalog: Path, base_name: str, child_name: str, *, role_id: str, version: str, name: str) -> Path:
    role = catalog / base_name / child_name
    role.mkdir(parents=True)
    (role / 'role.toml').write_text(
        '\n'.join(
            [
                'schema = "rolepack/v1"',
                f'id = "{role_id}"',
                f'name = "{name}"',
                f'version = "{version}"',
                f'description = "{name} fixture."',
            ]
        )
        + '\n',
        encoding='utf-8',
    )
    return role


def _write_direct_role(
    root: Path,
    child_name: str,
    *,
    role_id: str,
    version: str,
    name: str,
    default_agent_name: str | None = None,
    providers: tuple[str, ...] = ('codex',),
) -> Path:
    role = root / child_name
    role.mkdir(parents=True)
    lines = [
        'schema = "rolepack/v1"',
        f'id = "{role_id}"',
        f'name = "{name}"',
        f'version = "{version}"',
        f'description = "{name} fixture."',
    ]
    if default_agent_name is not None:
        lines.extend(
            [
                '',
                '[identity]',
                f'default_agent_name = "{default_agent_name}"',
            ]
        )
    if providers:
        rendered_providers = ', '.join(f'"{item}"' for item in providers)
        lines.extend(
            [
                '',
                '[compatibility]',
                f'providers = [{rendered_providers}]',
            ]
        )
    (role / 'role.toml').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    return role


def _write_memory_catalog_role(
    catalog: Path,
    *,
    role_id: str = 'test.locked',
    version: str = '1.0.0',
    default_agent_name: str = 'locked',
    memory_text: str = 'locked memory v1',
) -> Path:
    role = catalog / 'roles' / role_id.rsplit('.', 1)[-1]
    role.mkdir(parents=True, exist_ok=True)
    (role / 'role.toml').write_text(
        '\n'.join(
            [
                'schema = "rolepack/v1"',
                f'id = "{role_id}"',
                'name = "Locked Role"',
                f'version = "{version}"',
                'description = "Role lock fixture."',
                '',
                '[identity]',
                f'default_agent_name = "{default_agent_name}"',
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
    (role / 'memory.md').write_text(memory_text + '\n', encoding='utf-8')
    return role


def test_role_manifest_validation_is_host_runtime_independent(tmp_path: Path) -> None:
    role = tmp_path / 'roles' / 'test.archi'
    role.mkdir(parents=True)
    (role / 'role.toml').write_text(
        '\n'.join(
            [
                'schema = "rolepack/v1"',
                'id = "test.archi"',
                'name = "Test Architecture Role"',
                'version = "1.2.3"',
                'description = "Portable manifest validation fixture."',
                '',
                '[identity]',
                'default_agent_name = "archi"',
                '',
                '[compatibility]',
                'providers = ["codex", "claude"]',
            ]
        )
        + '\n',
        encoding='utf-8',
    )

    manifest = load_role_manifest(role)

    assert manifest.id == 'test.archi'
    assert manifest.default_agent_name == 'archi'
    assert manifest.providers == ('codex', 'claude')


def test_role_manifest_requires_publisher_qualified_id(tmp_path: Path) -> None:
    role = tmp_path / 'roles' / 'archi'
    role.mkdir(parents=True)
    (role / 'role.toml').write_text(
        '\n'.join(
            [
                'schema = "rolepack/v1"',
                'id = "archi"',
                'name = "Archi"',
                'version = "1.0.0"',
                'description = "Invalid role id fixture."',
            ]
        )
        + '\n',
        encoding='utf-8',
    )

    with pytest.raises(RoleManifestError, match='publisher.role'):
        load_role_manifest(role)


def test_role_manifest_rejects_non_table_identity(tmp_path: Path) -> None:
    role = tmp_path / 'roles' / 'test.bad'
    role.mkdir(parents=True)
    (role / 'role.toml').write_text(
        '\n'.join(
            [
                'schema = "rolepack/v1"',
                'id = "test.bad"',
                'name = "Bad Role"',
                'version = "1.0.0"',
                'description = "Invalid identity fixture."',
                'identity = "bad"',
            ]
        )
        + '\n',
        encoding='utf-8',
    )

    manifest = load_role_manifest(role)
    with pytest.raises(RoleManifestError, match='identity must be a table'):
        _ = manifest.default_agent_name


def test_agent_role_preview_manifest_translates_for_ccb() -> None:
    manifest = load_role_manifest(_agent_roles_archi())

    assert manifest.id == 'agentroles.archi'
    assert manifest.default_agent_name == 'archi'
    assert manifest.providers == ('codex', 'claude')
    assert manifest.manifest['schema'] == 'rolepack/v1'
    assert manifest.manifest['source_schema'] == 'agent-role/preview-0.1'
    assert manifest.manifest['memory']['files'] == ['memory.md', 'adapters/ccb/memory.md']
    assert manifest.manifest['skills']['codex'] == [
        'skills/archi-advice',
        'skills/archi-diff',
        'skills/archi-full',
        'skills/archi-goal',
        'adapters/ccb/skills/archi-tooling',
    ]
    assert manifest.manifest['tools']['architec']['doctor'] == 'python -B adapters/ccb/tools/doctor.py'


def test_catalog_discovery_prefers_roles_and_hides_reference_roles_by_default(tmp_path: Path, monkeypatch) -> None:
    catalog = tmp_path / 'agent-roles-spec'
    reference_archi = _write_catalog_role(
        catalog,
        'reference_roles',
        'archi',
        role_id='agentroles.archi',
        version='0.1.0',
        name='Reference Archi',
    )
    production_archi = _write_catalog_role(
        catalog,
        'roles',
        'archi',
        role_id='agentroles.archi',
        version='0.2.0',
        name='Production Archi',
    )
    _write_catalog_role(
        catalog,
        'reference_roles',
        'demo',
        role_id='agentroles.demo',
        version='0.1.0',
        name='Reference Demo',
    )
    monkeypatch.setenv('AGENT_ROLES_SPEC_HOME', str(catalog))

    rows = {str(row['role_id']): row for row in role_catalog_status()}

    assert rows['agentroles.archi']['version'] == '0.2.0'
    assert rows['agentroles.archi']['path'] == str(production_archi)
    assert 'agentroles.demo' not in rows

    roles_with_references = {role.role_id: role for role in discover_source_roles(include_reference=True)}
    assert roles_with_references['agentroles.archi'].path == production_archi
    assert roles_with_references['agentroles.archi'].path != reference_archi
    assert roles_with_references['agentroles.archi'].duplicates == (f'agentroles:{reference_archi}',)
    assert roles_with_references['agentroles.demo'].version == '0.1.0'

    monkeypatch.setenv('CCB_AGENT_ROLES_INCLUDE_REFERENCE', '1')
    reference_rows = {str(row['role_id']): row for row in role_catalog_status()}
    assert reference_rows['agentroles.archi']['duplicates'] == (f'agentroles:{reference_archi}',)
    assert 'duplicate_source_roles' in reference_rows['agentroles.archi']['warning']

    code, out, err = _run_cli(['roles', 'list'], cwd=tmp_path)
    assert code == 0
    assert err == ''
    assert f'ignored agentroles:{reference_archi}' in out


def test_legacy_builtin_role_root_points_to_catalog_root(tmp_path: Path, monkeypatch) -> None:
    catalog = tmp_path / 'agent-roles-spec'
    production_archi = _write_catalog_role(
        catalog,
        'roles',
        'archi',
        role_id='agentroles.archi',
        version='0.2.0',
        name='Production Archi',
    )
    monkeypatch.setenv('AGENT_ROLES_SPEC_HOME', str(catalog))

    assert builtin_role_root() == catalog.resolve()
    assert production_archi == catalog / 'roles' / 'archi'


def test_catalog_discovery_reports_duplicate_registered_sources(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    default_catalog = tmp_path / 'agent-roles-spec'
    override_catalog = tmp_path / 'override-agent-roles'
    default_archi = _write_catalog_role(
        default_catalog,
        'roles',
        'archi',
        role_id='agentroles.archi',
        version='0.2.0',
        name='Default Archi',
    )
    override_archi = _write_catalog_role(
        override_catalog,
        'roles',
        'archi',
        role_id='agentroles.archi',
        version='9.9.9',
        name='Override Archi',
    )
    monkeypatch.setenv('AGENT_ROLES_SPEC_HOME', str(default_catalog))
    add_role_source('override', override_catalog)

    rows = {str(row['role_id']): row for row in role_catalog_status()}

    assert rows['agentroles.archi']['version'] == '0.2.0'
    assert rows['agentroles.archi']['path'] == str(default_archi)
    assert rows['agentroles.archi']['duplicates'] == (f'override:{override_archi}',)
    assert 'duplicate_source_roles' in rows['agentroles.archi']['warning']


def test_system_role_source_precedes_default_agent_roles_catalog(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('HOME', str(tmp_path / 'home'))
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    system_roles = tmp_path / 'home' / '.ccb' / 'roles'
    local_archi = _write_direct_role(
        system_roles,
        'archi',
        role_id='agentroles.archi',
        version='9.9.9',
        name='Local Archi',
    )

    rows = {str(row['role_id']): row for row in role_catalog_status()}

    assert rows['agentroles.archi']['source'] == 'systemroles'
    assert rows['agentroles.archi']['version'] == '9.9.9'
    assert rows['agentroles.archi']['path'] == str(local_archi)
    assert rows['agentroles.archi']['duplicates'] == (f'agentroles:{_agent_roles_archi()}',)
    assert 'duplicate_source_roles' in rows['agentroles.archi']['warning']


def test_roles_add_snapshots_uninstalled_system_role_source(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('HOME', str(tmp_path / 'home'))
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    system_roles = tmp_path / 'home' / '.ccb' / 'roles'
    _write_direct_role(
        system_roles,
        'review',
        role_id='local.review',
        version='0.1.0',
        name='Local Review',
        default_agent_name='review',
    )
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)

    code, out, err = _run_cli(['roles', 'list'], cwd=project)
    assert code == 0
    assert err == ''
    assert 'role: id=local.review' in out
    assert 'source=systemroles' in out

    code, out, err = _run_cli(['roles', 'add', 'local.review:codex'], cwd=project)
    assert code == 0
    assert err == ''
    assert 'role_status: added' in out
    assert 'install: snapshotted_from_system_source' in out
    assert load_installed_role('local.review') is not None
    assert (tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'local.review' / 'install.json').is_file()
    lock_payload = json.loads((project / '.ccb' / 'role-lock.json').read_text(encoding='utf-8'))
    assert lock_payload['roles']['local.review']['version'] == '0.1.0'
    assert lock_payload['roles']['local.review']['default_agent_name'] == 'review'
    loaded = load_project_config(project).config
    assert loaded.agents['review'].role == 'local.review'


def test_roles_sync_defaults_to_current_role_directory(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    source = tmp_path / 'role-source'
    role = _write_direct_role(
        source,
        'review',
        role_id='local.review',
        version='0.1.0',
        name='Local Review',
    )
    install_role(source_path=role, with_tools=False)
    metadata_path = tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'local.review' / 'install.json'
    old_metadata = json.loads(metadata_path.read_text(encoding='utf-8'))
    (role / 'memory.md').write_text('updated local role source\n', encoding='utf-8')

    code, out, err = _run_cli(['roles', 'sync'], cwd=role)
    new_metadata = json.loads(metadata_path.read_text(encoding='utf-8'))

    assert code == 0
    assert err == ''
    assert f'path: {role}' in out
    assert 'role: id=local.review status=synced' in out
    assert new_metadata['digest'] != old_metadata['digest']


def test_roles_sync_path_processes_only_that_role_library(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('HOME', str(tmp_path / 'home'))
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    library = tmp_path / 'role-library'
    installed_role = _write_direct_role(
        library,
        'installed',
        role_id='local.installed',
        version='0.1.0',
        name='Installed Local Role',
    )
    missing_role = _write_direct_role(
        library,
        'missing',
        role_id='local.missing',
        version='0.1.0',
        name='Missing Local Role',
    )
    global_role = _write_direct_role(
        tmp_path / 'home' / '.ccb' / 'roles',
        'global',
        role_id='local.global',
        version='0.1.0',
        name='Global Local Role',
    )
    install_role(source_path=installed_role, with_tools=False)
    install_role(source_path=global_role, with_tools=False)
    (installed_role / 'memory.md').write_text('updated installed role\n', encoding='utf-8')

    code, out, err = _run_cli(['roles', 'sync'], cwd=library)

    assert code == 0
    assert err == ''
    assert f'path: {library}' in out
    assert 'role: id=local.installed status=synced' in out
    assert 'role: id=local.missing status=skipped_not_installed' in out
    assert 'local.global' not in out
    assert not (tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'local.missing' / 'install.json').exists()
    assert missing_role.is_dir()


def test_dotroles_system_source_is_visible_when_ccb_roles_dir_missing(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('HOME', str(tmp_path / 'home'))
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    dotroles = tmp_path / 'home' / '.roles'
    dot_role = _write_direct_role(
        dotroles,
        'helper',
        role_id='local.helper',
        version='0.1.0',
        name='Local Helper',
    )

    rows = {str(row['role_id']): row for row in role_catalog_status()}

    assert rows['local.helper']['source'] == 'dotroles'
    assert rows['local.helper']['path'] == str(dot_role)


def test_catalog_discovery_falls_back_to_github_cache_when_local_catalog_missing(
    tmp_path: Path,
    monkeypatch,
) -> None:
    monkeypatch.delenv('AGENT_ROLES_SPEC_HOME', raising=False)
    monkeypatch.delenv('CCB_AGENT_ROLES_SPEC_HOME', raising=False)
    monkeypatch.setenv('HOME', str(tmp_path / 'home'))
    monkeypatch.setenv('XDG_CACHE_HOME', str(tmp_path / 'xdg-cache'))
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    remote_catalog = tmp_path / 'remote-agent-roles-spec'
    _write_catalog_role(
        remote_catalog,
        'roles',
        'remote',
        role_id='agentroles.remote',
        version='0.1.0',
        name='Remote Role',
    )
    commands: list[list[str]] = []

    def fake_run(cmd, **kwargs):
        commands.append(list(cmd))
        target = Path(cmd[-1])
        shutil.copytree(remote_catalog, target)
        return subprocess.CompletedProcess(cmd, 0, stdout='', stderr='')

    monkeypatch.setattr(role_sources.subprocess, 'run', fake_run)

    source = default_agent_roles_source()
    rows = {str(row['role_id']): row for row in role_catalog_status()}

    expected_cache = tmp_path / 'xdg-cache' / 'ccb' / 'role-catalogs' / 'agent-roles-spec'
    assert source == expected_cache.resolve()
    assert commands == [['git', 'clone', '--depth', '1', DEFAULT_AGENT_ROLES_SPEC_GIT_URL, str(expected_cache)]]
    assert rows['agentroles.remote']['source'] == 'agentroles'
    assert rows['agentroles.remote']['path'] == str(expected_cache / 'roles' / 'remote')


def test_catalog_refresh_pulls_existing_github_cache(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.delenv('AGENT_ROLES_SPEC_HOME', raising=False)
    monkeypatch.delenv('CCB_AGENT_ROLES_SPEC_HOME', raising=False)
    monkeypatch.setenv('HOME', str(tmp_path / 'home'))
    monkeypatch.setenv('XDG_CACHE_HOME', str(tmp_path / 'xdg-cache'))
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    cache = tmp_path / 'xdg-cache' / 'ccb' / 'role-catalogs' / 'agent-roles-spec'
    _write_catalog_role(
        cache,
        'roles',
        'remote',
        role_id='agentroles.remote',
        version='0.1.0',
        name='Remote Role',
    )
    (cache / '.git').mkdir()
    commands: list[list[str]] = []

    def fake_run(cmd, **kwargs):
        commands.append(list(cmd))
        return subprocess.CompletedProcess(cmd, 0, stdout='', stderr='')

    monkeypatch.setattr(role_sources.subprocess, 'run', fake_run)

    rows = {str(row['role_id']): row for row in role_catalog_status(refresh_default=True)}

    assert rows['agentroles.remote']['status'] == 'available'
    assert commands == [['git', '-C', str(cache), 'pull', '--ff-only']]


def test_remote_github_catalog_cache_can_be_disabled(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.delenv('AGENT_ROLES_SPEC_HOME', raising=False)
    monkeypatch.delenv('CCB_AGENT_ROLES_SPEC_HOME', raising=False)
    monkeypatch.setenv('HOME', str(tmp_path / 'home'))
    monkeypatch.setenv('XDG_CACHE_HOME', str(tmp_path / 'xdg-cache'))
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    monkeypatch.setenv('CCB_AGENT_ROLES_SPEC_NO_REMOTE', '1')

    def fail_run(cmd, **kwargs):
        raise AssertionError(f'unexpected git command: {cmd}')

    monkeypatch.setattr(role_sources.subprocess, 'run', fail_run)

    assert default_agent_roles_source() is None
    assert role_catalog_status() == ()


def test_agent_role_preview_can_install_from_path_and_project_skills(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)

    payload = install_role(source_path=_agent_roles_archi(), with_tools=False)
    assert payload['role_status'] == 'installed'
    assert payload['role_id'] == 'agentroles.archi'
    assert payload['source'] == 'path'

    update_payload = update_role('agentroles.archi', with_tools=False)
    assert update_payload['role_status'] == 'updated'
    assert update_payload['role_id'] == 'agentroles.archi'
    assert update_payload['source'] == 'path'

    assert _run_cli(['roles', 'add', 'agentroles.archi:codex'], cwd=project)[0] == 0
    source_home = tmp_path / 'source-codex'
    source_home.mkdir()
    target_home = tmp_path / 'managed-codex'

    materialize_codex_home_config(
        target_home,
        source_home=source_home,
        project_root=project,
        agent_name='archi',
        workspace_path=project,
    )

    assert (target_home / 'skills' / 'archi-diff' / 'SKILL.md').is_file()
    assert (target_home / 'skills' / 'archi-tooling' / 'SKILL.md').is_file()
    sources = load_memory_sources(project, agent_name='archi', provider='codex')
    role_memory = [source for source in sources if source.kind == 'role_memory']
    assert len(role_memory) == 2
    assert any('Architecture Reviewer Memory' in source.content for source in role_memory)
    assert any('CCB Adapter Memory' in source.content for source in role_memory)


def test_agent_role_preview_path_install_cli_supports_shorthand(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()

    code, out, err = _run_cli(
        ['roles', 'install', '--path', str(_agent_roles_archi()), '--skip-tools'],
        cwd=tmp_path,
    )
    assert code == 0
    assert err == ''
    assert 'role_id: agentroles.archi' in out
    assert 'source: path' in out

    _write_project_config_text(project, 'agent1:codex, agentroles.archi:codex\n')
    loaded = load_project_config(project).config

    assert loaded.default_agents == ('agent1', 'archi')
    assert loaded.agents['archi'].role == 'agentroles.archi'
    assert loaded.layout_spec == 'agent1:codex, archi:codex'


def test_roles_list_show_and_install_use_system_store(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))

    code, out, err = _run_cli(['roles', 'list'], cwd=tmp_path)
    assert code == 0
    assert err == ''
    assert 'roles_status: ok' in out
    assert 'role: id=agentroles.archi' in out

    code, out, err = _run_cli(['roles', 'show', 'agentroles.archi'], cwd=tmp_path)
    assert code == 0
    assert err == ''
    assert 'id: agentroles.archi' in out

    code, out, err = _run_cli(['roles', 'install', 'agentroles.archi', '--skip-tools'], cwd=tmp_path)
    assert code == 0
    assert err == ''
    assert 'role_status: installed' in out
    assert load_installed_role('agentroles.archi') is not None
    assert (tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'agentroles.archi' / 'install.json').is_file()


def test_legacy_ccb_archi_role_id_aliases_to_agentroles_archi(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)

    code, out, err = _run_cli(['roles', 'show', 'ccb.archi'], cwd=tmp_path)
    assert code == 0
    assert err == ''
    assert 'id: agentroles.archi' in out

    code, out, err = _run_cli(['roles', 'install', 'ccb.archi', '--skip-tools'], cwd=tmp_path)
    assert code == 0
    assert err == ''
    assert 'role_id: agentroles.archi' in out
    assert load_installed_role('ccb.archi') is not None
    assert (tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'agentroles.archi' / 'install.json').is_file()
    assert not (tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'ccb.archi' / 'install.json').exists()

    code, out, err = _run_cli(['roles', 'add', 'ccb.archi:codex'], cwd=project)
    assert code == 0
    assert err == ''
    assert 'role_id: agentroles.archi' in out
    text = (project / '.ccb' / 'ccb.config').read_text(encoding='utf-8')
    assert 'agentroles.archi:codex' in text
    assert 'ccb.archi:codex' not in text
    loaded = load_project_config(project).config
    assert loaded.agents['archi'].role == 'agentroles.archi'


def test_legacy_ccb_archi_current_store_migrates_to_canonical_metadata(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    install_role('agentroles.archi', with_tools=False)
    store = tmp_path / 'xdg-data' / 'ccb' / 'roles'
    canonical = store / 'agentroles.archi'
    legacy = store / 'ccb.archi'
    shutil.copytree(canonical, legacy, symlinks=False)
    shutil.rmtree(canonical)
    metadata_path = legacy / 'install.json'
    metadata = json.loads(metadata_path.read_text(encoding='utf-8'))
    metadata['id'] = 'ccb.archi'
    metadata['source_path'] = str(tmp_path / 'old-ccb' / 'roles' / 'ccb.archi')
    metadata_path.write_text(json.dumps(metadata, sort_keys=True, indent=2) + '\n', encoding='utf-8')

    rows = {str(row['role_id']): row for row in role_catalog_status()}

    assert rows['agentroles.archi']['status'] == 'current'
    assert rows['agentroles.archi']['path'] == str(_agent_roles_archi())
    canonical_metadata = json.loads((canonical / 'install.json').read_text(encoding='utf-8'))
    assert canonical_metadata['id'] == 'agentroles.archi'
    assert canonical_metadata['source_path'] == str(_agent_roles_archi())
    assert (canonical / 'current' / 'role.toml').is_file()
    assert load_installed_role('ccb.archi').id == 'agentroles.archi'


def test_roles_status_legacy_ccb_archi_migrates_on_status_query(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    install_role('agentroles.archi', with_tools=False)
    store = tmp_path / 'xdg-data' / 'ccb' / 'roles'
    canonical = store / 'agentroles.archi'
    legacy = store / 'ccb.archi'
    shutil.copytree(canonical, legacy, symlinks=False)
    shutil.rmtree(canonical)
    metadata_path = legacy / 'install.json'
    metadata = json.loads(metadata_path.read_text(encoding='utf-8'))
    metadata['id'] = 'ccb.archi'
    metadata['source_path'] = str(tmp_path / 'old-ccb' / 'roles' / 'ccb.archi')
    metadata_path.write_text(json.dumps(metadata, sort_keys=True, indent=2) + '\n', encoding='utf-8')

    payload = role_status('ccb.archi')

    assert payload['role_id'] == 'agentroles.archi'
    assert payload['installed'] is True
    assert payload['available'] is True
    assert payload['source_path'] == str(_agent_roles_archi())
    canonical_metadata = json.loads((canonical / 'install.json').read_text(encoding='utf-8'))
    assert canonical_metadata['id'] == 'agentroles.archi'
    assert canonical_metadata['source_path'] == str(_agent_roles_archi())
    assert (canonical / 'current' / 'role.toml').is_file()


def test_roles_update_legacy_ccb_archi_missing_source_falls_back_to_catalog(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    install_role('agentroles.archi', with_tools=False)
    store = tmp_path / 'xdg-data' / 'ccb' / 'roles'
    canonical = store / 'agentroles.archi'
    legacy = store / 'ccb.archi'
    canonical.rename(legacy)
    metadata_path = legacy / 'install.json'
    metadata = json.loads(metadata_path.read_text(encoding='utf-8'))
    metadata['id'] = 'ccb.archi'
    metadata['version'] = '0.1.0'
    metadata['digest'] = 'sha256:legacy'
    metadata['source_path'] = str(tmp_path / 'removed-source' / 'roles' / 'ccb.archi')
    metadata_path.write_text(json.dumps(metadata, sort_keys=True, indent=2) + '\n', encoding='utf-8')

    payload = update_role('ccb.archi', with_tools=False)

    assert payload['role_status'] == 'updated'
    assert payload['role_id'] == 'agentroles.archi'
    assert payload['source'] == 'agentroles'
    canonical_metadata = json.loads((canonical / 'install.json').read_text(encoding='utf-8'))
    assert canonical_metadata['id'] == 'agentroles.archi'
    assert canonical_metadata['source_path'] == str(_agent_roles_archi())
    assert canonical_metadata['version'] == '0.2.0'
    assert canonical_metadata['digest'] != 'sha256:legacy'


def test_roles_install_can_skip_tool_hooks_for_tests_or_advanced_use(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    sentinel = tmp_path / 'sentinel.txt'
    monkeypatch.setenv('FAKE_ROLE_SENTINEL', str(sentinel))
    script_root = tmp_path / 'ccb-root'
    _write_fake_tool_role(script_root)
    monkeypatch.setenv('AGENT_ROLES_SPEC_HOME', str(script_root))

    payload = install_role('test.fake', script_root=script_root, with_tools=False)

    assert payload['role_status'] == 'installed'
    assert payload['tools_status'] == 'skipped'
    assert not sentinel.exists()


def test_roles_install_and_update_run_tool_hooks_by_default(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    sentinel = tmp_path / 'sentinel.txt'
    monkeypatch.setenv('FAKE_ROLE_SENTINEL', str(sentinel))
    script_root = tmp_path / 'ccb-root'
    _write_fake_tool_role(script_root)
    monkeypatch.setenv('AGENT_ROLES_SPEC_HOME', str(script_root))

    install_payload = install_role('test.fake', script_root=script_root)
    assert install_payload['tools_status'] == 'ok'
    assert sentinel.read_text(encoding='utf-8') == 'install'
    installed_root = Path(str(install_payload['path']))
    assert not tuple(installed_root.rglob('__pycache__'))
    assert not tuple(installed_root.rglob('*.pyc'))
    metadata = json.loads((tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'test.fake' / 'install.json').read_text(encoding='utf-8'))
    assert metadata['digest'] == f'sha256:{tree_digest(installed_root)}'

    update_payload = update_role('test.fake', script_root=script_root)
    assert update_payload['role_status'] == 'updated'
    assert update_payload['tools_status'] == 'ok'
    assert sentinel.read_text(encoding='utf-8') == 'update'
    assert not tuple(Path(str(update_payload['path'])).rglob('__pycache__'))
    assert not tuple(Path(str(update_payload['path'])).rglob('*.pyc'))


def test_roles_install_repairs_drifted_content_addressed_target(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    source = tmp_path / 'role-source'
    role = _write_direct_role(
        source,
        'review',
        role_id='local.review',
        version='0.1.0',
        name='Local Review',
    )

    first = install_role(source_path=role, with_tools=False)
    target = Path(str(first['path']))
    drift = target / 'runtime-drift.txt'
    drift.write_text('not part of source\n', encoding='utf-8')
    assert f'sha256:{tree_digest(target)}' != first['digest']

    second = install_role(source_path=role, with_tools=False)

    assert Path(str(second['path'])) == target
    assert not drift.exists()
    assert second['digest'] == f'sha256:{tree_digest(target)}'


def test_archi_doctor_degrades_when_managed_wrapper_missing(tmp_path: Path, monkeypatch) -> None:
    home = tmp_path / 'home'
    fake_bin = tmp_path / 'bin'
    fake_bin.mkdir()
    (home / '.llmgateway').mkdir(parents=True)
    (home / '.llmgateway' / 'config.yaml').write_text('version: 1\n', encoding='utf-8')
    fake_archi = fake_bin / 'archi'
    fake_archi.write_text('#!/usr/bin/env sh\nexit 0\n', encoding='utf-8')
    fake_archi.chmod(0o755)
    monkeypatch.setenv('HOME', str(home))
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    monkeypatch.setenv('PATH', str(fake_bin))

    result = subprocess.run(
        [sys.executable, str(_agent_roles_archi() / 'adapters' / 'ccb' / 'tools' / 'doctor.py')],
        cwd=_agent_roles_archi(),
        env=dict(os.environ),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )

    assert result.returncode == 1
    assert result.stderr == ''
    assert 'architec_status: degraded' in result.stdout
    assert 'managed_wrapper_exists: False' in result.stdout
    assert 'managed_archi_binary_exists: False' in result.stdout
    assert 'selected_kind: path_archi' in result.stdout
    assert 'llmgateway_config: present' in result.stdout


def test_roles_update_cli_runs_tool_hooks_by_default(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    sentinel = tmp_path / 'sentinel.txt'
    monkeypatch.setenv('FAKE_ROLE_SENTINEL', str(sentinel))
    script_root = tmp_path / 'ccb-root'
    _write_fake_tool_role(script_root)
    monkeypatch.setenv('AGENT_ROLES_SPEC_HOME', str(script_root))

    code, out, err = _run_cli(['roles', 'update', 'test.fake'], cwd=tmp_path, script_root=script_root)

    assert code == 0
    assert err == ''
    assert 'role_status: updated' in out
    assert 'tools_status: ok' in out
    assert 'tool: id=fake action=update status=ok required=true' in out
    assert sentinel.read_text(encoding='utf-8') == 'update'


def test_roles_update_cli_can_skip_tool_hooks_for_advanced_use(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    sentinel = tmp_path / 'sentinel.txt'
    monkeypatch.setenv('FAKE_ROLE_SENTINEL', str(sentinel))
    script_root = tmp_path / 'ccb-root'
    _write_fake_tool_role(script_root)
    monkeypatch.setenv('AGENT_ROLES_SPEC_HOME', str(script_root))

    code, out, err = _run_cli(['roles', 'update', 'test.fake', '--skip-tools'], cwd=tmp_path, script_root=script_root)

    assert code == 0
    assert err == ''
    assert 'role_status: updated' in out
    assert 'tools_status: skipped' in out
    assert not sentinel.exists()


def test_roles_add_accepts_compact_role_provider_spec(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)
    install_role('agentroles.archi', script_root=REPO_ROOT, with_tools=False)

    code, out, err = _run_cli(['roles', 'add', 'agentroles.archi:codex'], cwd=project)

    assert code == 0
    assert err == ''
    assert 'role_status: added' in out
    assert 'config_binding: shorthand' in out
    text = (project / '.ccb' / 'ccb.config').read_text(encoding='utf-8')
    assert 'main = "agent1:codex, agentroles.archi:codex"' in text
    assert '[agents.archi]' not in text
    assert (project / '.ccb' / 'role-lock.json').is_file()
    metadata = json.loads((tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'agentroles.archi' / 'install.json').read_text(encoding='utf-8'))
    lock_payload = json.loads((project / '.ccb' / 'role-lock.json').read_text(encoding='utf-8'))
    assert lock_payload['roles']['agentroles.archi']['digest'] == metadata['digest']
    loaded = load_project_config(project).config
    assert loaded.agents['archi'].role == 'agentroles.archi'


def test_roles_add_accepts_provider_flag_for_compatibility(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)
    install_role('agentroles.archi', script_root=REPO_ROOT, with_tools=False)

    code, out, err = _run_cli(['roles', 'add', 'agentroles.archi', '--provider', 'codex'], cwd=project)

    assert code == 0
    assert err == ''
    assert 'config_binding: shorthand' in out
    text = (project / '.ccb' / 'ccb.config').read_text(encoding='utf-8')
    assert 'main = "agent1:codex, agentroles.archi:codex"' in text


def test_roles_add_rejects_non_single_leaf_spec(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)

    code, _out, err = _run_cli(['roles', 'add', 'agentroles.archi:codex,agent2:codex'], cwd=project)

    assert code == 1
    assert 'expected a single role leaf' in err


def test_roles_add_rejects_workspace_mode_in_compact_spec(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)

    code, _out, err = _run_cli(['roles', 'add', 'agentroles.archi:codex(worktree)'], cwd=project)

    assert code == 1
    assert 'does not accept workspace mode' in err


def test_roles_add_uses_explicit_overlay_for_custom_agent_name(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)
    install_role('agentroles.archi', script_root=REPO_ROOT, with_tools=False)

    code, out, err = _run_cli(['roles', 'add', 'agentroles.archi:codex', '--agent', 'archi-review'], cwd=project)

    assert code == 0
    assert err == ''
    assert 'config_binding: explicit' in out
    text = (project / '.ccb' / 'ccb.config').read_text(encoding='utf-8')
    assert 'main = "agent1:codex, archi-review:codex"' in text
    assert '[agents.archi-review]' in text
    assert 'role = "agentroles.archi"' in text
    loaded = load_project_config(project).config
    assert loaded.agents['archi-review'].role == 'agentroles.archi'


def test_role_id_shorthand_in_windows_resolves_to_default_agent_name(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    install_role('agentroles.archi', script_root=REPO_ROOT, with_tools=False)
    _write_project_config_text(
        project,
        '\n'.join(
            [
                'version = 2',
                'entry_window = "main"',
                '',
                '[windows]',
                'main = "agent1:codex, agentroles.archi:codex"',
            ]
        )
        + '\n',
    )

    loaded = load_project_config(project).config

    assert set(loaded.agents) == {'agent1', 'archi'}
    assert loaded.agents['archi'].role == 'agentroles.archi'
    assert loaded.windows[0].layout_spec == 'agent1:codex, archi:codex'
    assert loaded.windows[0].agent_names == ('agent1', 'archi')


def test_legacy_ccb_archi_shorthand_resolves_to_canonical_role(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    install_role('agentroles.archi', script_root=REPO_ROOT, with_tools=False)
    _write_project_config_text(
        project,
        'version = 2\nentry_window = "main"\n\n[windows]\nmain = "agent1:codex, ccb.archi:codex"\n',
    )

    loaded = load_project_config(project).config

    assert set(loaded.agents) == {'agent1', 'archi'}
    assert loaded.agents['archi'].role == 'agentroles.archi'
    assert loaded.windows[0].layout_spec == 'agent1:codex, archi:codex'


def test_role_id_shorthand_requires_installed_role(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config_text(
        project,
        'version = 2\nentry_window = "main"\n\n[windows]\nmain = "agentroles.archi:codex"\n',
    )

    with pytest.raises(Exception, match='ccb roles install agentroles.archi'):
        load_project_config(project)


def test_role_id_shorthand_in_compact_config_resolves_layout(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    install_role('agentroles.archi', script_root=REPO_ROOT, with_tools=False)
    _write_project_config_text(project, 'agent1:codex, agentroles.archi:codex\n')

    loaded = load_project_config(project).config

    assert loaded.default_agents == ('agent1', 'archi')
    assert loaded.agents['archi'].role == 'agentroles.archi'
    assert loaded.layout_spec == 'agent1:codex, archi:codex'
    assert loaded.windows[0].layout_spec == 'agent1:codex, archi:codex'


def test_role_id_shorthand_conflict_requires_explicit_binding(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    install_role('agentroles.archi', script_root=REPO_ROOT, with_tools=False)
    _write_project_config_text(
        project,
        'version = 2\nentry_window = "main"\n\n[windows]\nmain = "archi:codex, agentroles.archi:codex"\n',
    )

    with pytest.raises(Exception, match='duplicate agent across windows: archi'):
        load_project_config(project)


def test_role_memory_is_included_before_agent_private_memory(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)
    install_role('agentroles.archi', script_root=REPO_ROOT, with_tools=False)
    assert _run_cli(['roles', 'add', 'agentroles.archi', '--agent', 'archi'], cwd=project)[0] == 0
    (project / '.ccb' / 'agents' / 'archi').mkdir(parents=True)
    (project / '.ccb' / 'agents' / 'archi' / 'memory.md').write_text('agent-private\n', encoding='utf-8')

    sources = load_memory_sources(project, agent_name='archi', provider='codex')

    kinds = [source.kind for source in sources]
    assert 'role_memory' in kinds
    assert kinds.index('role_memory') < kinds.index('agent_private')
    role_sources = [source for source in sources if source.kind == 'role_memory']
    role_memory = '\n'.join(source.content for source in role_sources)
    assert len(role_sources) == 2
    assert 'architecture reviewer' in role_memory.lower()
    assert 'Architec is the architecture analysis CLI' in role_memory
    assert 'managed Architec wrapper named `ccb-archi`' in role_memory
    assert 'llmgateway secrets' in role_memory


def test_project_role_lock_blocks_silent_current_drift(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    catalog = tmp_path / 'agent-roles-spec'
    monkeypatch.setenv('AGENT_ROLES_SPEC_HOME', str(catalog))
    _write_memory_catalog_role(catalog, memory_text='locked memory v1')
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)
    install_role('test.locked', with_tools=False)
    assert _run_cli(['roles', 'add', 'test.locked:codex'], cwd=project)[0] == 0
    lock_payload = json.loads((project / '.ccb' / 'role-lock.json').read_text(encoding='utf-8'))
    locked_digest = str(lock_payload['roles']['test.locked']['digest'])
    locked_digest_hex = locked_digest.removeprefix('sha256:')
    locked_path = tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'test.locked' / 'versions' / '1.0.0' / locked_digest_hex
    assert locked_path.is_dir()

    _write_memory_catalog_role(catalog, default_agent_name='drifted', memory_text='drifted memory v2')
    update_role('test.locked', with_tools=False)
    current_path = (tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'test.locked' / 'current').resolve()

    loaded = load_project_config(project).config
    sources = load_memory_sources(project, agent_name='locked', provider='codex')
    warnings = [source.warning for source in sources if source.kind == 'role_memory' and source.warning]
    role_memory = '\n'.join(source.content for source in sources if source.kind == 'role_memory')
    source_home = tmp_path / 'source-codex'
    source_home.mkdir()
    target_home = tmp_path / 'managed-codex'
    materialize_codex_home_config(
        target_home,
        source_home=source_home,
        project_root=project,
        agent_name='locked',
        workspace_path=project,
    )
    rendered_memory = (target_home / 'AGENTS.md').read_text(encoding='utf-8')

    assert set(loaded.agents) == {'agent1', 'locked'}
    assert 'drifted' not in loaded.agents
    assert warnings == []
    assert current_path != locked_path
    assert 'locked memory v1' in role_memory
    assert 'drifted memory v2' not in role_memory
    assert 'locked memory v1' in rendered_memory
    assert 'role_lock_mismatch: test.locked' not in rendered_memory
    assert 'drifted memory v2' not in rendered_memory


def test_codex_role_skills_project_to_managed_home(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)
    install_role('agentroles.archi', script_root=REPO_ROOT, with_tools=False)
    assert _run_cli(['roles', 'add', 'agentroles.archi', '--agent', 'archi'], cwd=project)[0] == 0
    source_home = tmp_path / 'source-codex'
    source_home.mkdir()
    target_home = tmp_path / 'managed-codex'

    materialize_codex_home_config(
        target_home,
        source_home=source_home,
        project_root=project,
        agent_name='archi',
        workspace_path=project,
    )

    projected = target_home / 'skills' / 'archi-diff' / 'SKILL.md'
    assert projected.is_file()
    assert 'architecture risk' in projected.read_text(encoding='utf-8')
    assert (target_home / 'skills' / 'archi-diff.ccb-projection.json').is_file()
