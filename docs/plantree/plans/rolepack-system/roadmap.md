# Role Pack System Roadmap

Date: 2026-06-01

## Done

- Defined Role Pack as a reusable package of identity, responsibility, memory,
  skills, tools, permissions, and host adapters.
- Separated stable role ids from project-local agent names in
  [decisions/001-role-id-separate-from-agent-name.md](decisions/001-role-id-separate-from-agent-name.md).
- Chose a shared system role store with project locks and runtime projection in
  [decisions/002-system-role-store-project-locks.md](decisions/002-system-role-store-project-locks.md).
- Chose a host-neutral Role Pack core with host/provider adapters in
  [decisions/003-rolepacks-are-host-neutral-with-adapters.md](decisions/003-rolepacks-are-host-neutral-with-adapters.md).
- Accepted CCB role-id shorthand and role-id ask alias semantics in
  [decisions/004-role-id-shorthand-resolves-to-agent-name.md](decisions/004-role-id-shorthand-resolves-to-agent-name.md).
- Captured the first `ccb.archi` role slice in
  [topics/archi-role-first-slice.md](topics/archi-role-first-slice.md).
- Added the first source-tree `roles/ccb.archi` package, role manifest parsing,
  system-store install, `ccb roles list/show/install/add/doctor`, config
  `role` parsing, project role locks, role memory inclusion, and Codex/Claude
  role skill projection.

## In Progress

- Validate the first CCB adapter slice in a live project and decide whether
  `ccb roles refresh` or provider restart is the right first projection update
  command.

## Next

1. Harden role projection cleanup when a role is removed or changed.
2. Implement CCB role-id shorthand in config loading and role-id ask alias
   routing, with sidebar rows still displaying project-local names such as
   `archi`; keep shorthand expansion before defaults/overlay/topology
   validation and share explicit-binding validation.
3. Add `ccb roles refresh` and decide its relationship to `ccb reload`.
4. Add role tool lifecycle execution for `install/update/doctor` using
   [topics/lifecycle-and-tooling.md](topics/lifecycle-and-tooling.md).
5. Land the built-in `ccb.archi` Role Pack and validate it in a real
   `/home/bfly/yunwei/test_ccb2` project.
6. Add PR governance and compatibility tests from
   [topics/test-and-governance.md](topics/test-and-governance.md).

## Deferred

- Public role registry or marketplace.
- Signed remote role distribution.
- Automatic background update checks.
- Role replacement for already-running agents without restart or explicit
  projection refresh.
- Multi-role composition on one agent.
- Role dependency solving across conflicting tool versions.
- UI browser for discovering community roles.
