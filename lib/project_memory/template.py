from __future__ import annotations

TEMPLATE_VERSION = 4

DEFAULT_PROJECT_MEMORY = """# CCB Project Memory

This project uses CCB for visible multi-agent collaboration.

## Collaboration

- You are one agent in a CCB-managed project team.
- Use CCB `ask` for project-level collaboration with configured agents.
- Delegate with the goal, scope/files, assumptions, expected output, and verification needs.
- Reply concisely with findings, changes, verification, blockers, and risks when relevant.

## Ask Communication

Preferred form:

```text
/ask <agent> <message>
```

Shell fallback:

```bash
command ask "$TARGET" <<'EOF'
$MESSAGE
EOF
```

- Submit once, then stop. Do not wait, poll, or run `pend`/`watch`/`ping` unless diagnostics were requested.
- During an active CCB ask task, use `ask --callback` when a child result is needed to finish; use `ask --silence` only for independent no-result-needed work.
- Plain nested `ask` from an active task is rejected by CCB.
"""

__all__ = ['DEFAULT_PROJECT_MEMORY', 'TEMPLATE_VERSION']
