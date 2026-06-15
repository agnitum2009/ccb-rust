# Agent Roles Spec Owns The Roles Store

Date: 2026-06-04

## Context

Decision 005 moved production role package content out of `ccb_source` and made
`agent-roles-spec` the catalog authority, but it still left local role package
installation, update, and the installed-role store as CCB-owned behavior.

That boundary fixed the immediate release problem, but it keeps role package
management tied to CCB implementation details. A better long-term split is to
let `agent-roles-spec` own the reusable role package manager and `.roles`
store, while CCB only consumes that store and integrates resolved role content
into CCB projects.

## Decision

`agent-roles-spec` should own role package management and the system-level
`.roles` store.

The spec project owns:

- catalog sync from the upstream `agent-roles-spec` repository
- local `.roles` store layout and metadata
- role package install, update, list, doctor, and repair primitives
- role version, digest, provenance, and alias migration metadata
- same-id update decisions for role package payloads
- role package validation and contribution gates

CCB owns:

- `.ccb/ccb.config` role bindings
- `.ccb/role-lock.json` project adoption and digest pins
- provider-home projection of memory, skills, prompts, and plugins
- CCB host-adapter execution policy and user prompts
- CCB update orchestration and failure semantics
- CCB-specific diagnostics, sidebar, ask alias, reload, and runtime behavior

CCB may keep `ccb roles ...` commands, but they become host-facing wrappers
around the `agent-roles-spec` package-management contract for role payload
operations. CCB should not be the canonical implementation of `.roles`
sync/install/update semantics.

## Target Boundary

The target shape is:

```text
agent-roles-spec
  owns: .roles package store, catalog cache, role payload install/update,
        digest/provenance, aliases, package validation

CCB
  owns: .ccb project config, project locks, provider projection, host adapter
        policy, post-update orchestration, runtime diagnostics
```

For user commands, CCB can present a stable facade:

```bash
ccb roles sync
ccb roles install agentroles.archi
ccb roles update agentroles.archi
ccb roles doctor agentroles.archi
```

Internally, those commands should delegate role package resolution and store
mutation to the spec-owned tool or library, then apply CCB-specific adapter and
project integration steps.

## Relationship To Decision 005

This decision partially supersedes
[005-agent-roles-spec-is-catalog-authority.md](005-agent-roles-spec-is-catalog-authority.md).

Decision 005 remains authoritative that production role content belongs in
`agent-roles-spec`, not in `ccb_source`. This decision changes the ownership of
the local role package store and role payload install/update semantics from
CCB-owned to `agent-roles-spec`-owned.

## Consequences

- Role package updates can evolve independently from CCB releases.
- `ccb.archi -> agentroles.archi` and future alias migrations can be expressed
  once in the role package ecosystem and reused by CCB and future hosts.
- CCB release updates no longer need to own GitHub catalog clone/pull behavior
  as a private implementation detail.
- The spec project must eventually provide a stable CLI, library API, or JSON
  protocol that CCB can call without importing CCB runtime code.
- Existing CCB-installed role stores need a migration or compatibility bridge
  so project locks continue to resolve old content-addressed snapshots.
- CCB still needs adapter-specific tests because project config, locks,
  provider projection, and CCB post-update failure semantics remain CCB-owned.
