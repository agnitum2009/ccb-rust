from __future__ import annotations

from io import StringIO
from pathlib import Path

import pytest

from agents.config_loader import load_project_config
from cli.entrypoint import run_cli_entrypoint
from project_memory import load_memory_sources
from provider_profiles.codex_home_config import materialize_codex_home_config
from rolepacks import RoleManifestError, install_role, load_installed_role, load_role_manifest, update_role


REPO_ROOT = Path(__file__).resolve().parents[1]


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
                'target = Path(os.environ["FAKE_ROLE_SENTINEL"])',
                'target.write_text(os.environ["CCB_ROLE_TOOL_ACTION"], encoding="utf-8")',
                'print("hook_action: " + os.environ["CCB_ROLE_TOOL_ACTION"])',
            ]
        )
        + '\n',
        encoding='utf-8',
    )


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


def test_roles_list_show_and_install_use_system_store(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))

    code, out, err = _run_cli(['roles', 'list'], cwd=tmp_path)
    assert code == 0
    assert err == ''
    assert 'roles_status: ok' in out
    assert 'role: id=ccb.archi' in out

    code, out, err = _run_cli(['roles', 'show', 'ccb.archi'], cwd=tmp_path)
    assert code == 0
    assert err == ''
    assert 'id: ccb.archi' in out

    code, out, err = _run_cli(['roles', 'install', 'ccb.archi', '--skip-tools'], cwd=tmp_path)
    assert code == 0
    assert err == ''
    assert 'role_status: installed' in out
    assert load_installed_role('ccb.archi') is not None
    assert (tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'ccb.archi' / 'install.json').is_file()


