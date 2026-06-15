# Spec-Owned Roles Store First Slice

Date: 2026-06-04

## Scope

This checkpoint records the first executable bridge from CCB-owned Role Pack
payload management toward a spec-owned `agent-roles` package manager and
`.roles/installed` store.

## Implemented

- `agent-roles-spec` provides a preview Python CLI/package named
  `agent-roles`.
- The preview CLI owns `.roles/installed` role payload writes and stable JSON
  output for package operations.
- The preview alias table maps `ccb.archi` to `agentroles.archi`.
- CCB reads both legacy `$XDG_DATA_HOME/ccb/roles` and spec-owned
  `.roles/installed` stores for config loading, runtime projection, lock
  lookup, role status, and catalog status.
- CCB can delegate `roles install`, `roles update`, and `roles sync` payload
  operations to `agent-roles` when `CCB_AGENT_ROLES_MANAGER=1`.
- CCB wraps `agent-roles` missing executable, exec failure, timeout, nonzero
  JSON error, and non-JSON failure paths as Role Pack errors so the CLI emits
  `roles_status: failed` without traceback.

## Direct-Switch Delta

The initial opt-in position was superseded during the same release train. The
current direction is default-on delegation to `agent-roles`, `.roles/installed`
as the preferred store, automatic copy migration from the legacy CCB role store,
and `CCB_AGENT_ROLES_MANAGER=0` as a temporary rollback valve.

## Validation

- `agent-roles-spec`: `3 passed`
- CCB `test/test_rolepacks.py`: `48 passed`
- CCB targeted Role Pack/update/source guard/repo hygiene suite: `99 passed`
- CCB compileall for touched runtime modules: passed
- CCB and `agent-roles-spec` `git diff --check`: passed
- Real isolated `ccb_test` smoke proved:
  - `ccb roles install ccb.archi --skip-tools` can call `agent-roles` and write
    `.roles/installed/agentroles.archi`.
  - `ccb roles show ccb.archi` resolves the spec-owned store snapshot as
    canonical `agentroles.archi`.

## Release Position

This slice is no longer the final release position. `v7.2.11` was created from
the opt-in handoff before cancellation completed and must be superseded by the
direct-switch migration build after review and validation.

## Residual Risks

- A globally installed incompatible `agent-roles` command could return an
  unexpected JSON schema until version negotiation is added.
- Dual-store lookup must keep old project locks resolving old content-addressed
  snapshots through the copy migration window.
- Tool hook execution remains CCB-owned; the package manager writes role
  payloads but does not decide CCB required/optional tool policy.
