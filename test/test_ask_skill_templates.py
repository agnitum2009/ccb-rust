from __future__ import annotations

from pathlib import Path
import re


REPO_ROOT = Path(__file__).resolve().parents[1]


def test_shell_ask_skill_templates_use_short_ask_command() -> None:
    for relative_path in (
        'inherit_skills/claude_skills/ask/SKILL.md',
        'inherit_skills/claude_skills/ask/RUNTIME.md',
        'inherit_skills/codex_skills/ask/SKILL.md',
        'inherit_skills/droid_skills/ask/SKILL.md',
        'inherit_skills/droid_skills/ask.md',
    ):
        text = (REPO_ROOT / relative_path).read_text(encoding='utf-8')
        assert 'command ask ' in text
        assert 'command ccb ask' not in text
        assert 'canonical `ccb ask`' not in text
        assert 'compatibility alias' not in text
        assert 'forwarded verbatim' not in text
        assert re.search(r'[\u4e00-\u9fff]', text) is None


def test_powershell_ask_skill_template_uses_short_ask_command() -> None:
    text = (REPO_ROOT / 'inherit_skills/claude_skills/ask/SKILL.md.powershell').read_text(encoding='utf-8')

    assert 'FilePath "ask"' in text
    assert 'ccb ask' not in text
    assert 'compatibility alias' not in text
    assert 'forwarded verbatim' not in text
    assert re.search(r'[\u4e00-\u9fff]', text) is None
