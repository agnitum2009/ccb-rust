from __future__ import annotations

import json
import os
from pathlib import Path

import pytest


pytestmark = pytest.mark.skipif(
    os.environ.get('CCB_PROVIDER_MEMORY_MATRIX_CHECK') != '1',
    reason='set CCB_PROVIDER_MEMORY_MATRIX_CHECK=1 to inspect the external provider memory matrix project',
)


DEFAULT_MATRIX_PROJECT = Path('/home/bfly/yunwei/test_ccb_provider_memory_matrix')


def test_external_provider_memory_matrix_has_expected_layered_context() -> None:
    project_root = Path(os.environ.get('CCB_PROVIDER_MEMORY_MATRIX_PROJECT') or DEFAULT_MATRIX_PROJECT).resolve()
    if not project_root.exists():
        pytest.skip(f'external provider memory matrix project does not exist: {project_root}')

    codex = _read(project_root / '.ccb' / 'agents' / 'codexer' / 'provider-state' / 'codex' / 'home' / 'AGENTS.md')
    claude = _read(
        project_root
        / '.ccb'
        / 'agents'
        / 'clauder'
        / 'provider-state'
        / 'claude'
        / 'home'
        / '.claude'
        / 'CLAUDE.md'
    )
    opencode = _read(project_root / '.ccb' / 'runtime' / 'memory' / 'opencoder.md')
    gemini = _read(
        project_root
        / '.ccb'
        / 'agents'
        / 'geminier'
        / 'provider-state'
        / 'gemini'
        / 'home'
        / '.gemini'
        / 'GEMINI.md'
    )

    for text in (codex, claude, opencode, gemini):
        assert text.count('## CCB Runtime Coordination Rules') == 1
        assert text.count('command ask "$TARGET"') == 1
        assert 'MATRIX-SHARED-MEMORY-SENTINEL' in text
        assert 'Ask Communication' not in text

    assert '## Provider User Memory' in codex
    assert 'MATRIX-USER-CODEX-MEMORY-SENTINEL' in codex
    assert 'MATRIX-OLD-CODEX-ROLES-SHOULD-BE-FILTERED' not in codex
    assert 'MATRIX-AGENT-CODEXER-PRIVATE-SENTINEL' in codex
    assert 'MATRIX-PROJECT-AGENTS-SENTINEL' not in codex
    assert '## Provider-Native Project Memory' not in codex

    assert '## Provider User Memory' in claude
    assert 'MATRIX-USER-CLAUDE-MEMORY-SENTINEL' in claude
    assert 'MATRIX-OLD-CLAUDE-CONFIG-SHOULD-BE-FILTERED' not in claude
    assert 'MATRIX-AGENT-CLAUDER-PRIVATE-SENTINEL' in claude
    assert 'MATRIX-PROJECT-CLAUDE-SENTINEL' not in claude
    assert '## Provider-Native Project Memory' not in claude

    assert '## Provider User Memory' not in opencode
    assert 'MATRIX-AGENT-OPENCODER-PRIVATE-SENTINEL' in opencode
    assert 'MATRIX-PROJECT-AGENTS-SENTINEL' not in opencode
    assert '## Provider-Native Project Memory' not in opencode
    opencode_config = json.loads(
        (project_root / '.ccb' / 'agents' / 'opencoder' / 'provider-state' / 'opencode' / 'opencode.json').read_text(
            encoding='utf-8'
        )
    )
    assert opencode_config['instructions'] == ['AGENTS.md', '.ccb/runtime/memory/opencoder.md']

    assert '## Provider User Memory' in gemini
    assert 'MATRIX-USER-GEMINI-MEMORY-SENTINEL' in gemini
    assert 'MATRIX-OLD-GEMINI-INSPIRATION-SHOULD-BE-FILTERED' not in gemini
    assert 'MATRIX-AGENT-GEMINIER-PRIVATE-SENTINEL' in gemini
    assert '## Provider-Native Project Memory' in gemini
    assert 'MATRIX-PROJECT-GEMINI-SENTINEL' in gemini
    gemini_settings = json.loads(
        (
            project_root
            / '.ccb'
            / 'agents'
            / 'geminier'
            / 'provider-state'
            / 'gemini'
            / 'home'
            / '.gemini'
            / 'settings.json'
        ).read_text(encoding='utf-8')
    )
    assert gemini_settings['contextFileName'] == 'GEMINI.md'


def _read(path: Path) -> str:
    try:
        return path.read_text(encoding='utf-8')
    except OSError as exc:
        pytest.fail(f'missing generated memory file {path}: {exc}')
