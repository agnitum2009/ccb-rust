from __future__ import annotations

import json
from pathlib import Path

from provider_hooks.activity import load_activity, read_activity_evidence, write_activity


def test_write_activity_persists_agent_scoped_snapshot(tmp_path: Path) -> None:
    runtime_dir = tmp_path / 'agents' / 'agent2' / 'provider-runtime' / 'codex'

    path = write_activity(
        provider='codex',
        project_id='project-1',
        agent_name='agent2',
        runtime_dir=runtime_dir,
        state='tool',
        source='codex_hook',
        event_name='PreToolUse',
        ccb_session_id='ccb-agent2-1',
        pane_id='%42',
        workspace_path=tmp_path / 'workspace',
        diagnostics={'tool_name': 'shell', 'api_key': 'must-not-leak'},
        updated_at='2026-05-27T00:00:00Z',
    )

    assert path == runtime_dir / 'activity.json'
    payload = json.loads(path.read_text(encoding='utf-8'))
    assert payload['record_type'] == 'provider_activity'
    assert payload['state'] == 'active'
    assert payload['provider'] == 'codex'
    assert payload['agent_name'] == 'agent2'
    assert payload['diagnostics'] == {'tool_name': 'shell'}


def test_read_activity_evidence_accepts_matching_identity(tmp_path: Path) -> None:
    runtime_dir = tmp_path / 'runtime'
    workspace = tmp_path / 'workspace'
    write_activity(
        provider='claude',
        project_id='project-1',
        agent_name='agent3',
        runtime_dir=runtime_dir,
        state='waiting',
        source='claude_hook',
        event_name='Notification',
        ccb_session_id='ccb-agent3-1',
        pane_id='%5',
        workspace_path=workspace,
        updated_at='2026-05-27T00:00:00Z',
    )

    evidence = read_activity_evidence(
        runtime_dir,
        project_id='project-1',
        agent_name='agent3',
        provider='claude',
        ccb_session_id='ccb-agent3-1',
        pane_id='%5',
        workspace_path=workspace,
        now='2026-05-27T00:00:05Z',
    )

    assert evidence is not None
    assert evidence.state == 'pending'
    assert evidence.source == 'claude_hook'
    assert evidence.reason == 'provider_Notification'


def test_read_activity_evidence_treats_provider_session_as_diagnostic_identity(tmp_path: Path) -> None:
    runtime_dir = tmp_path / 'runtime'
    write_activity(
        provider='codex',
        project_id='project-1',
        agent_name='agent2',
        runtime_dir=runtime_dir,
        state='active',
        source='codex_hook',
        event_name='UserPromptSubmit',
        ccb_session_id='ccb-agent2-launch',
        provider_session_id='codex-session-1',
        pane_id='%1',
        updated_at='2026-05-27T00:00:00Z',
    )

    evidence = read_activity_evidence(
        runtime_dir,
        project_id='project-1',
        agent_name='agent2',
        provider='codex',
        provider_session_id='codex-session-1',
        pane_id='%1',
        now='2026-05-27T00:00:01Z',
    )

    assert evidence is not None
    assert evidence.state == 'active'

    mismatched_runtime_session = read_activity_evidence(
        runtime_dir,
        project_id='project-1',
        agent_name='agent2',
        provider='codex',
        provider_session_id='codex-session-2',
        pane_id='%1',
        now='2026-05-27T00:00:01Z',
    )

    assert mismatched_runtime_session is not None
    assert mismatched_runtime_session.provider_session_id == 'codex-session-1'


