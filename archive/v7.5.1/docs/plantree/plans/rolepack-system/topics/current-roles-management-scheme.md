# Current Roles Management Scheme

Date: 2026-06-03

## Status

This topic summarizes the current first-slice CCB Role Pack behavior after the
external `agent-roles-spec` catalog, user-level local role sources, installed
role store, project locks, and explicit local `sync` command were introduced.
It is a current implementation snapshot, not the final public RolePack
standard.

## Authority Layers

Role source layers:

1. User-level editable role libraries:
   `~/.ccb/roles`, `~/.roles`, or `CCB_SYSTEM_ROLES_HOME` /
   `CCB_ROLES_HOME`.
2. Local `agent-roles-spec` paths:
   `AGENT_ROLES_SPEC_HOME`, `CCB_AGENT_ROLES_SPEC_HOME`, or
   `~/yunwei/agent-roles-spec`.
3. CCB-managed GitHub catalog cache:
   `$XDG_CACHE_HOME/ccb/role-catalogs/agent-roles-spec`, cloned from
   `https://github.com/SeemSeam/agent-roles-spec`.
4. Additional registered local catalog sources.
5. Explicit command path, such as `ccb roles install --path <role-root>`.

Runtime authority:

1. Installed role store:
   `$XDG_DATA_HOME/ccb/roles/<role-id>/versions/<version>/<digest>/`.
2. Mutable convenience pointer:
   `$XDG_DATA_HOME/ccb/roles/<role-id>/current`.
3. Project lock:
   `project/.ccb/role-lock.json`, storing role id, version, digest, source, and
   locked `default_agent_name`.

Editable role sources are discovery and authoring surfaces only. A project does
not run directly from `~/.ccb/roles`, `~/.roles`, or the GitHub cache. Project
runtime reads installed immutable snapshots through the project lock.

## Discovery

`ccb roles list` builds a catalog view from the source layers above, computes a
tree digest for each discovered role, and compares that against installed
metadata.

Status values include:

- `available`: discovered but not installed.
- `current`: installed metadata matches the discovered source version and
  digest.
- `update_available`: installed metadata differs from discovered source
  version or digest.
- `installed_source_missing`: installed locally but no matching catalog source
  is currently discoverable.

Duplicate role ids do not silently shadow earlier sources. The earlier source
wins and ignored duplicates are carried in diagnostics. `ccb roles list`
renders those diagnostics so source shadowing is visible without needing
internal JSON or test fixtures. When `reference_roles/` is enabled for
development, production `roles/` still wins and the ignored reference package
is reported as a duplicate.

## Installation

`ccb roles install <role-id>` resolves a role from the installed source path,
the discovered role catalog, or an explicit `--path`. It copies the source into
a staging area, computes a digest, stores it at:

```text
$XDG_DATA_HOME/ccb/roles/<role-id>/versions/<version>/<digest>/
```

Then it moves `current` to that digest and writes `install.json`.

`ccb roles update <role-id>` uses the same install machinery but reports update
semantics and runs update hooks by default. Tool hooks can be skipped with the
existing skip flag.

The installed-store path is treated as content-addressed authority. If a target
`versions/<version>/<digest>/` directory already exists but its tree digest no
longer matches the path digest, reinstall/update replaces it from a clean
staging copy. CCB also runs Python role tool hooks with bytecode generation
disabled, and production CCB adapter hook commands should use `python -B`, so
install/update/doctor hooks do not create `__pycache__` files inside role
sources or installed snapshots.

## Local System Role Editing

User-level system role libraries are intended for local editable roles:

```text
~/.ccb/roles/<role>/
~/.roles/<role>/
```

If `ccb roles add <role-id>:<provider>` sees an uninstalled role in these
system sources, it snapshots that source into the installed store before
writing project config and lock. This is a convenience path for local roles,
not a general implicit install from remote catalogs.

`ccb roles sync [path]` handles edits to local role sources:

- omitted path means `.`, resolved against the command working directory
- if the path is a single role root, only that role is considered
- if the path is a role library, only roles under that path are considered
- only already installed same-id roles are updated
- uninstalled roles are reported as skipped
- unrelated global sources are not scanned
- project config, project locks, and live provider homes are not changed

This keeps local development explicit while avoiding automatic behavior changes
on CCB restart.

## Project Binding

`ccb roles add <role-id>:<provider>` requires an installed role or a
discoverable user-level system role that can be snapshotted. It then:

1. validates provider compatibility
2. chooses the role `default_agent_name` unless `--agent` is supplied
3. writes shorthand config when the selected agent name matches the default
4. writes an explicit `[agents.<name>] role = "<role-id>"` overlay otherwise
5. writes `.ccb/role-lock.json` with version, digest, source, and
   `default_agent_name`

Project locks are not updated by installed-store updates, catalog refreshes, or
`roles sync`. Re-adoption remains explicit through `roles add` or a future
project role refresh/adopt command. The lock digest is the installed metadata
digest, so it points at an existing content-addressed snapshot path instead of
recomputing after tool hooks or other runtime side effects.

## Runtime Projection

Config shorthand such as `agentroles.archi:codex` expands to the project-local
agent name from the locked role identity when possible. Runtime memory and
skill projection resolve the project lock by `version + digest` before
consulting mutable `current`.

If locked content exists, the project keeps using that immutable snapshot even
when installed `current` has moved. If locked content is missing or mismatched,
CCB emits `role_lock_mismatch` and suppresses role memory and skills rather
than silently projecting drifted content.

## Update Flow

`ccb update` runs a catalog-aware role pass after CCB itself updates:

1. resolve role sources
2. refresh the CCB-managed GitHub `agent-roles-spec` cache with
   `git pull --ff-only`
3. compare discovered roles against installed metadata
4. update already installed roles with newer source version or digest
5. report newly available roles
6. prompt interactively before installing newly available roles into the
   installed store
7. leave project locks unchanged

User-owned local source paths are not pulled by CCB. Role content changes in
the upstream GitHub catalog should arrive through `agent-roles-spec` pull
requests.

## Deferred Work

- A project role refresh/adopt command that updates project locks deliberately.
- Projection cleanup when role assets are removed or changed.
- A decision on whether stale or missing locked content remains warning-only or
  becomes a hard startup error for mounted agents.
- Project-level `.roles` or `.ccb/roles` sources. The first slice uses
  user-level system role sources only.