def test_roles_install_can_skip_tool_hooks_for_tests_or_advanced_use(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    sentinel = tmp_path / 'sentinel.txt'
    monkeypatch.setenv('FAKE_ROLE_SENTINEL', str(sentinel))
    script_root = tmp_path / 'ccb-root'
    _write_fake_tool_role(script_root)

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

    install_payload = install_role('test.fake', script_root=script_root)
    assert install_payload['tools_status'] == 'ok'
    assert sentinel.read_text(encoding='utf-8') == 'install'

    update_payload = update_role('test.fake', script_root=script_root)
    assert update_payload['role_status'] == 'updated'
    assert update_payload['tools_status'] == 'ok'
    assert sentinel.read_text(encoding='utf-8') == 'update'


def test_roles_update_cli_runs_tool_hooks_by_default(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    sentinel = tmp_path / 'sentinel.txt'
    monkeypatch.setenv('FAKE_ROLE_SENTINEL', str(sentinel))
    script_root = tmp_path / 'ccb-root'
    _write_fake_tool_role(script_root)

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
    install_role('ccb.archi', script_root=REPO_ROOT, with_tools=False)

    code, out, err = _run_cli(['roles', 'add', 'ccb.archi:codex'], cwd=project)

    assert code == 0
    assert err == ''
    assert 'role_status: added' in out
    assert 'config_binding: shorthand' in out
    text = (project / '.ccb' / 'ccb.config').read_text(encoding='utf-8')
    assert 'main = "agent1:codex, ccb.archi:codex"' in text
    assert '[agents.archi]' not in text
    assert (project / '.ccb' / 'role-lock.json').is_file()
    loaded = load_project_config(project).config
    assert loaded.agents['archi'].role == 'ccb.archi'


def test_roles_add_accepts_provider_flag_for_compatibility(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)
    install_role('ccb.archi', script_root=REPO_ROOT, with_tools=False)

    code, out, err = _run_cli(['roles', 'add', 'ccb.archi', '--provider', 'codex'], cwd=project)

    assert code == 0
    assert err == ''
    assert 'config_binding: shorthand' in out
    text = (project / '.ccb' / 'ccb.config').read_text(encoding='utf-8')
    assert 'main = "agent1:codex, ccb.archi:codex"' in text


def test_roles_add_rejects_non_single_leaf_spec(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)

    code, _out, err = _run_cli(['roles', 'add', 'ccb.archi:codex,agent2:codex'], cwd=project)

    assert code == 1
    assert 'expected a single role leaf' in err


def test_roles_add_rejects_workspace_mode_in_compact_spec(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)

    code, _out, err = _run_cli(['roles', 'add', 'ccb.archi:codex(worktree)'], cwd=project)

    assert code == 1
    assert 'does not accept workspace mode' in err


def test_roles_add_uses_explicit_overlay_for_custom_agent_name(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)
    install_role('ccb.archi', script_root=REPO_ROOT, with_tools=False)

    code, out, err = _run_cli(['roles', 'add', 'ccb.archi:codex', '--agent', 'archi-review'], cwd=project)

    assert code == 0
    assert err == ''
    assert 'config_binding: explicit' in out
    text = (project / '.ccb' / 'ccb.config').read_text(encoding='utf-8')
    assert 'main = "agent1:codex, archi-review:codex"' in text
    assert '[agents.archi-review]' in text
    assert 'role = "ccb.archi"' in text
    loaded = load_project_config(project).config
    assert loaded.agents['archi-review'].role == 'ccb.archi'


def test_role_id_shorthand_in_windows_resolves_to_default_agent_name(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    install_role('ccb.archi', script_root=REPO_ROOT, with_tools=False)
    _write_project_config_text(
        project,
        '\n'.join(
            [
                'version = 2',
                'entry_window = "main"',
                '',
                '[windows]',
                'main = "agent1:codex, ccb.archi:codex"',
            ]
        )
        + '\n',
    )

    loaded = load_project_config(project).config

    assert set(loaded.agents) == {'agent1', 'archi'}
    assert loaded.agents['archi'].role == 'ccb.archi'
    assert loaded.windows[0].layout_spec == 'agent1:codex, archi:codex'
    assert loaded.windows[0].agent_names == ('agent1', 'archi')


def test_role_id_shorthand_requires_installed_role(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config_text(
        project,
        'version = 2\nentry_window = "main"\n\n[windows]\nmain = "ccb.archi:codex"\n',
    )

    with pytest.raises(Exception, match='ccb roles install ccb.archi'):
        load_project_config(project)


def test_role_id_shorthand_in_compact_config_resolves_layout(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    install_role('ccb.archi', script_root=REPO_ROOT, with_tools=False)
    _write_project_config_text(project, 'agent1:codex, ccb.archi:codex\n')

    loaded = load_project_config(project).config

    assert loaded.default_agents == ('agent1', 'archi')
    assert loaded.agents['archi'].role == 'ccb.archi'
    assert loaded.layout_spec == 'agent1:codex, archi:codex'
    assert loaded.windows[0].layout_spec == 'agent1:codex, archi:codex'


def test_role_id_shorthand_conflict_requires_explicit_binding(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    install_role('ccb.archi', script_root=REPO_ROOT, with_tools=False)
    _write_project_config_text(
        project,
        'version = 2\nentry_window = "main"\n\n[windows]\nmain = "archi:codex, ccb.archi:codex"\n',
    )

    with pytest.raises(Exception, match='duplicate agent across windows: archi'):
        load_project_config(project)


def test_role_memory_is_included_before_agent_private_memory(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)
    install_role('ccb.archi', script_root=REPO_ROOT, with_tools=False)
    assert _run_cli(['roles', 'add', 'ccb.archi', '--agent', 'archi'], cwd=project)[0] == 0
    (project / '.ccb' / 'agents' / 'archi').mkdir(parents=True)
    (project / '.ccb' / 'agents' / 'archi' / 'memory.md').write_text('agent-private\n', encoding='utf-8')

    sources = load_memory_sources(project, agent_name='archi', provider='codex')

    kinds = [source.kind for source in sources]
    assert 'role_memory' in kinds
    assert kinds.index('role_memory') < kinds.index('agent_private')
    role_source = next(source for source in sources if source.kind == 'role_memory')
    assert 'architecture reviewer' in role_source.content.lower()


def test_codex_role_skills_project_to_managed_home(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config(project)
    install_role('ccb.archi', script_root=REPO_ROOT, with_tools=False)
    assert _run_cli(['roles', 'add', 'ccb.archi', '--agent', 'archi'], cwd=project)[0] == 0
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
    assert 'architecture analysis' in projected.read_text(encoding='utf-8')
    assert (target_home / 'skills' / 'archi-diff.ccb-projection.json').is_file()
