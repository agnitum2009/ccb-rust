from __future__ import annotations

from io import StringIO
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import zipfile

import pytest

import rolepacks.agent_roles_manager as agent_roles_manager
import rolepacks.sources as role_sources
from agents.config_loader import load_project_config
from cli.entrypoint import run_cli_entrypoint
from project_memory import load_memory_sources
from provider_profiles.codex_home_config import materialize_codex_home_config
from rolepacks.manifest import RoleManifestError, load_role_manifest
from rolepacks.runtime_lookup import tree_digest
from rolepacks.service import builtin_role_root, install_role, load_installed_role, role_status, run_role_tool_hooks, sync_roles_from_path, update_role
from rolepacks.sources import (
    DEFAULT_AGENT_ROLES_SPEC_GIT_URL,
    add_role_source,
    default_agent_roles_source,
    discover_source_roles,
    migrate_legacy_installed_roles,
    role_catalog_status,
)


REPO_ROOT = Path(__file__).resolve().parents[1]


def _agent_roles_archi() -> Path:
    return Path(os.environ['AGENT_ROLES_SPEC_HOME']) / 'roles' / 'archi'


def _agent_roles_installed_root(tmp_path: Path) -> Path:
    return tmp_path / '.roles' / 'installed'


@pytest.fixture(autouse=True)
def _agent_roles_catalog(monkeypatch, tmp_path: Path) -> None:
    agent_roles_spec = tmp_path / 'default-agent-roles-spec'
    _write_agent_roles_archi_fixture(agent_roles_spec)
    fake_cli = _write_fake_agent_roles_cli(tmp_path)
    monkeypatch.setenv('HOME', str(tmp_path / 'home'))
    monkeypatch.setenv('AGENT_ROLES_SPEC_HOME', str(agent_roles_spec))
    monkeypatch.setenv('AGENT_ROLES_CLI', f'{sys.executable} {fake_cli}')
    monkeypatch.setenv('AGENT_ROLES_STORE', str(tmp_path / '.roles'))
    monkeypatch.delenv('CCB_AGENT_ROLES_INCLUDE_REFERENCE', raising=False)


