from __future__ import annotations

from datetime import datetime, timedelta, timezone
import os
from pathlib import Path

import pytest

from storage.paths import PathLayout
from storage.text_artifacts import (
    maybe_spill_text,
    read_text_artifact,
    sweep_expired_text_artifacts,
    validate_text_artifact_ref,
)

from ccbd.services.dispatcher_runtime.artifact_maintenance import sweep_text_artifacts_if_due


def test_maybe_spill_text_keeps_small_text_inline(tmp_path: Path) -> None:
    layout = PathLayout(tmp_path / 'repo-inline')

    body, artifact = maybe_spill_text(
        layout,
        text='short body',
        kind='ask-request',
        owner_id='agent1',
        prefix='large body',
    )

    assert body == 'short body'
    assert artifact is None
    assert not layout.ccbd_text_artifacts_dir.exists()


def test_maybe_spill_text_writes_large_text_artifact(tmp_path: Path) -> None:
    layout = PathLayout(tmp_path / 'repo-spill')
    text = 'x' * 5000

    body, artifact = maybe_spill_text(
        layout,
        text=text,
        kind='ask-request',
        owner_id='agent1',
        prefix='large body',
        now='2026-05-22T00:00:00Z',
    )

    assert artifact is not None
    assert len(body.encode('utf-8')) <= 4096
    assert 'large body' in body
    assert Path(str(artifact['path'])).read_text(encoding='utf-8') == text
    assert read_text_artifact(layout, artifact) == text


def test_validate_text_artifact_ref_rejects_path_escape(tmp_path: Path) -> None:
    layout = PathLayout(tmp_path / 'repo-escape')
    outside = tmp_path / 'outside.txt'
    outside.write_text('secret', encoding='utf-8')

    with pytest.raises(ValueError, match='escapes'):
        validate_text_artifact_ref(
            layout,
            {
                'path': str(outside),
                'bytes': 6,
                'sha256': 'bad',
            },
        )


def test_validate_text_artifact_ref_rejects_sha_mismatch(tmp_path: Path) -> None:
    layout = PathLayout(tmp_path / 'repo-sha')
    _, artifact = maybe_spill_text(
        layout,
        text='x' * 5000,
        kind='reply',
        owner_id='job1',
        prefix='large reply',
    )
    assert artifact is not None
    artifact['sha256'] = '0' * 64

    with pytest.raises(ValueError, match='sha256'):
        validate_text_artifact_ref(layout, artifact)


def test_sweep_expired_text_artifacts_removes_old_files(tmp_path: Path) -> None:
    layout = PathLayout(tmp_path / 'repo-sweep')
    _, artifact = maybe_spill_text(
        layout,
        text='x' * 5000,
        kind='reply',
        owner_id='job1',
        prefix='large reply',
    )
    assert artifact is not None
    path = Path(str(artifact['path']))
    old = (datetime.now(timezone.utc) - timedelta(days=2)).timestamp()
    os.utime(path, (old, old))

    removed = sweep_expired_text_artifacts(layout)

    assert path in removed
    assert not path.exists()


def test_sweep_text_artifacts_if_due_uses_cooldown(tmp_path: Path) -> None:
    layout = PathLayout(tmp_path / 'repo-sweep-cooldown')
    _, artifact = maybe_spill_text(
        layout,
        text='x' * 5000,
        kind='reply',
        owner_id='job1',
        prefix='large reply',
    )
    assert artifact is not None
    path = Path(str(artifact['path']))
    old = (datetime.now(timezone.utc) - timedelta(days=2)).timestamp()
    os.utime(path, (old, old))

    class _Dispatcher:
        _layout = layout
        _last_text_artifact_sweep_at = 10.0

    assert sweep_text_artifacts_if_due(_Dispatcher(), monotonic_fn=lambda: 60.0) == ()
    assert path.exists()
