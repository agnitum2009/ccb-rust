# Test And Governance

Date: 2026-06-01

## Objective

Make Role Packs reviewable, testable, and safe enough for community PRs.
Testing must cover both the host-neutral schema and the CCB adapter.

## Static Tests

- Manifest schema validation.
- Role id normalization and collision checks.
- Required field validation.
- Provider skill path validation.
- Forbidden file scan for sessions, auth, private keys, and runtime authority.
- Link checks for README, topics, and examples.
- Permission declaration coverage for tools and network access.

## Projection Tests

- Project config with `[agents.<name>] role = "<role-id>"` loads correctly.
- Project config shorthand such as `agentroles.archi:codex` requires the
  installed system role store and resolves to the role manifest's default
  agent name.
- Shorthand expansion runs before defaults, overlay merge, and final topology
  validation, so rendered config records match the explicit binding path.
- Missing shorthand roles fail config loading with an install command hint.
- Unreadable role stores and invalid role manifests fail with diagnostics that
  do not imply a simple install will fix the store.
- Explicit role bindings and shorthand-derived bindings share role lookup,
  provider compatibility, and conflict validation.
- Unsupported provider fails with a clear config or doctor message.
- Codex skills project only into the role-bound Codex agent.
- Claude skills project only into the role-bound Claude agent.
- Role memory appears in generated provider memory after project memory and
  before agent private memory.
- Removing a role removes only role-owned projected assets.
- Symlink fallback to copy writes projection markers.
- Sidebar, mailbox, job, and pane records display the project-local agent name
  such as `archi`, not the role id `agentroles.archi`.
- `ccb ask agentroles.archi ...` resolves only when exactly one configured
  agent is bound to that role id; no-match and multi-match cases fail with
  explicit guidance.
- Multi-match ask alias errors list the matching project-local agent names.

## Lifecycle Tests

- `roles install` writes the system store and install metadata.
- Role tool hooks do not create Python bytecode caches inside role sources or
  installed snapshots.
- Reinstall/update repairs a polluted content-addressed target when the target
  path digest no longer matches the target tree.
- `roles install` resolves from `agent-roles-spec` by default and does not
  require production role content in the CCB source tree.
- `roles list` discovers user-level system roles from `~/.ccb/roles` and
  `~/.roles`, and these local sources take precedence over duplicate remote
  catalog role ids with a visible duplicate warning.
- `roles add <role-id>:<provider>` can snapshot an uninstalled user-level
  system role into the installed store before binding it to the current
  project.
- `roles sync` defaults to the current working directory, updates already
  installed same-id roles discovered under that directory, skips uninstalled
  roles, and does not scan unrelated system sources.
- When no local `agent-roles-spec` path exists, catalog discovery clones the
  GitHub catalog into the CCB-owned user cache without requiring role content
  in the CCB source tree.
- `ccb update` refreshes the CCB-owned GitHub catalog cache with
  `git pull --ff-only`; ordinary catalog reads should not force a pull.
- Remote catalog fallback can be disabled for offline or policy-controlled
  environments.
- `roles add` updates project config and lock without copying role assets into
  the project.
- `roles doctor` reports installed, missing, degraded, stale, and projected
  states.
- `roles update` refreshes the system store from `agent-roles-spec` without
  floating project locks.
- `ccb update` refreshes installed roles that have newer catalog versions.
- `ccb update` reports newly available catalog roles and prompts before
  installing them.
- non-interactive `ccb update` reports newly available roles without installing
  them implicitly.
- CCB source-tree `roles/` content is not used as production role catalog
  authority.
- `roles refresh` updates projections and reports digest changes.
- `ccb reload` does not turn role memory, skill, prompt, or tool-state changes
  into topology replacement when the configured agent set and provider leaves
  did not change.

## Resilience Tests

- Import smoke tests prove provider hooks, config loading, provider-home
  materialization, and project memory rendering do not import role management
  services or network-capable source discovery by accident.
- Concurrent operations on the same role are covered, including two installs,
  install plus add, and sync plus project reload.
- Tool hook failure during install, update, and sync verifies either rollback
  behavior or explicit degraded install metadata.
- Project config mutation plus role-lock mutation failure is covered so partial
  writes are diagnosed and recoverable.
- Locked digest content deletion is covered separately from mutable `current`
  movement while locked content still exists.
- Same-version digest changes are covered for both catalog roles and
  user-level system role sources.
- GitHub catalog clone/pull timeout, permission failure, and remote-disabled
  behavior are covered with visible diagnostics.
- Duplicate role-id source diagnostics are visible in CLI output, not only in
  internal row data.

## Real Project Tests

Use a disposable project such as `/home/bfly/yunwei/test_ccb2`:

1. Start an existing CCB project with two normal agents.
2. Install `agentroles.archi`.
3. Add `archi` to the config and run `ccb reload`.
4. Verify existing agents remain alive and keep their provider sessions.
5. Verify the new `archi` pane starts and appears in sidebar.
6. Verify the sidebar row is named `archi`, not `agentroles.archi`.
7. Run `ccb ask archi ...` and confirm the role skills are discoverable.
8. Run `ccb ask agentroles.archi ...` and confirm it resolves to `archi`.
9. Run `ccb roles doctor agentroles.archi`.
10. Remove `archi` while idle and confirm unrelated agents continue running.

Checkpoint 2026-06-03:

- Disposable project:
  `/home/bfly/yunwei/test_ccb2/roles_release.zoFqSP`.
- Validated default catalog list, reference duplicate warning, tool-backed
  install/update/doctor, project add/lock, lock pinning across current drift,
  explicit re-adoption, `roles sync <path>`, no-argument `roles sync` from cwd,
  Codex memory/skill projection, `ccb` startup, `ccb reload`, and runtime
  doctor with `archi` bound and healthy.
- Final clean production role digest:
  `sha256:ca22724106f53fb984dac94f4ef279729c557062df5b4e7107c1062ae0bf67ba`.
- `ccb ask archi` was not submitted in this checkpoint because CCB ask is
  submit-only and the next CCB ask is reserved for the release handoff to
  agent4. Ask routing remains covered by automated ask-service tests and the
  mounted runtime accepted the `archi` agent as bound/healthy.

## PR Acceptance Rules

A role PR should include:

- role manifest
- README
- memory file or explicit no-memory rationale
- skills or explicit no-skill rationale
- tools doctor or explicit no-tool rationale
- tests for manifest and projection
- permission declaration
- changelog entry when behavior changes

CCB config or usage changes that affect how users add, bind, name, ask, or
diagnose Role Pack agents must also update the inherited `ccb-config` skill and
its `references/ccb-config.md` copies for Codex and Claude. This prevents CCB
agents from generating stale config such as new `ccb.archi` bindings after the
canonical role id moved to `agentroles.archi`.

Role content PRs belong in `agent-roles-spec`. CCB PRs should reject
production role package content under the CCB source tree and should instead
change catalog consumption, adapter behavior, projection, diagnostics, or test
fixtures.

Review should reject roles that:

- bundle secrets, sessions, or auth material
- hide install behavior in memory text
- project skills globally instead of to the bound role agent
- mutate user-global config without explicit consent
- lack a clear purpose or non-goals