def _write_fake_agent_roles_cli(tmp_path: Path) -> Path:
    script = tmp_path / 'fake-agent-roles.py'
    script.write_text(
        r'''from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import importlib


def _canonical_role_id(value: str | None) -> str | None:
    if value is None:
        return None
    text = str(value).strip().lower()
    if text == "ccb.archi":
        return "agentroles.archi"
    return text


def _store_root() -> Path:
    raw = str(os.environ.get("AGENT_ROLES_STORE") or "").strip()
    return (Path(raw).expanduser() if raw else Path.home() / ".roles") / "installed"


def _tree_digest(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(Path(root).rglob("*")):
        rel = path.relative_to(root)
        digest.update(str(rel).encode("utf-8"))
        digest.update(b"\0")
        if path.is_file():
            digest.update(path.read_bytes())
        elif path.is_symlink():
            digest.update(str(path.readlink()).encode("utf-8"))
        digest.update(b"\0")
    return digest.hexdigest()


def _role_manifest(path: Path) -> dict[str, object]:
    text = (path / "role.toml").read_text(encoding="utf-8")
    for module_name in ("tomllib", "tomli", "toml"):
        try:
            module = importlib.import_module(module_name)
            break
        except ModuleNotFoundError:
            continue
    else:
        return _parse_minimal_toml(text)
    return module.loads(text)


def _parse_minimal_toml(text: str) -> dict[str, object]:
    root: dict[str, object] = {}
    current: dict[str, object] = root
    for raw_line in text.splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            current = root
            for part in line[1:-1].split("."):
                table = current.setdefault(part.strip(), {})
                if not isinstance(table, dict):
                    table = {}
                    current[part.strip()] = table
                current = table
            continue
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        current[key.strip()] = _parse_minimal_toml_value(value.strip())
    return root


def _parse_minimal_toml_value(value: str) -> object:
    if value.startswith("[") and value.endswith("]"):
        inner = value[1:-1].strip()
        if not inner:
            return []
        return [_parse_minimal_toml_value(item.strip()) for item in inner.split(",")]
    if (value.startswith('"') and value.endswith('"')) or (value.startswith("'") and value.endswith("'")):
        return value[1:-1]
    return value


def _iter_role_paths(root: Path) -> list[Path]:
    root = root.expanduser()
    paths: list[Path] = []
    for base in (root / "roles", root):
        if not base.is_dir():
            continue
        for child in sorted(base.iterdir(), key=lambda item: item.name):
            if child.is_dir() and (child / "role.toml").is_file():
                paths.append(child)
    if (root / "role.toml").is_file():
        paths.append(root)
    deduped: list[Path] = []
    seen: set[Path] = set()
    for path in paths:
        resolved = path.resolve()
        if resolved in seen:
            continue
        deduped.append(path)
        seen.add(resolved)
    return deduped


def _find_source(role_id: str | None, source_path: Path | None) -> Path:
    if source_path is not None:
        return source_path.expanduser().resolve()
    if not role_id:
        raise SystemExit(_error("role id is required unless --path is provided"))
    metadata_path = _store_root() / role_id / "install.json"
    if metadata_path.is_file():
        try:
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
        except Exception:
            metadata = {}
        installed_source = Path(str(metadata.get("source_path") or "")).expanduser()
        if installed_source.is_dir():
            return installed_source.resolve()
    catalog = Path(os.environ["AGENT_ROLES_SPEC_HOME"])
    for path in _iter_role_paths(catalog):
        try:
            manifest = _role_manifest(path)
        except Exception:
            continue
        if _canonical_role_id(str(manifest.get("id") or "")) == role_id:
            return path.resolve()
    raise SystemExit(_error(f"role source not found: {role_id}"))


def _copy_install(role_id: str | None, source_path: Path | None, *, status: str) -> dict[str, object]:
    requested_id = _canonical_role_id(role_id)
    source_kind = "path" if source_path is not None else "agentroles"
    if source_path is None and requested_id:
        metadata_path = _store_root() / requested_id / "install.json"
        if metadata_path.is_file():
            try:
                metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            except Exception:
                metadata = {}
            installed_source = Path(str(metadata.get("source_path") or "")).expanduser()
            if installed_source.is_dir():
                source_kind = str(metadata.get("source") or "path")
    source = _find_source(role_id, source_path)
    manifest = _role_manifest(source)
    actual_id = _canonical_role_id(str(manifest.get("id") or ""))
    version = str(manifest.get("version") or "")
    source_digest = _tree_digest(source)
    target = _store_root() / actual_id / "versions" / version / source_digest
    if target.exists():
        shutil.rmtree(target)
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source, target, symlinks=True)
    current = _store_root() / actual_id / "current"
    if current.exists() or current.is_symlink():
        if current.is_symlink() or current.is_file():
            current.unlink()
        else:
            shutil.rmtree(current)
    try:
        current.symlink_to(target, target_is_directory=True)
    except OSError:
        shutil.copytree(target, current)
    metadata = {
        "schema": "agent-roles-install/v1",
        "id": actual_id,
        "version": version,
        "digest": f"sha256:{source_digest}",
        "source": source_kind,
        "source_path": str(source),
    }
    metadata_path = _store_root() / actual_id / "install.json"
    metadata_path.parent.mkdir(parents=True, exist_ok=True)
    metadata_path.write_text(json.dumps(metadata, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    return {
        "schema": f"agent-roles/{status}/v1",
        "status": "ok",
        "role_status": "installed" if status == "install" else "updated",
        "role_id": actual_id,
        "version": version,
        "digest": f"sha256:{source_digest}",
        "path": str(target),
        "source": metadata["source"],
    }


def _sync(path: Path) -> dict[str, object]:
    rows: list[dict[str, object]] = []
    for source in _iter_role_paths(path):
        manifest = _role_manifest(source)
        role_id = _canonical_role_id(str(manifest.get("id") or ""))
        version = str(manifest.get("version") or "")
        source_digest = f"sha256:{_tree_digest(source)}"
        metadata_path = _store_root() / role_id / "install.json"
        if not metadata_path.is_file():
            rows.append({
                "role_id": role_id,
                "status": "skipped_not_installed",
                "version": version,
                "digest": source_digest,
                "path": str(source),
            })
            continue
        metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
        if metadata.get("version") == version and metadata.get("digest") == source_digest:
            rows.append({
                "role_id": role_id,
                "status": "current",
                "version": version,
                "digest": source_digest,
                "path": str(source),
            })
            continue
        payload = _copy_install(role_id, source, status="install")
        rows.append({
            "role_id": role_id,
            "status": "synced",
            "version": payload["version"],
            "digest": payload["digest"],
            "path": payload["path"],
        })
    return {"schema": "agent-roles/sync/v1", "status": "ok", "path": str(path), "roles": rows}


def _error(message: str) -> int:
    print(json.dumps({"status": "failed", "error": message}), file=sys.stderr)
    return 1


def main() -> int:
    args = [arg for arg in sys.argv[1:] if arg != "--json"]
    if not args:
        return _error("missing command")
    command = args.pop(0)
    raw_target: str | None = None
    source_path: Path | None = None
    index = 0
    while index < len(args):
        arg = args[index]
        if arg == "--path":
            source_path = Path(args[index + 1])
            index += 2
            continue
        if raw_target is None:
            raw_target = arg
        index += 1
    if command == "install":
        role_id = _canonical_role_id(raw_target)
        print(json.dumps(_copy_install(role_id, source_path, status="install")))
        return 0
    if command == "update":
        role_id = _canonical_role_id(raw_target)
        print(json.dumps(_copy_install(role_id, source_path, status="update")))
        return 0
    if command == "sync":
        if raw_target is None:
            return _error("sync path is required")
        print(json.dumps(_sync(Path(raw_target))))
        return 0
    return _error(f"unknown command: {command}")


raise SystemExit(main())
''',
        encoding='utf-8',
    )
    return script


