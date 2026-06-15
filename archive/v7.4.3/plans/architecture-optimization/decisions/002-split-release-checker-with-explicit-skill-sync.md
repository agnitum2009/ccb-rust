# Split Release Checker With Explicit Skill Sync

Date: 2026-05-18

## Context

`dev_tools/skills/ccb-github/scripts/check_release_state.py` was the highest
single-file Architec hotspot. It also lives inside a Codex skill directory, so a
multi-file split must preserve the script entrypoint and ensure managed skill
homes receive every helper module.

## Decision

Split the checker into sibling modules under
`dev_tools/skills/ccb-github/scripts/` while keeping
`check_release_state.py` as the executable entrypoint and compatibility import
surface.

Track the new helper modules in `TRACKED_SKILL_FILES` so
`check_active_skill_sync` warns when active managed `ccb-github` skill copies
are missing or stale.

## Consequences

The release checker no longer concentrates local release checks, GitHub API
reads, workflow polling, Markdown parsing, and output orchestration in one file.
The skill projection remains directory-based, and the active sync check now has
explicit coverage for the helper modules introduced by the split.