def test_read_activity_evidence_rejects_wrong_identity(tmp_path: Path) -> None:
    runtime_dir = tmp_path / 'runtime'
    write_activity(
        provider='codex',
        project_id='project-1',
        agent_name='agent2',
        runtime_dir=runtime_dir,
        state='active',
        source='codex_hook',
        ccb_session_id='ccb-agent2-old',
        pane_id='%1',
        updated_at='2026-05-27T00:00:00Z',
    )

    assert (
        read_activity_evidence(
            runtime_dir,
            project_id='project-1',
            agent_name='agent2',
            provider='codex',
            ccb_session_id='ccb-agent2-new',
            pane_id='%1',
            now='2026-05-27T00:00:01Z',
        )
        is None
    )
    assert (
        read_activity_evidence(
            runtime_dir,
            project_id='project-1',
            agent_name='agent2',
            provider='claude',
            ccb_session_id='ccb-agent2-old',
            pane_id='%1',
            now='2026-05-27T00:00:01Z',
        )
        is None
    )


def test_read_activity_evidence_rejects_malformed_and_future_timestamp(tmp_path: Path) -> None:
    runtime_dir = tmp_path / 'runtime'
    activity_file = runtime_dir / 'activity.json'
    activity_file.parent.mkdir(parents=True)
    activity_file.write_text('{not-json', encoding='utf-8')

    assert (
        read_activity_evidence(
            runtime_dir,
            project_id='project-1',
            agent_name='agent2',
            provider='codex',
        )
        is None
    )

    write_activity(
        provider='codex',
        project_id='project-1',
        agent_name='agent2',
        runtime_dir=runtime_dir,
        state='active',
        source='codex_hook',
        updated_at='2026-05-27T00:10:00Z',
    )

    assert (
        read_activity_evidence(
            runtime_dir,
            project_id='project-1',
            agent_name='agent2',
            provider='codex',
            now='2026-05-27T00:00:00Z',
            max_future_skew_s=30,
        )
        is None
    )


def test_load_activity_returns_none_for_missing_or_invalid_payload(tmp_path: Path) -> None:
    runtime_dir = tmp_path / 'runtime'
    assert load_activity(runtime_dir) is None
    (runtime_dir / 'activity.json').parent.mkdir(parents=True)
    (runtime_dir / 'activity.json').write_text('[]', encoding='utf-8')
    assert load_activity(runtime_dir) is None


def test_failed_activity_is_sticky_until_next_active_turn_or_identity_change(tmp_path: Path) -> None:
    runtime_dir = tmp_path / 'runtime'
    write_activity(
        provider='codex',
        project_id='project-1',
        agent_name='agent2',
        runtime_dir=runtime_dir,
        state='failed',
        source='codex_hook',
        ccb_session_id='ccb-agent2-1',
        pane_id='%1',
        updated_at='2026-05-27T00:00:00Z',
    )

    write_activity(
        provider='codex',
        project_id='project-1',
        agent_name='agent2',
        runtime_dir=runtime_dir,
        state='idle',
        source='codex_hook',
        ccb_session_id='ccb-agent2-1',
        pane_id='%1',
        updated_at='2026-05-27T00:00:01Z',
    )

    assert load_activity(runtime_dir)['state'] == 'failed'

    write_activity(
        provider='codex',
        project_id='project-1',
        agent_name='agent2',
        runtime_dir=runtime_dir,
        state='active',
        source='codex_hook',
        ccb_session_id='ccb-agent2-1',
        pane_id='%1',
        updated_at='2026-05-27T00:00:02Z',
    )

    assert load_activity(runtime_dir)['state'] == 'active'

    write_activity(
        provider='codex',
        project_id='project-1',
        agent_name='agent2',
        runtime_dir=runtime_dir,
        state='failed',
        source='codex_hook',
        ccb_session_id='ccb-agent2-1',
        pane_id='%1',
        updated_at='2026-05-27T00:00:03Z',
    )
    write_activity(
        provider='codex',
        project_id='project-1',
        agent_name='agent2',
        runtime_dir=runtime_dir,
        state='idle',
        source='codex_hook',
        ccb_session_id='ccb-agent2-2',
        pane_id='%2',
        updated_at='2026-05-27T00:00:04Z',
    )

    assert load_activity(runtime_dir)['state'] == 'idle'
    assert load_activity(runtime_dir)['ccb_session_id'] == 'ccb-agent2-2'
