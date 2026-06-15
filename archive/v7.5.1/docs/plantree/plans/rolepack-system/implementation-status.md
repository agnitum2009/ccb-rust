# Role Pack System Implementation Status

Date: 2026-06-04

## Current Phase

Spec-owned `.roles` package-manager bridge, single-store simplification slice.

## Done This Phase

- `agent-roles-spec` now has a preview `agent-roles` package manager with JSON
  `list`, `install`, `update`, `sync`, `doctor`, and `resolve` commands.
- `agent-roles-spec` records the legacy alias
  `ccb.archi -> agentroles.archi`.
- CCB runtime/config lookup reads only the spec-owned `AGENT_ROLES_STORE` /
  `~/.roles/installed` installed store.
- CCB role package operations delegate to `agent-roles`; the old CCB-owned
  installed-role writer and `CCB_AGENT_ROLES_MANAGER` rollback switch have been
  removed.
- CCB management commands migrate installed legacy role snapshots from
  `$XDG_DATA_HOME/ccb/roles` into `.roles/installed` without deleting the old
  store. The legacy store is migration input only, not a runtime fallback.
- The `agent-roles` subprocess bridge now reports missing CLI, exec failures,
  timeouts, nonzero JSON errors, and non-JSON failures through the normal
  `roles_status: failed` CLI channel instead of leaking tracebacks.
- `ccb roles sync --with-tools` now composes manager-owned package sync with
  CCB-owned tool-hook execution, and malformed sync `roles` payloads fail closed.
- `ccb-config` skill docs say configs must keep canonical role ids and must not
  write local store paths such as `~/.roles` into `.ccb/ccb.config`.

## Active TODO

1. Verify the default-on release in a real old-version upgrade directory with a
   legacy `$XDG_DATA_HOME/ccb/roles` install and a project role lock.
2. Remove or dev-gate the source-checkout fallback for `~/yunwei/agent-roles-spec`
   before the bridge graduates from preview.
3. Add review coverage for import boundaries so provider startup cannot import
   package-manager subprocess or network-capable code.
4. Decide whether CCB should install/refresh `agent-roles` during `ccb update`
   or only consume an already-installed tool.
5. Move the migration command from the CCB bridge into `agent-roles` once the
   spec package manager exposes it.

## Blockers

- Release is blocked until the single-store simplification receives archi/agent3
  review and real old-version upgrade validation.
- `v7.2.11` was created from the earlier opt-in handoff before cancellation
  completed; `VERSION`, `ccb`, README, README_zh, and CHANGELOG now target
  `v7.2.12` and mark `v7.2.11` as superseded.

## Next Commit Target

Commit only the Role Pack / Agent Roles bridge files and plan-tree updates.
Do not include unrelated ask/runtime dirty files currently present in the
worktree.

## Last Verified Commands

- In `agent-roles-spec`: `python -m pytest -q`
- In `agent-roles-spec`: `python -m compileall -q agent_roles`
- In `agent-roles-spec`: `git diff --check`
- In `ccb_source`: `python -m pytest -q test/test_rolepacks.py` (`53 passed`)
- In `ccb_source`: `python -m pytest -q test/test_rolepacks.py test/test_cli_management_update.py test/test_repo_hygiene.py test/test_source_runtime_guard.py` (`104 passed`)
- In `ccb_source`: `python -m compileall -q lib/agents/config_loader_runtime/role_lookup.py lib/rolepacks/runtime_lookup.py lib/rolepacks/sources.py lib/rolepacks/service.py lib/rolepacks/agent_roles_manager.py`
- In `ccb_source`: `git diff --check`
- In `ccb_source`: `python ccb --print-version` (`v7.2.12`)
- In `/home/bfly/yunwei/test_ccb2`: isolated default-switch smoke used a legacy
  `$XDG_DATA_HOME/ccb/roles` install, then manager mode with
  `AGENT_ROLES_CLI=/home/bfly/yunwei/agent-roles-spec/cli/agent-roles` ran
  `ccb_test roles update agentroles.archi --skip-tools` and
  `ccb_test roles show ccb.archi`. Current resolved from `.roles/installed`.
