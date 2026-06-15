# Spec-Owned Roles Store Boundary

Date: 2026-06-04

## Objective

Define the target split where `agent-roles-spec` owns reusable role package
management and CCB consumes resolved role packages for project runtime
integration.

This topic refines the earlier CCB-first implementation without deleting the
current implementation snapshot in
[current-roles-management-scheme.md](current-roles-management-scheme.md).

## Target Authority Split

`agent-roles-spec` owns role package state:

- the system-level `.roles` store or equivalent XDG-backed store
- catalog clone/cache state for `https://github.com/SeemSeam/agent-roles-spec`
- role package list, install, update, sync, doctor, repair, and metadata
- content digest, version, provenance, and source records
- role id aliases and migrations such as `ccb.archi -> agentroles.archi`
- package validation, schema compatibility, and contribution gates

CCB owns CCB runtime state:

- `.ccb/ccb.config`
- `.ccb/role-lock.json`
- provider home projection and cleanup
- CCB adapter policy for tool hooks, prompts, i18n output, and required
  failure semantics
- CCB update sequencing around old and new entrypoints
- ask/sidebar/reload/diagnostic behavior for mounted agents

The practical rule is: `.roles` is package-manager state; `.ccb` is project
runtime state.

## Store Model

The target store should be described by `agent-roles-spec`, not CCB:

```text
~/.roles/
  catalogs/
    agent-roles-spec/
      .git/
      roles/
      reference_roles/
  installed/
    agentroles.archi/
      current -> versions/0.2.0/<digest>/
      install.json
      versions/
        0.2.0/<digest>/
          role.toml
          memory.md
          adapters/
          skills/
          tools/
```

The exact path is still an open design point. The important ownership rule is
that CCB does not define the package store schema as a private CCB runtime
detail.

CCB may keep a compatibility bridge from the current
`$XDG_DATA_HOME/ccb/roles/` store while the spec-owned store is introduced.
Project locks must keep resolving old installed snapshots until a deliberate
migration path exists.

The current CCB bridge uses `.roles/installed` as the preferred installed-role
store and keeps `$XDG_DATA_HOME/ccb/roles` as a read fallback. Role-management
commands copy legacy installed snapshots into `.roles/installed` before package
operations so existing project locks can resolve the same version/digest from
the new store. The old store is not deleted during migration.

## Agent Roles Tool Contract

The spec project should expose a stable tool or library boundary. CCB should be
able to call it without importing CCB runtime modules.

Minimum operations:

```bash
agent-roles sync .
agent-roles list --json
agent-roles install agentroles.archi --json
agent-roles update agentroles.archi --json
agent-roles doctor agentroles.archi --json
agent-roles resolve agentroles.archi --json
```

The JSON contract should include:

- canonical role id
- accepted legacy aliases
- version and digest
- installed path
- source kind and source path
- adapter compatibility
- declared tool hooks and permissions
- warning and failure diagnostics with stable codes

CCB should treat this as a package-manager API. It should not parse human
output or depend on transient filesystem implementation details beyond the
resolved installed path and metadata.

## CCB Wrapper Behavior

CCB keeps user-facing commands where they are already part of CCB workflows:

```bash
ccb roles list
ccb roles sync
ccb roles install agentroles.archi
ccb roles update agentroles.archi
ccb roles doctor agentroles.archi
ccb roles add agentroles.archi:codex
```

For package operations, CCB delegates to the spec-owned package manager and
then adds CCB-specific behavior:

- enforce CCB post-update required/optional failure policy
- run or validate CCB adapter hooks according to CCB policy
- write project config and role locks for `roles add`
- project role memory, skills, prompts, and plugins into provider homes
- report project lock drift and runtime projection diagnostics

`ccb roles add` remains CCB-owned because it mutates `.ccb/ccb.config` and
`.ccb/role-lock.json`.

Delegation is unconditional in CCB. The CCB-private installed-role writer and
`CCB_AGENT_ROLES_MANAGER` rollback switch are removed so role payload writes have
one owner: `agent-roles`.

## Migration Sequence

1. Done: specify the `agent-roles` package-manager CLI/API and `.roles`
   metadata in `agent-roles-spec`.
2. Done: add CCB compatibility reads for `.roles/installed`.
3. Done: make `ccb roles install/update/sync` call `agent-roles` for package
   payload operations while preserving CCB output and tool-hook policy.
4. Done: copy legacy installed role snapshots into `.roles/installed` at CCB
   management boundaries, including legacy `ccb.archi` aliases.
5. Done: remove the CCB-private installed role writer, remove the rollback
   switch, and make runtime lookup use `.roles/installed` as the only installed
   role store.
6. Next: validate old-version upgrades with existing project locks and stale
   `source_path` metadata.
7. Next: move migration ownership from the CCB compatibility bridge into
   `agent-roles` once the package manager exposes a stable migration command.

## Non-Goals

- Do not move `.ccb/ccb.config` or `.ccb/role-lock.json` into
  `agent-roles-spec`.
- Do not let `agent-roles-spec` manage provider sessions, auth, tmux panes,
  mailbox state, or CCB lifecycle files.
- Do not make CCB startup depend on network access to resolve a mounted role.
- Do not silently update project locks when the package store updates.

## Risks

- A subprocess-only package manager may make errors harder to type-check unless
  the JSON protocol is strict and versioned.
- A library API may tempt CCB runtime paths to import management code; import
  boundary tests remain required.
- Legacy migration can confuse users unless diagnostics clearly distinguish
  migration input from the active `.roles/installed` store.
- Tool hook ownership must stay explicit: role packages declare hooks, but CCB
  decides whether running them is allowed or required in a CCB update/install
  context.
- CCB currently owns the compatibility migration bridge. Long term, migration
  should be a first-class `agent-roles` operation because `.roles` package state
  belongs to `agent-roles-spec`.
- Development-only source checkout discovery must not become a stable production
  dependency. Released CCB should prefer `AGENT_ROLES_CLI`, `agent-roles` on
  `PATH`, an installed Python package, or an explicit `AGENT_ROLES_SPEC_HOME`.
