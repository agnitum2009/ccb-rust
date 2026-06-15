from __future__ import annotations

from pathlib import Path

from .types import ProjectMemorySource

CCB_RUNTIME_COORDINATION_RULES = """## CCB Runtime Coordination Rules

- CCB `ask` is submit-only: submit once, then stop. Do not wait, poll, or run `pend`/`watch`/`ping` unless diagnostics were requested.
- Prefer `/ask <agent> <message>` when available. Shell fallback:

```bash
command ask "$TARGET" <<'EOF'
$MESSAGE
EOF
```

- During an active CCB ask task, use `ask --callback` when a child result is needed to finish; use `ask --silence` only for independent no-result-needed work.
"""


def render_memory_bundle(
    *,
    project_root: Path,
    agent_name: str,
    provider: str,
    sources: tuple[ProjectMemorySource, ...],
    workspace_path: Path | None = None,
) -> str:
    lines = [
        '# CCB Managed Agent Memory',
        '',
        '<!-- ccb-memory-bundle schema_version=1',
        'generated_by: ccb',
        'do_not_edit: true',
        f'agent: {agent_name}',
        f'provider: {provider}',
        f'project_root: {Path(project_root).expanduser().resolve()}',
    ]
    if workspace_path is not None:
        lines.append(f'workspace_path: {Path(workspace_path).expanduser().resolve()}')
    lines.extend(['-->', '', CCB_RUNTIME_COORDINATION_RULES.rstrip(), ''])

    for source in sources:
        if not source.exists and not source.warning:
            continue
        if not source.content.strip() and not source.warning:
            continue
        lines.extend(_render_source_section(source))

    return '\n'.join(lines).rstrip() + '\n'


def _render_source_section(source: ProjectMemorySource) -> list[str]:
    content = source.content.rstrip()
    lines = [
        f'## {source.title}',
        f'source: {source.path}',
    ]
    if source.warning:
        lines.append(f'warning: {source.warning}')
    lines.extend(['', content, ''])
    return lines


__all__ = ['render_memory_bundle']
