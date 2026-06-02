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
- Project config shorthand such as `ccb.archi:codex` requires the installed
  system role store and resolves to the role manifest's default agent name.
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
  such as `archi`, not the role id `ccb.archi`.
- `ccb ask ccb.archi ...` resolves only when exactly one configured agent is
  bound to that role id; no-match and multi-match cases fail with explicit
  guidance.
- Multi-match ask alias errors list the matching project-local agent names.

## Lifecycle Tests

- `roles install` writes the system store and install metadata.
- `roles add` updates project config and lock without copying role assets into
  the project.
- `roles doctor` reports installed, missing, degraded, stale, and projected
  states.
- `roles update` refreshes the system store without floating project locks.
- `roles refresh` updates projections and reports digest changes.
- `ccb reload` does not turn role memory, skill, prompt, or tool-state changes
  into topology replacement when the configured agent set and provider leaves
  did not change.

## Real Project Tests

Use a disposable project such as `/home/bfly/yunwei/test_ccb2`:

1. Start an existing CCB project with two normal agents.
2. Install `ccb.archi`.
3. Add `archi` to the config and run `ccb reload`.
4. Verify existing agents remain alive and keep their provider sessions.
5. Verify the new `archi` pane starts and appears in sidebar.
6. Verify the sidebar row is named `archi`, not `ccb.archi`.
7. Run `ccb ask archi ...` and confirm the role skills are discoverable.
8. Run `ccb ask ccb.archi ...` and confirm it resolves to `archi`.
9. Run `ccb roles doctor ccb.archi`.
10. Remove `archi` while idle and confirm unrelated agents continue running.

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

Review should reject roles that:

- bundle secrets, sessions, or auth material
- hide install behavior in memory text
- project skills globally instead of to the bound role agent
- mutate user-global config without explicit consent
- lack a clear purpose or non-goals
