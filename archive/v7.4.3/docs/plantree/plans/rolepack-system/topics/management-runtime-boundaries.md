# Management Runtime Boundaries

Date: 2026-06-03

## Objective

Keep Role Pack management commands, config loading, provider-home projection,
and provider hook execution on separate dependency paths. A failure in role
catalog discovery, install/update tooling, or CLI formatting must not break
Codex/Claude/Gemini startup, provider activity hooks, ask submission, or daemon
reload for unrelated agents.

This topic was added after the first roles implementation exposed a concrete
boundary risk: adding `lib/rolepacks/__init__.py` made provider hook startup
execute the package initializer, and a syntax error there caused Codex
`SessionStart`, `UserPromptSubmit`, and `Stop` hooks to exit with code 1.

## Current Shape

The first implementation exposed broad module boundaries. The first refactor
has split role memory and skill lookup into `rolepacks.runtime_lookup`, but
remaining cleanup is still needed:

- `rolepacks.__init__` is now a small manifest facade and should stay that
  way.
- CCB still has source-tree role package content under `roles/`, which makes
  the CCB repo look like a role catalog even though
  [decisions/005-agent-roles-spec-is-catalog-authority.md](../decisions/005-agent-roles-spec-is-catalog-authority.md)
  makes `agent-roles-spec` the catalog authority.
- `rolepacks.service` owns several different responsibilities: built-in role
  lookup compatibility, system-store install/update, project config mutation,
  role lock writing, role status, and tool hook execution.
- `rolepacks.projection` is imported by provider-home materialization and now
  reaches only into `rolepacks.runtime_lookup` for project role skill sources.
- config loading uses lightweight role-store lookup to expand shorthand, while
  CLI management commands use the full `rolepacks` aggregate import surface.
- ask role-id alias resolution is implemented in ask submission with lightweight
  role-id helpers, which is the right direction and should stay separate from
  install/update services.

The architectural issue is not the existence of roles. It is that management
and runtime paths are currently easy to couple by import accident.

The long-term plan in
[spec-owned-roles-store.md](spec-owned-roles-store.md) moves role payload
package management behind an `agent-roles-spec` tool/API. That reduces CCB's
management surface, but the import boundary still matters: provider startup
must not import package-manager subprocess/network paths through CCB wrappers.

## Boundary Model

Use four layers:

1. Core manifest layer:
   `rolepacks.manifest` and `rolepacks.agent_role_adapter`.
   This layer parses and translates role package metadata. It must not import
   CCB project config, provider backends, CLI code, subprocess execution, or
   daemon/runtime modules.

2. Runtime lookup layer:
   lightweight store and binding queries used by config loading, ask routing,
   memory loading, and provider projection. This layer may read installed role
   manifests and project config, but it must not mutate stores, run tools,
   execute subprocesses, or format CLI output.

3. Management service layer:
   install, update, add, source registry, lock writing, doctor, tool lifecycle
   hooks, and future refresh/repair/uninstall. This layer may depend on the
   runtime lookup and manifest layers, but provider startup and provider hooks
   must not import it.

4. Host/provider adapter layer:
   provider-home projection and generated memory rendering. This layer may call
   runtime lookup functions and projection utilities only. It must not import
   management services or package-level `rolepacks` aggregation.

## Import Rules

- `rolepacks.__init__` should either be removed or reduced to a tiny stable
  facade that exports only host-neutral manifest symbols.
- Runtime hot paths must import explicit modules, for example
  `rolepacks.projection` or `agents.config_loader_runtime.role_lookup`, not
  `from rolepacks import ...`.
- `rolepacks.projection` should not import `rolepacks.service`. Move
  `project_role_skill_sources` and role memory lookup into a runtime-focused
  module such as `rolepacks.runtime_lookup`.
- CLI command modules may import management services directly, for example
  `rolepacks.management` or `rolepacks.service`, but that import must remain
  outside provider startup and hook scripts.
- provider hook scripts should keep imports minimal. A provider activity hook
  must not import role management merely because another package initializer
  happens to aggregate it.

## Command Responsibilities

`ccb roles install/update`:

- mutate the system role store
- run declared tool lifecycle hooks only after trust policy allows it
- update install metadata
- never mutate project config or live provider homes

`ccb roles add`:

- mutate project config and role lock
- validate provider compatibility
- tell the user whether `ccb reload` or `ccb roles refresh` is required
- never install external tools implicitly unless it is a deliberate prompt path

`ccb roles sync [path]`:

- default omitted path to the current working directory
- discover only roles under the provided path
- update already installed same-id roles in the system role store
- skip roles that are not already installed unless a future explicit install
  flag is added
- never mutate project config, project locks, or live provider homes

`ccb roles refresh`:

