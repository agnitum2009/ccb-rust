# Role Pack System Plan

Date: 2026-06-01

## Purpose

Plan a sustainable role system that can start inside CCB but is not limited to
CCB. A role is a reusable package of agent identity, responsibility, memory,
skills, tools, permissions, and host adapters. The goal is to let users install
or review a role once, bind it to an agent instance, and reuse the same assets
across projects without copying every skill or tool into every `.ccb` tree.

## File Map

- [roadmap.md](roadmap.md): current planning and implementation sequence.
- [implementation-status.md](implementation-status.md): current handoff for the
  spec-owned `.roles` bridge implementation.
- [open-questions.md](open-questions.md): unresolved product and contract
  questions only.
- [topics/rolepack-core-spec.md](topics/rolepack-core-spec.md): host-neutral
  Role Pack manifest, directory shape, required fields, and validation rules.
- [topics/asset-storage-and-projection.md](topics/asset-storage-and-projection.md):
  system role store, project locks, agent runtime projection, and state
  boundaries.
- [topics/catalog-update-flow.md](topics/catalog-update-flow.md): `ccb update`
  behavior for refreshing `agent-roles-spec`, updating installed roles, and
  prompting for newly available roles.
- [topics/current-roles-management-scheme.md](topics/current-roles-management-scheme.md):
  current first-slice source resolution, installed-store, sync, project-lock,
  and update behavior.
- [topics/host-adapter-ccb.md](topics/host-adapter-ccb.md): CCB-specific
  config, CLI, reload, memory, and skill projection behavior.
- [topics/distribution-and-trust.md](topics/distribution-and-trust.md):
  `agent-roles-spec`, local path, GitHub, and future registry distribution
  with trust controls.
- [topics/lifecycle-and-tooling.md](topics/lifecycle-and-tooling.md): install,
  update, doctor, repair, and external tool dependency semantics.
- [topics/spec-owned-roles-store.md](topics/spec-owned-roles-store.md): target
  boundary where `agent-roles-spec` owns `.roles` package management while CCB
  owns project runtime integration.
- [topics/management-runtime-boundaries.md](topics/management-runtime-boundaries.md):
  import, dependency, and command-boundary rules that keep role management from
  breaking provider startup or hooks.
- [topics/test-and-governance.md](topics/test-and-governance.md): automated
  tests, real-project tests, PR acceptance rules, and compatibility gates.
- [topics/archi-role-first-slice.md](topics/archi-role-first-slice.md): first
  concrete role slice for Architec-backed architecture review.
- [history/agent3-roles-management-review-2026-06-03.md](history/agent3-roles-management-review-2026-06-03.md):
  architecture review findings for the current roles management scheme.
- [history/final-rolepack-validation-2026-06-03.md](history/final-rolepack-validation-2026-06-03.md):
  final PR/review/test checkpoint before handing release to agent4.
- [history/spec-owned-roles-store-first-slice-2026-06-04.md](history/spec-owned-roles-store-first-slice-2026-06-04.md):
  first executable `agent-roles` package-manager bridge, later superseded by the
  direct-switch migration delta.
- [decisions/001-role-id-separate-from-agent-name.md](decisions/001-role-id-separate-from-agent-name.md):
  fixed role identity must be independent from the project-local agent name.
- [decisions/002-system-role-store-project-locks.md](decisions/002-system-role-store-project-locks.md):
  role assets are installed once in a system store and projects keep locks and
  overrides.
- [decisions/003-rolepacks-are-host-neutral-with-adapters.md](decisions/003-rolepacks-are-host-neutral-with-adapters.md):
  Role Packs define a host-neutral core with optional host/provider adapters.
- [decisions/004-role-id-shorthand-resolves-to-agent-name.md](decisions/004-role-id-shorthand-resolves-to-agent-name.md):
  CCB role-id shorthand expands to a project-local agent name, while sidebar
  and ask use that local name.
- [decisions/005-agent-roles-spec-is-catalog-authority.md](decisions/005-agent-roles-spec-is-catalog-authority.md):
  `agent-roles-spec` owns role package content; the first CCB slice owns
  consumption, installation, projection, update prompts, and diagnostics. This
  is partially superseded by decision 006 for long-term store ownership.
- [decisions/006-agent-roles-spec-owns-roles-store.md](decisions/006-agent-roles-spec-owns-roles-store.md):
  `agent-roles-spec` should own `.roles` package management; CCB should wrap
  those operations for CCB project/runtime integration.

## Related Sources

- [../../../ccb-config-layout-contract.md](../../../ccb-config-layout-contract.md)
- [../../../ccb-provider-state-storage-boundary-plan.md](../../../ccb-provider-state-storage-boundary-plan.md)
- [../../../codex-session-isolation-contract.md](../../../codex-session-isolation-contract.md)
- [../../../claude-session-isolation-contract.md](../../../claude-session-isolation-contract.md)
- [../../../codex-plugin-projection-plan.md](../../../codex-plugin-projection-plan.md)
- [../managed-tool-windows/README.md](../managed-tool-windows/README.md)
- [../ccbd-agent-hot-reload/README.md](../ccbd-agent-hot-reload/README.md)

## Scope

In scope:

- A host-neutral Role Pack schema and directory convention.
- A role identity model that separates stable role ids from agent instance
  names.
- CCB role-id shorthand that keeps sidebar, mailbox, job, pane, and primary ask
  labels on project-local agent names.
- Shared installation of role assets with per-project locks and overrides.
- Projection of role memory, skills, prompts, and tools into managed provider
  homes without sharing provider sessions or auth.
- CCB adapter behavior for `.ccb/ccb.config`, `ccb roles ...`, `ccb reload`,
  and diagnostics.
- Dependency boundaries that keep role management, config loading, projection,
  provider startup, and provider hooks independently failure-contained.
- `agent-roles-spec` catalog consumption, including update-time refresh of
  installed roles and prompts for newly available roles.
- A migration path from CCB-owned role payload installation to a spec-owned
  `.roles` package manager that CCB can delegate to.
- A first CCB-consumable architecture role from `agent-roles-spec`, backed by
  Architec where the role declares those tools.

Out of scope for the first slice:

- A public marketplace service.
- Automatic background role updates.
- Sharing provider session roots or auth across agents.
- Making MCP mandatory for roles.
- Running arbitrary third-party installers without explicit user approval.
- Full UI-driven role browsing.
- Keeping production role package content inside the CCB source tree.

## Guiding Model

Skill answers "what task can this agent perform?".

MCP answers "what remote or local tool protocol can this agent call?".

Role answers "who is this agent, what is it responsible for, what memory and
tools shape that responsibility, and how is it safely projected into a host?".
