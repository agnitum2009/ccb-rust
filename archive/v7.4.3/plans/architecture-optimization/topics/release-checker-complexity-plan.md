# Release Checker Complexity Plan

Date: 2026-05-18

## Purpose

Reduce the highest-ranked Architec hotspot:

```text
dev_tools/skills/ccb-github/scripts/check_release_state.py
```

The file is 1130 lines and owns local git checks, release-file checks, README
surface checks, GitHub API calls, workflow polling, asset verification, and CLI
output.

## Current Inventory

Primary file:

- `dev_tools/skills/ccb-github/scripts/check_release_state.py`

Existing tests:

- `test/test_ccb_github_skill.py`

Key high-complexity functions from the debt ledger:

- `check_github`
- `check_dev_branch_workflows`
- `check_local_files`
- `check_local_git_state`
- `check_readme_surface`
- `_check_branch_validation_runs`

## Target Shape

Keep the script entrypoint stable, but split internals by responsibility:

- command runner and Git helpers;
- release file and README validators;
- GitHub API client helpers;
- workflow polling and status formatting;
- release asset and checksum verification;
- report output.

The first pass can stay inside the same script if packaging constraints make a
multi-file split risky. The second pass should move stable groups into sibling
modules under `dev_tools/skills/ccb-github/scripts/` if imports are compatible
with skill projection.

The `ccb-github` skill is a directory, but active skill sync currently checks a
small tracked set that includes `scripts/check_release_state.py`. A multi-file
split must update the sync check and verify that managed Codex skill homes carry
any new helper modules.

Implemented Phase 2 target:

- `scripts/check_release_state.py` keeps CLI parsing, result printing, and the
  stable import surface used by tests.
- `scripts/release_checker_shared.py` owns constants, command execution, git
  output helpers, and issue/warning formatting.
- `scripts/release_checker_markdown.py` owns Markdown/release-note parsing.
- `scripts/release_checker_local.py` owns local git state, local release-file
  validation, development change classification, git tag checks, and active
  skill sync tracking.
- `scripts/release_checker_github.py` owns GitHub API reads, release asset and
  checksum checks, remote README checks, workflow polling, and published-release
  state validation.
- `TRACKED_SKILL_FILES` now includes all release checker helper modules.

Implemented follow-up target:

- `scripts/release_checker_github.py` now keeps GitHub orchestration, GitHub
  auth/default-branch helpers, default-branch containment, and remote homepage
  checks.
- `scripts/release_checker_workflows.py` owns workflow run reads, dev workflow
  polling, workflow wait status formatting, and branch validation run checks.
- `scripts/release_checker_assets.py` owns release payload reads,
  SHA256/download verification, release workflow candidate matching, and
  published release wait-state evaluation.
- `scripts/check_release_state.py` re-exports functions from the new modules so
  existing imports remain compatible.
- `TRACKED_SKILL_FILES` includes both new helper modules.

## Sequence

1. Extract pure helpers for release file validation and README release-surface
   checks.
2. Extract GitHub API read helpers so `check_github` becomes orchestration.
3. Extract workflow polling into a small state reader that returns structured
   status.
4. Keep CLI parsing and final printing in `main`.
5. Expand tests only around extracted helpers where existing behavior is not
   already covered.

## Constraints

- Preserve the read-only nature of the checker.
- Preserve exit code behavior.
- Preserve warning/failure message intent because users rely on actionable
  release instructions.
- Preserve active skill sync checks; this script is itself projected into
  managed Codex skill homes.
- Before a multi-file split, confirm that the skill projection path includes
  the new helper modules and that `check_active_skill_sync` tracks them.

## Verification

```bash
pytest test/test_ccb_github_skill.py
rg -n "check_release_state|ccb-github|scripts/" dev_tools/skills/ccb-github test/test_ccb_github_skill.py
python dev_tools/skills/ccb-github/scripts/check_release_state.py --phase prepare --wait-seconds 0
```

The second command may report legitimate local release-state issues; use it to
verify the script still runs and reports coherently, not necessarily that the
current workspace is release-ready.