- rebuild role-owned projections for already bound agents
- remove stale role-owned projected assets with markers
- report digest changes and restart requirements
- not change topology or mount/unmount agents

`ccb reload`:

- reconcile configured agents and panes
- detect role-only changes separately from topology changes
- call refresh-style projection rebuild only when that is the documented
  behavior for the provider and state transition

`ccb roles doctor`:

- report catalog/install/lock/binding/projection/tool state
- distinguish missing install, invalid manifest, unreadable store, stale lock,
  unsupported provider, and projection failure
- avoid repairing unless an explicit repair command or flag exists

## First Refactor Slice

1. Replace `from rolepacks import ...` in CLI commands with explicit imports
   from management modules. Keep CLI on the management side of the boundary.
2. Slim or delete `rolepacks.__init__` so importing a submodule cannot pull in
   install/update/project mutation code.
3. Move runtime lookup helpers out of `rolepacks.service`:
   `load_project_agent_role`, `project_role_memory_sources`, and
   `project_role_skill_sources`.
4. Make `rolepacks.projection` depend only on runtime lookup and projection
   utilities.
5. Add import smoke tests for provider activity hooks and provider-home
   materialization that fail if role management imports break runtime paths.
6. Replace source-tree production role discovery with installed-store and
   `agent-roles-spec` catalog discovery.

## Current Checkpoint

Completed:

- `rolepacks.__init__` is a manifest-only facade.
- `rolepacks.runtime_lookup` owns project role memory lookup, skill lookup, and
  stale role-lock detection.
- `rolepacks.projection` and `project_memory.sources` import runtime lookup
  instead of `rolepacks.service`.
- Stale project locks produce `role_lock_mismatch` warnings and suppress role
  memory/skill projection instead of silently using `current`.
- Project role locks record `default_agent_name`, and shorthand config loading
  uses that locked visible name before consulting mutable installed `current`.
- Warning-only memory sources are rendered, so `role_lock_mismatch` is visible
  inside generated provider memory bundles.
- Installed roles are stored at content-addressed paths
  `versions/<version>/<digest>/`, and project runtime/config lookup resolves
  locks by version plus digest before consulting mutable `current`.
- `ccb.archi` is a compatibility input alias for `agentroles.archi`; internal
  storage, locks, config writes, and ask routing use the canonical role id.
- Source-tree role content under `roles/ccb.archi` has been removed, and
  release packaging excludes source-tree `roles/`.
- Default catalog discovery resolves user-level system role sources at
  `~/.ccb/roles` and `~/.roles` first, then local env/default
  `agent-roles-spec` paths, then falls back to a CCB-owned GitHub cache under
  `$XDG_CACHE_HOME/ccb/role-catalogs/agent-roles-spec`. The managed cache is
  consumption-only and refreshes with `git pull --ff-only` during `ccb update`.
- `ccb roles add` may snapshot an uninstalled role from a user-level system
  source into the installed store before writing project config and locks.
  This convenience path does not make editable source files project runtime
  authority.

Remaining:

- Split source registry/catalog diagnostics and role status out of
  `rolepacks.service` if provider or hook paths grow new dependencies.
- Decide whether missing/stale locked content should stay warning-only or
  block mounted agents during startup.
- Keep release/package tests guarding that CCB does not ship source-tree role
  content as a production catalog.

## Design Risks To Watch

- A role source catalog should be a discovery surface, not project authority.
  Installed role metadata and project lock remain authority for runtime use.
- Built-in role fallback is convenient for `roles add`, but shorthand config
  loading should still require installed roles unless the decision record is
  changed.
- Tool lifecycle hooks are management-time behavior. They must not run during
  config validation, ask routing, provider startup, or projection refresh unless
  explicitly requested.
- Project lock writes should not silently float a project to a new system-store
  version. Update and refresh need visible stale/current status.
- Runtime errors in one role-bound agent should not block unrelated agents from
  starting unless topology validation itself is invalid.

## Catalog Boundary

Do not let `script_root / "roles"` remain the production discovery path. CCB
may keep role fixtures for tests, but runtime and CLI role discovery should
resolve from:

1. installed role store
2. user-level system role sources at `~/.ccb/roles` and `~/.roles`
3. local `agent-roles-spec` env/default paths
4. CCB-owned GitHub `agent-roles-spec` cache
5. additional registered local catalog sources
6. explicit user-provided install path

The implementation has removed `roles/ccb.archi` and excludes source-tree
`roles/` from release artifacts. Tests that need role packages should use
`agent-roles-spec` fixtures or temporary local catalogs, not CCB source-tree
production roles. The managed GitHub cache is not a role authoring workspace;
production role content changes should go through upstream `agent-roles-spec`
pull requests. Project-local `.roles` directories are deferred for the first
slice.
