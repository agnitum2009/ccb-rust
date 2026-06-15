from __future__ import annotations

import json
from pathlib import Path

from project_memory.hashing import sha256_text
from provider_core.memory_projection import (
    memory_projection_result,
    record_memory_projection_event,
    same_memory_projection_signature,
    text_file_sha256,
    write_projection_event_and_marker,
)


def test_memory_projection_result_normalizes_warning_and_error_fields(tmp_path: Path) -> None:
    result = memory_projection_result(
        status='failed',
        reason='missing_project_context',
        path=tmp_path / 'AGENTS.md',
        warnings=['warn', '', 'also-warn'],
        error_detail=None,
    )

    assert result['status'] == 'failed'
    assert result['reason'] == 'missing_project_context'
    assert result['path'] == str(tmp_path / 'AGENTS.md')
    assert result['warnings'] == ('warn', 'also-warn')
    assert result['error_detail'] == ''


def test_record_memory_projection_event_uses_caller_provider_and_dedupes(tmp_path: Path) -> None:
    event_path = tmp_path / 'events.jsonl'
    marker_path = tmp_path / 'projection-marker.json'
    result = memory_projection_result(
        status='ok',
        reason='written',
        path=tmp_path / 'CLAUDE.md',
        sha256='abc123',
        source_count=2,
        warnings=('careful',),
    )

    record_memory_projection_event(
        result,
        provider='claude',
        event_path=event_path,
        marker_path=marker_path,
        agent_name='agent1',
    )
    record_memory_projection_event(
        result,
        provider='claude',
        event_path=event_path,
        marker_path=marker_path,
        agent_name='agent1',
    )

    events = [json.loads(line) for line in event_path.read_text(encoding='utf-8').splitlines()]
    assert len(events) == 1
    assert events[0]['event_type'] == 'claude_memory_projection_ok'
    assert events[0]['provider'] == 'claude'
    assert events[0]['agent_name'] == 'agent1'
    assert events[0]['projection_path'] == str(tmp_path / 'CLAUDE.md')
    assert events[0]['sha256'] == 'abc123'
    assert events[0]['source_count'] == 2
    assert events[0]['warnings'] == ['careful']

    marker = json.loads(marker_path.read_text(encoding='utf-8'))
    assert marker == {
        'status': 'ok',
        'reason': 'written',
        'path': str(tmp_path / 'CLAUDE.md'),
        'sha256': 'abc123',
        'warnings': ['careful'],
    }


def test_record_memory_projection_event_requires_provider_and_targets(tmp_path: Path) -> None:
    result = memory_projection_result(status='ok', reason='written', path=tmp_path / 'GEMINI.md')

    record_memory_projection_event(
        result,
        provider='',
        event_path=tmp_path / 'events.jsonl',
        marker_path=tmp_path / 'marker.json',
        agent_name='agent1',
    )
    record_memory_projection_event(
        result,
        provider='gemini',
        event_path=None,
        marker_path=tmp_path / 'marker.json',
        agent_name='agent1',
    )
    record_memory_projection_event(
        result,
        provider='gemini',
        event_path=tmp_path / 'events.jsonl',
        marker_path=None,
        agent_name='agent1',
    )
    record_memory_projection_event(
        result,
        provider='gemini',
        event_path=tmp_path / 'events.jsonl',
        marker_path=tmp_path / 'marker.json',
        agent_name=None,
    )

    assert not (tmp_path / 'events.jsonl').exists()
    assert not (tmp_path / 'marker.json').exists()


def test_same_memory_projection_signature_requires_sha_for_unchanged_fast_path(tmp_path: Path) -> None:
    marker = tmp_path / 'marker.json'
    marker.write_text(
        json.dumps(
            {
                'status': 'ok',
                'reason': 'written',
                'path': str(tmp_path / 'AGENTS.md'),
                'sha256': '',
                'warnings': [],
            },
            ensure_ascii=False,
            indent=2,
        )
        + '\n',
        encoding='utf-8',
    )

    assert not same_memory_projection_signature(
        marker,
        {
            'status': 'skipped',
            'reason': 'unchanged',
            'path': str(tmp_path / 'AGENTS.md'),
            'sha256': '',
            'warnings': [],
        },
    )


def test_same_memory_projection_signature_allows_extra_fields_exact_match(tmp_path: Path) -> None:
    marker = tmp_path / 'opencode-marker.json'
    signature = {
        'status': 'ok',
        'reason': 'written',
        'path': str(tmp_path / 'bundle.md'),
        'config_path': str(tmp_path / 'opencode.json'),
        'bundle_path': str(tmp_path / 'bundle.md'),
        'sha256': 'bundle-sha',
        'config_sha256': 'config-sha',
        'warnings': [],
        'config_merge_status': 'merged',
        'config_merge_reason': 'merged_project_opencode_json',
    }
    marker.write_text(json.dumps(signature, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

    assert same_memory_projection_signature(marker, signature)
    assert not same_memory_projection_signature(marker, {**signature, 'config_sha256': 'other'})


def test_same_memory_projection_signature_skipped_fast_path_uses_base_fields(tmp_path: Path) -> None:
    marker = tmp_path / 'opencode-marker.json'
    marker.write_text(
        json.dumps(
            {
                'status': 'ok',
                'reason': 'written',
                'path': str(tmp_path / 'bundle.md'),
                'config_path': str(tmp_path / 'opencode.json'),
                'sha256': 'bundle-sha',
                'config_sha256': 'old-config-sha',
                'warnings': [],
            },
            ensure_ascii=False,
            indent=2,
        )
        + '\n',
        encoding='utf-8',
    )

    assert same_memory_projection_signature(
        marker,
        {
            'status': 'skipped',
            'reason': 'unchanged',
            'path': str(tmp_path / 'bundle.md'),
            'config_path': str(tmp_path / 'opencode.json'),
            'sha256': 'bundle-sha',
            'config_sha256': 'new-config-sha',
            'warnings': [],
        },
    )


def test_write_projection_event_and_marker_appends_event_and_writes_signature(tmp_path: Path) -> None:
    event_path = tmp_path / 'events.jsonl'
    marker_path = tmp_path / 'projection-marker.json'
    event = {
        'record_type': 'agent_event',
        'event_type': 'opencode_memory_projection_ok',
        'provider': 'opencode',
    }
    signature = {
        'status': 'ok',
        'reason': 'written',
        'path': str(tmp_path / 'memory.md'),
        'sha256': 'abc123',
        'warnings': [],
    }

    write_projection_event_and_marker(event, signature, event_path=event_path, marker_path=marker_path)

    assert [json.loads(line) for line in event_path.read_text(encoding='utf-8').splitlines()] == [event]
    assert json.loads(marker_path.read_text(encoding='utf-8')) == signature


def test_text_file_sha256_hashes_existing_file(tmp_path: Path) -> None:
    path = tmp_path / 'memory.md'
    path.write_text('memory bundle\n', encoding='utf-8')

    assert text_file_sha256(path) == sha256_text('memory bundle\n')


def test_text_file_sha256_returns_empty_for_missing_file(tmp_path: Path) -> None:
    assert text_file_sha256(tmp_path / 'missing.md') == ''