def _write_legacy_installed_role(
    tmp_path: Path,
    *,
    role_dir_name: str = 'agentroles.archi',
    source: Path | None = None,
) -> dict[str, object]:
    source = source or _agent_roles_archi()
    role = load_role_manifest(source)
    digest = tree_digest(source)
    role_dir = tmp_path / 'xdg-data' / 'ccb' / 'roles' / role_dir_name
    target = role_dir / 'versions' / role.version / digest
    shutil.copytree(source, target, symlinks=True)
    current = role_dir / 'current'
    current.symlink_to(target, target_is_directory=True)
    metadata = {
        'schema': 'rolepack-install/v1',
        'id': role_dir_name,
        'version': role.version,
        'source': 'agentroles',
        'source_path': str(source),
        'digest': f'sha256:{digest}',
    }
    (role_dir / 'install.json').write_text(json.dumps(metadata, sort_keys=True, indent=2) + '\n', encoding='utf-8')
    return {
        'role_id': role.id,
        'version': role.version,
        'digest': f'sha256:{digest}',
        'path': str(target),
        'role_dir': str(role_dir),
    }


def _write_spec_installed_role(tmp_path: Path, source: Path) -> dict[str, object]:
    role = load_role_manifest(source)
    digest = tree_digest(source)
    role_dir = _agent_roles_installed_root(tmp_path) / role.id
    target = role_dir / 'versions' / role.version / digest
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
        'source': 'agentroles',
        'source_path': str(source),
        'digest': f'sha256:{digest}',
    }
    (role_dir / 'install.json').write_text(json.dumps(metadata, sort_keys=True, indent=2) + '\n', encoding='utf-8')
    return {
        'role_id': role.id,
        'version': role.version,
        'digest': f'sha256:{digest}',
        'path': str(target),
        'role_dir': str(role_dir),
    }


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
        'Architecture Reviewer Memory\n'
        'Architec is the architecture analysis CLI installed from the @seemseam/architec npm package.\n'
        'The package also provides the Hippo and llmgateway capabilities Archi uses.\n',
        encoding='utf-8',
    )
    (role / 'adapters' / 'ccb' / 'memory.md').write_text(
        'CCB Adapter Memory\n'
        'Use the `archi` CLI provided by the global `@seemseam/architec` npm package.\n'
        'If Archi, Hippo, or llmgateway commands are missing, install or update `@seemseam/architec`.\n'
        'Do not split Hippo or llmgateway into CCB-managed pip, venv, git, or editable installs.\n'
        'Do not copy llmgateway secrets into role memory.\n',
        encoding='utf-8',
    )
    for action in ('install', 'update'):
        (role / 'adapters' / 'ccb' / 'tools' / f'{action}.py').write_text(
            f'print("architec_status: ok\\naction: {action}\\npackage: @seemseam/architec\\ninstall_command: npm install -g @seemseam/architec")\n',
            encoding='utf-8',
        )
    (role / 'adapters' / 'ccb' / 'tools' / 'doctor.py').write_text(
        '\n'.join(
            [
                'from __future__ import annotations',
                '',
                'import shutil',
                'import subprocess',
                '',
                '',
                'def _probe(path: str | None) -> str:',
                '    if not path:',
                "        return 'missing'",
                '    try:',
                "        result = subprocess.run([path, '--help'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=20, check=False)",
                '    except Exception:',
                "        return 'failed'",
                "    return 'ok' if result.returncode == 0 else 'failed'",
                '',
                '',
                'def main() -> int:',
                "    path_archi = shutil.which('archi')",
                "    archi_help = _probe(path_archi)",
                "    help_text = ''",
                "    if path_archi:",
                '        result = subprocess.run([path_archi, "--help"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=20, check=False)',
                "        help_text = (result.stdout + result.stderr).lower()",
                "    bundled_hippo = 'available' if '--refresh-from-hippos' in help_text or 'hippo' in help_text else 'unknown'",
                "    bundled_llmgateway = 'available' if '--check' in help_text or '--full' in help_text or 'llm' in help_text else 'unknown'",
                "    if not path_archi or archi_help == 'failed':",
                "        status = 'missing' if not path_archi else 'failed'",
                "    elif bundled_hippo != 'available' or bundled_llmgateway != 'available':",
                "        status = 'degraded'",
                '    else:',
                "        status = 'ok'",
                "    print(f'architec_status: {status}')",
                "    print('package: @seemseam/architec')",
                "    print('install_command: npm install -g @seemseam/architec')",
                "    print('archi_binary: ' + (path_archi or ''))",
                "    print('archi_help: ' + archi_help)",
                "    print('bundled_hippo: ' + bundled_hippo)",
                "    print('bundled_llmgateway: ' + bundled_llmgateway)",
                "    return 0 if status in {'ok', 'degraded'} else 1",
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
    assert (_agent_roles_installed_root(tmp_path) / 'local.review' / 'install.json').is_file()
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
    metadata_path = _agent_roles_installed_root(tmp_path) / 'local.review' / 'install.json'
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
    assert not (_agent_roles_installed_root(tmp_path) / 'local.missing' / 'install.json').exists()
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


def test_catalog_discovery_downloads_archive_when_git_is_missing(
    tmp_path: Path,
    monkeypatch,
) -> None:
    monkeypatch.delenv('AGENT_ROLES_SPEC_HOME', raising=False)
    monkeypatch.delenv('CCB_AGENT_ROLES_SPEC_HOME', raising=False)
    monkeypatch.setenv('HOME', str(tmp_path / 'home'))
    monkeypatch.setenv('XDG_CACHE_HOME', str(tmp_path / 'xdg-cache'))
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    remote_catalog = tmp_path / 'remote-agent-roles-spec-main'
    _write_catalog_role(
        remote_catalog,
        'roles',
        'remote',
        role_id='agentroles.remote',
        version='0.1.0',
        name='Remote Role',
    )
    archive_path = tmp_path / 'agent-roles-spec.zip'
    with zipfile.ZipFile(archive_path, 'w') as archive:
        for path in remote_catalog.rglob('*'):
            if path.is_file():
                archive.write(path, path.relative_to(tmp_path))

    def missing_git(cmd, **kwargs):
        raise FileNotFoundError(cmd[0])

    monkeypatch.setattr(role_sources.subprocess, 'run', missing_git)
    monkeypatch.setenv('CCB_AGENT_ROLES_SPEC_ARCHIVE_URL', archive_path.as_uri())

    source = default_agent_roles_source()
    rows = {str(row['role_id']): row for row in role_catalog_status()}

    expected_cache = tmp_path / 'xdg-cache' / 'ccb' / 'role-catalogs' / 'agent-roles-spec'
    assert source == expected_cache.resolve()
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


def test_roles_list_show_and_install_use_agent_roles_store(tmp_path: Path, monkeypatch) -> None:
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
    assert (_agent_roles_installed_root(tmp_path) / 'agentroles.archi' / 'install.json').is_file()


def test_load_installed_role_reads_spec_owned_roles_store(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    monkeypatch.setenv('AGENT_ROLES_STORE', str(tmp_path / '.roles'))
    source = _agent_roles_archi()
    digest = tree_digest(source)
    target = tmp_path / '.roles' / 'installed' / 'agentroles.archi' / 'versions' / '0.2.0' / digest
    shutil.copytree(source, target)
    current = tmp_path / '.roles' / 'installed' / 'agentroles.archi' / 'current'
    current.symlink_to(target, target_is_directory=True)
    metadata = {
        'schema': 'agent-roles-install/v1',
        'id': 'agentroles.archi',
        'version': '0.2.0',
        'digest': f'sha256:{digest}',
        'source': 'agentroles',
        'source_path': str(source),
    }
    (tmp_path / '.roles' / 'installed' / 'agentroles.archi' / 'install.json').write_text(
        json.dumps(metadata, sort_keys=True, indent=2) + '\n',
        encoding='utf-8',
    )

    role = load_installed_role('agentroles.archi')
    rows = {str(row['role_id']): row for row in role_catalog_status()}

    assert role is not None
    assert role.id == 'agentroles.archi'
    assert rows['agentroles.archi']['status'] == 'current'
    assert rows['agentroles.archi']['installed_digest'] == f'sha256:{digest}'


def test_roles_install_delegates_to_agent_roles_manager_by_default(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    monkeypatch.setenv('AGENT_ROLES_STORE', str(tmp_path / '.roles'))
    fake_cli = tmp_path / 'agent-roles-fake.py'
    fake_cli.write_text(
        f'''from __future__ import annotations
import json
import os
from pathlib import Path
import shutil
import sys

args = sys.argv[1:]
source = Path(os.environ["AGENT_ROLES_SPEC_HOME"]) / "roles" / "archi"
root = Path(os.environ["AGENT_ROLES_STORE"]) / "installed" / "agentroles.archi"
target = root / "versions" / "0.2.0" / "fake-digest"
if target.exists():
    shutil.rmtree(target)
target.parent.mkdir(parents=True, exist_ok=True)
shutil.copytree(source, target)
current = root / "current"
if current.exists() or current.is_symlink():
    current.unlink()
current.symlink_to(target, target_is_directory=True)
(root / "install.json").write_text(json.dumps({{
    "schema": "agent-roles-install/v1",
    "id": "agentroles.archi",
    "version": "0.2.0",
    "digest": "sha256:fake-digest",
    "source": "agentroles",
    "source_path": str(source),
}}, sort_keys=True, indent=2) + "\\n", encoding="utf-8")
print(json.dumps({{
    "schema": "agent-roles/install/v1",
    "status": "ok",
    "role_status": "installed",
    "role_id": "agentroles.archi",
    "version": "0.2.0",
    "digest": "sha256:fake-digest",
    "path": str(target),
    "source": "agentroles",
    "store_root": os.environ["AGENT_ROLES_STORE"],
}}))
''',
        encoding='utf-8',
    )
    monkeypatch.setenv('AGENT_ROLES_CLI', f'{sys.executable} {fake_cli}')

    payload = install_role('agentroles.archi', with_tools=False)

    assert payload['role_status'] == 'installed'
    assert payload['role_id'] == 'agentroles.archi'
    assert payload['path'].endswith('/.roles/installed/agentroles.archi/versions/0.2.0/fake-digest')
    assert load_installed_role('agentroles.archi') is not None
    assert (tmp_path / '.roles' / 'installed' / 'agentroles.archi' / 'install.json').is_file()
    assert not (tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'agentroles.archi' / 'install.json').exists()


def test_legacy_ccb_store_migrates_to_spec_owned_store(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    monkeypatch.setenv('AGENT_ROLES_STORE', str(tmp_path / '.roles'))
    legacy_payload = _write_legacy_installed_role(tmp_path)
    legacy_metadata = tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'agentroles.archi' / 'install.json'
    spec_metadata = tmp_path / '.roles' / 'installed' / 'agentroles.archi' / 'install.json'

    result = migrate_legacy_installed_roles()
    role = load_installed_role('agentroles.archi')
    metadata = json.loads(spec_metadata.read_text(encoding='utf-8'))

    assert result['migration_status'] == 'ok'
    assert result['migrated'] == 1
    assert legacy_metadata.is_file()
    assert spec_metadata.is_file()
    assert metadata['schema'] == 'agent-roles-install/v1'
    assert metadata['id'] == 'agentroles.archi'
    assert metadata['digest'] == legacy_payload['digest']
    assert role is not None
    assert str(role.root).startswith(str(tmp_path / '.roles' / 'installed'))


def test_roles_update_manager_migrates_legacy_store_before_delegating(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    monkeypatch.setenv('AGENT_ROLES_STORE', str(tmp_path / '.roles'))
    _write_legacy_installed_role(tmp_path)
    fake_cli = tmp_path / 'agent-roles-update-fake.py'
    fake_cli.write_text(
        '''from __future__ import annotations
import json
import os
from pathlib import Path
import sys

root = Path(os.environ["AGENT_ROLES_STORE"]) / "installed" / "agentroles.archi"
metadata_path = root / "install.json"
if not metadata_path.is_file():
    print(json.dumps({"status": "failed", "error": "legacy store was not migrated"}), file=sys.stderr)
    raise SystemExit(1)
metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
current = root / "current"
print(json.dumps({
    "schema": "agent-roles/update/v1",
    "status": "ok",
    "role_status": "updated",
    "role_id": "agentroles.archi",
    "version": metadata["version"],
    "digest": metadata["digest"],
    "path": str(current.resolve()),
    "source": metadata.get("source", "agentroles"),
}))
''',
        encoding='utf-8',
    )
    monkeypatch.setenv('AGENT_ROLES_CLI', f'{sys.executable} {fake_cli}')

    payload = update_role('agentroles.archi', with_tools=False)

    assert payload['role_status'] == 'updated'
    assert (tmp_path / '.roles' / 'installed' / 'agentroles.archi' / 'install.json').is_file()


def test_roles_update_path_uses_agent_roles_manager_store_by_default(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    monkeypatch.setenv('AGENT_ROLES_STORE', str(tmp_path / '.roles'))
    fake_cli = tmp_path / 'agent-roles-path-update-fake.py'
    fake_cli.write_text(
        '''from __future__ import annotations
import json
import os
from pathlib import Path
import shutil
import sys

args = sys.argv[1:]
source = Path(args[args.index("--path") + 1])
root = Path(os.environ["AGENT_ROLES_STORE"]) / "installed" / "agentroles.archi"
target = root / "versions" / "0.2.0" / "path-update-digest"
if target.exists():
    shutil.rmtree(target)
target.parent.mkdir(parents=True, exist_ok=True)
shutil.copytree(source, target)
current = root / "current"
if current.exists() or current.is_symlink():
    current.unlink()
current.symlink_to(target, target_is_directory=True)
(root / "install.json").write_text(json.dumps({
    "schema": "agent-roles-install/v1",
    "id": "agentroles.archi",
    "version": "0.2.0",
    "digest": "sha256:path-update-digest",
    "source": "path",
    "source_path": str(source),
}, sort_keys=True, indent=2) + "\\n", encoding="utf-8")
print(json.dumps({
    "schema": "agent-roles/install/v1",
    "status": "ok",
    "role_status": "installed",
    "role_id": "agentroles.archi",
    "version": "0.2.0",
    "digest": "sha256:path-update-digest",
    "path": str(target),
    "source": "path",
}))
''',
        encoding='utf-8',
    )
    monkeypatch.setenv('AGENT_ROLES_CLI', f'{sys.executable} {fake_cli}')

    payload = update_role('agentroles.archi', source_path=_agent_roles_archi(), with_tools=False)

    assert payload['role_status'] == 'updated'
    assert payload['path'].endswith('/.roles/installed/agentroles.archi/versions/0.2.0/path-update-digest')
    assert (tmp_path / '.roles' / 'installed' / 'agentroles.archi' / 'install.json').is_file()
    assert not (tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'agentroles.archi' / 'install.json').exists()


def test_agent_roles_manager_sync_rejects_malformed_roles_payload(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setattr(
        agent_roles_manager,
        'sync',
        lambda _path: {'schema': 'agent-roles/sync/v1', 'status': 'ok', 'roles': [{'role_id': 'ok'}, 'bad']},
    )

    with pytest.raises(Exception, match='invalid sync roles payload'):
        sync_roles_from_path(tmp_path)


def test_agent_roles_manager_uses_importable_module_when_command_missing(monkeypatch) -> None:
    monkeypatch.delenv('AGENT_ROLES_CLI', raising=False)
    monkeypatch.setattr(agent_roles_manager.shutil, 'which', lambda _name: None)
    monkeypatch.setattr(agent_roles_manager, '_agent_roles_source_root', lambda: None)
    monkeypatch.setattr(
        agent_roles_manager.importlib.util,
        'find_spec',
        lambda name: object() if name == 'agent_roles' else None,
    )

    command, cwd, _env = agent_roles_manager._command_context()

    assert command == [sys.executable, '-m', 'agent_roles']
    assert cwd is None


def test_agent_roles_manager_non_json_failure_reports_exit_code(monkeypatch) -> None:
    monkeypatch.setattr(agent_roles_manager, '_command_context', lambda: (['agent-roles'], None, {}))
    monkeypatch.setattr(agent_roles_manager, '_timeout_seconds', lambda: 1.0)

    def fake_run(*_args, **_kwargs):
        return subprocess.CompletedProcess(
            args=['agent-roles'],
            returncode=2,
            stdout='',
            stderr='Traceback: missing package',
        )

    monkeypatch.setattr(agent_roles_manager.subprocess, 'run', fake_run)

    with pytest.raises(agent_roles_manager.AgentRolesManagerError) as excinfo:
        agent_roles_manager.install('agentroles.archi')

    message = str(excinfo.value)
    assert 'failed with exit code 2' in message
    assert 'Traceback: missing package' in message
    assert 'invalid JSON' not in message


def test_agent_roles_manager_structured_failure_uses_error(monkeypatch) -> None:
    monkeypatch.setattr(agent_roles_manager, '_command_context', lambda: (['agent-roles'], None, {}))
    monkeypatch.setattr(agent_roles_manager, '_timeout_seconds', lambda: 1.0)

    def fake_run(*_args, **_kwargs):
        return subprocess.CompletedProcess(
            args=['agent-roles'],
            returncode=1,
            stdout='{"status":"failed","error":"role source not found"}',
            stderr='',
        )

    monkeypatch.setattr(agent_roles_manager.subprocess, 'run', fake_run)

    with pytest.raises(agent_roles_manager.AgentRolesManagerError) as excinfo:
        agent_roles_manager.install('agentroles.missing')

    assert str(excinfo.value) == 'role source not found'


def test_roles_install_manager_missing_cli_reports_failed_without_traceback(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('AGENT_ROLES_CLI', str(tmp_path / 'missing-agent-roles'))

    code, out, err = _run_cli(['roles', 'install', 'agentroles.archi', '--skip-tools'], cwd=tmp_path)

    assert code == 1
    assert out == ''
    assert 'roles_status: failed' in err
    assert 'agent-roles could not run:' in err
    assert 'Traceback' not in err


def test_roles_install_manager_timeout_reports_failed_without_traceback(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('AGENT_ROLES_CLI', 'agent-roles')

    def fake_run(command, **_kwargs):
        raise subprocess.TimeoutExpired(cmd=command, timeout=3.0)

    monkeypatch.setattr(agent_roles_manager.subprocess, 'run', fake_run)

    code, out, err = _run_cli(['roles', 'install', 'agentroles.archi', '--skip-tools'], cwd=tmp_path)

    assert code == 1
    assert out == ''
    assert 'roles_status: failed' in err
    assert 'agent-roles timed out after 3s for install agentroles.archi' in err
    assert 'Traceback' not in err


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
    assert (_agent_roles_installed_root(tmp_path) / 'agentroles.archi' / 'install.json').is_file()
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
    _write_legacy_installed_role(tmp_path, role_dir_name='ccb.archi')
    legacy = tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'ccb.archi'
    canonical = _agent_roles_installed_root(tmp_path) / 'agentroles.archi'
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
    assert canonical_metadata['migrated_from'] == 'ccb'
    assert (canonical / 'current' / 'role.toml').is_file()
    assert load_installed_role('ccb.archi').id == 'agentroles.archi'


def test_roles_status_legacy_ccb_archi_migrates_on_status_query(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    _write_legacy_installed_role(tmp_path, role_dir_name='ccb.archi')
    legacy = tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'ccb.archi'
    canonical = _agent_roles_installed_root(tmp_path) / 'agentroles.archi'
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
    assert canonical_metadata['migrated_from'] == 'ccb'
    assert (canonical / 'current' / 'role.toml').is_file()


def test_roles_update_legacy_ccb_archi_missing_source_falls_back_to_catalog(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    _write_legacy_installed_role(tmp_path, role_dir_name='ccb.archi')
    canonical = _agent_roles_installed_root(tmp_path) / 'agentroles.archi'
    legacy = tmp_path / 'xdg-data' / 'ccb' / 'roles' / 'ccb.archi'
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
    metadata = json.loads((_agent_roles_installed_root(tmp_path) / 'test.fake' / 'install.json').read_text(encoding='utf-8'))
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


def test_archi_doctor_accepts_bundled_capabilities_from_main_cli(tmp_path: Path, monkeypatch) -> None:
    fake_bin = tmp_path / 'bin'
    fake_bin.mkdir()
    fake_archi = fake_bin / 'archi'
    fake_archi.write_text(
        '#!/usr/bin/env sh\n'
        'if [ "$1" = "--help" ]; then echo "archi --refresh-from-hippos --check --full"; exit 0; fi\n'
        'exit 0\n',
        encoding='utf-8',
    )
    fake_archi.chmod(0o755)
    monkeypatch.setenv('PATH', os.pathsep.join((str(fake_bin), os.environ.get('PATH', ''))))

    result = subprocess.run(
        [sys.executable, str(_agent_roles_archi() / 'adapters' / 'ccb' / 'tools' / 'doctor.py')],
        cwd=_agent_roles_archi(),
        env=dict(os.environ),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )

    assert result.returncode == 0
    assert result.stderr == ''
    assert 'architec_status: ok' in result.stdout
    assert 'package: @seemseam/architec' in result.stdout
    assert 'install_command: npm install -g @seemseam/architec' in result.stdout
    assert f'archi_binary: {fake_archi}' in result.stdout
    assert 'bundled_hippo: available' in result.stdout
    assert 'bundled_llmgateway: available' in result.stdout


def test_archi_tool_install_uses_global_npm_package(tmp_path: Path, monkeypatch) -> None:
    fake_bin = tmp_path / 'bin'
    fake_bin.mkdir()
    calls = tmp_path / 'npm-calls.txt'
    npm = fake_bin / 'npm'
    npm.write_text(
        '#!/usr/bin/env sh\n'
        'printf "%s\\n" "$@" > "$NPM_CALLS"\n'
        'echo npm-ok\n',
        encoding='utf-8',
    )
    npm.chmod(0o755)
    monkeypatch.setenv('PATH', os.pathsep.join((str(fake_bin), os.environ.get('PATH', ''))))
    monkeypatch.setenv('NPM_CALLS', str(calls))

    manifest = load_role_manifest(_agent_roles_archi())

    results = run_role_tool_hooks(manifest, action='install', fail_required=True)

    assert len(results) == 1
    assert results[0]['tool_id'] == 'architec'
    assert results[0]['status'] == 'ok'
    assert calls.read_text(encoding='utf-8').splitlines() == ['install', '-g', '@seemseam/architec']
    assert 'package: @seemseam/architec' in str(results[0]['stdout'])
    assert 'install_command:' in str(results[0]['stdout'])
    assert 'npm-ok' in str(results[0]['stdout'])


def test_archi_tool_doctor_checks_bundled_capabilities_without_install(tmp_path: Path, monkeypatch) -> None:
    fake_bin = tmp_path / 'bin'
    fake_bin.mkdir()
    binary = fake_bin / 'archi'
    binary.write_text(
        '#!/usr/bin/env sh\n'
        'if [ "$1" = "--help" ]; then echo "archi --refresh-from-hippos --check --full"; exit 0; fi\n'
        'exit 0\n',
        encoding='utf-8',
    )
    binary.chmod(0o755)
    monkeypatch.setenv('PATH', os.pathsep.join((str(fake_bin), os.environ.get('PATH', ''))))

    manifest = load_role_manifest(_agent_roles_archi())

    results = run_role_tool_hooks(manifest, action='doctor', fail_required=True)

    assert len(results) == 1
    assert results[0]['tool_id'] == 'architec'
    assert results[0]['status'] == 'ok'
    stdout = str(results[0]['stdout'])
    assert 'architec_status: ok' in stdout
    assert f'archi_binary: {fake_bin / "archi"}' in stdout
    assert 'bundled_hippo: available' in stdout
    assert 'bundled_llmgateway: available' in stdout
    assert 'install_command: npm install -g @seemseam/architec' in stdout


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
    metadata = json.loads((_agent_roles_installed_root(tmp_path) / 'agentroles.archi' / 'install.json').read_text(encoding='utf-8'))
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
    assert '@seemseam/architec' in role_memory
    assert 'Do not split Hippo or llmgateway into CCB-managed pip, venv, git, or editable installs' in role_memory
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
    locked_path = _agent_roles_installed_root(tmp_path) / 'test.locked' / 'versions' / '1.0.0' / locked_digest_hex
    assert locked_path.is_dir()

    _write_memory_catalog_role(catalog, default_agent_name='drifted', memory_text='drifted memory v2')
    update_role('test.locked', with_tools=False)
    current_path = (_agent_roles_installed_root(tmp_path) / 'test.locked' / 'current').resolve()

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


def test_legacy_migration_merges_locked_digest_into_existing_spec_store(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv('XDG_DATA_HOME', str(tmp_path / 'xdg-data'))
    catalog = tmp_path / 'agent-roles-spec'
    monkeypatch.setenv('AGENT_ROLES_SPEC_HOME', str(catalog))
    old_role = _write_memory_catalog_role(catalog, memory_text='legacy locked memory')
    legacy_payload = _write_legacy_installed_role(tmp_path, role_dir_name='test.locked', source=old_role)
    new_role = _write_memory_catalog_role(catalog, memory_text='new current memory')
    current_payload = _write_spec_installed_role(tmp_path, new_role)
    project = tmp_path / 'project'
    project.mkdir()
    _write_project_config_text(
        project,
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
    )
    (project / '.ccb' / 'role-lock.json').write_text(
        json.dumps(
            {
                'schema': 'rolepack-lock/v1',
                'roles': {
                    'test.locked': {
                        'version': '1.0.0',
                        'digest': legacy_payload['digest'],
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

    result = migrate_legacy_installed_roles('test.locked')
    locked_digest_hex = str(legacy_payload['digest']).removeprefix('sha256:')
    locked_path = _agent_roles_installed_root(tmp_path) / 'test.locked' / 'versions' / '1.0.0' / locked_digest_hex
    metadata = json.loads((_agent_roles_installed_root(tmp_path) / 'test.locked' / 'install.json').read_text(encoding='utf-8'))
    current = (_agent_roles_installed_root(tmp_path) / 'test.locked' / 'current').resolve()
    sources = load_memory_sources(project, agent_name='locked', provider='codex')
    warnings = [source.warning for source in sources if source.kind == 'role_memory' and source.warning]
    role_memory = '\n'.join(source.content for source in sources if source.kind == 'role_memory')

    assert result['migration_status'] == 'ok'
    assert result['migrated'] == 1
    assert locked_path.is_dir()
    assert metadata['digest'] == current_payload['digest']
    assert current == Path(str(current_payload['path'])).resolve()
    assert warnings == []
    assert 'legacy locked memory' in role_memory
    assert 'new current memory' not in role_memory


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
