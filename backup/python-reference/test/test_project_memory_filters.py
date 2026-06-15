from __future__ import annotations

from pathlib import Path

from project_memory.filters import filter_memory_source
from project_memory.policy import FILTER_CCB_INSTALL_BLOCKS
from project_memory.types import ProjectMemorySource


def _source(text: str, *, kind: str = 'provider_user_memory') -> ProjectMemorySource:
    return ProjectMemorySource(
        kind=kind,
        title='Provider User Memory',
        path=Path('/tmp/AGENTS.md'),
        content=text,
        exists=True,
    )


def _filter(text: str, *, kind: str = 'provider_user_memory') -> ProjectMemorySource:
    return filter_memory_source(_source(text, kind=kind), filter_names=(FILTER_CCB_INSTALL_BLOCKS,))


def test_filter_strips_complete_ccb_config_block() -> None:
    result = _filter('before\n<!-- CCB_CONFIG_START -->\nconfig\n<!-- CCB_CONFIG_END -->\nafter\n')

    assert result.content == 'before\nafter\n'
    assert result.filtered is True
    assert result.filter_names == (FILTER_CCB_INSTALL_BLOCKS,)


def test_filter_strips_complete_roles_and_rubrics_blocks() -> None:
    result = _filter(
        'keep\n'
        '<!-- CCB_ROLES_START -->roles<!-- CCB_ROLES_END -->\n'
        '<!-- REVIEW_RUBRICS_START -->rubric<!-- REVIEW_RUBRICS_END -->\n'
        'tail\n'
    )

    assert result.content == 'keep\ntail\n'


def test_filter_strips_review_and_gemini_inspiration_blocks() -> None:
    result = _filter(
        'keep\n'
        '<!-- CODEX_REVIEW_START -->review<!-- CODEX_REVIEW_END -->\n'
        '<!-- GEMINI_INSPIRATION_START -->idea<!-- GEMINI_INSPIRATION_END -->\n'
        'tail\n'
    )

    assert result.content == 'keep\ntail\n'


def test_filter_strips_legacy_collaboration_sections() -> None:
    result = _filter(
        'intro\n'
        '## Codex Collaboration Rules\nold codex\n'
        '## Gemini Collaboration Rules\nold gemini\n'
        '## OpenCode Collaboration Rules\nold opencode\n'
    )

    assert 'Collaboration Rules' not in result.content
    assert result.content == 'intro\n'


def test_filter_strips_legacy_chinese_collaboration_sections() -> None:
    result = _filter(
        'intro\n'
        '## Codex 协作规则\nold codex\n'
        '## Gemini 协作规则\nold gemini\n'
        '## OpenCode 协作规则\nold opencode\n'
    )

    assert '协作规则' not in result.content
    assert result.content == 'intro\n'


def test_filter_preserves_user_paragraph_spacing_after_block_removal() -> None:
    result = _filter(
        'first paragraph\n'
        '\n'
        'second paragraph\n'
        '<!-- CCB_CONFIG_START -->\n'
        'old config\n'
        '<!-- CCB_CONFIG_END -->\n'
        'third paragraph\n'
    )

    assert result.content == 'first paragraph\n\nsecond paragraph\nthird paragraph\n'


def test_filter_preserves_isolated_marker() -> None:
    text = 'before\n<!-- CCB_CONFIG_START -->\nuser note without end marker\n'
    result = _filter(text)

    assert result.content == text
    assert result.filtered is False


def test_filter_preserves_unrelated_user_text() -> None:
    text = 'Use ask carefully, but this is user-authored and has no CCB marker pair.\n'
    result = _filter(text)

    assert result.content == text
    assert result.filtered is False


def test_filter_only_applies_to_provider_user_memory() -> None:
    text = 'before\n<!-- CCB_CONFIG_START -->\nconfig\n<!-- CCB_CONFIG_END -->\nafter\n'
    result = _filter(text, kind='ccb_shared')

    assert result.content == text
    assert result.filtered is False
