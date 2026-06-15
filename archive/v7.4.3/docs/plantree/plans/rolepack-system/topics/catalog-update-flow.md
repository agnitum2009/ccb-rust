# Catalog Update Flow

Date: 2026-06-03

## Objective

Define how CCB consumes updates from `agent-roles-spec` during `ccb update`
without making CCB source code a role catalog and without silently changing
project behavior.

This topic records the current CCB-first catalog update flow. The long-term
target is that catalog cache and `.roles` package-store mutation are owned by
the `agent-roles-spec` package manager, with CCB delegating payload operations
and retaining project lock, projection, prompt, and failure-policy ownership.
See [spec-owned-roles-store.md](spec-owned-roles-store.md).

## Authority Layers

Catalog authority:

```text
~/.ccb/roles/
~/.roles/
agent-roles-spec/
  roles/
  reference_roles/
  specs/
  schemas/
  host-adapters/
```

Installed-role cache:

```text
$XDG_DATA_HOME/ccb/roles/<role-id>/
  current -> versions/<version>/<digest>/
  install.json
```

Managed catalog cache:

```text
$XDG_CACHE_HOME/ccb/role-catalogs/agent-roles-spec/
  .git/
  roles/
  reference_roles/
```

Project authority:

```text
project/.ccb/ccb.config
project/.ccb/role-lock.json
```

The catalog says what roles exist. The installed-role cache says what this
user has installed. The project lock says what a project has adopted.
User-level system role libraries are editable role sources; they are not
project runtime authority until snapshotted into the installed-role cache.

## `ccb update` Role Pass

After updating CCB itself, the update command should run a role pass:

1. Resolve role sources: user-level system libraries first, then
   `agent-roles-spec`.
2. Refresh `agent-roles-spec` with `git pull --ff-only` when CCB owns the
   GitHub cache, or leave user-owned local paths untouched.
3. Load available catalog roles.
4. Load installed local roles and their install metadata.
5. Compute:
   - installed roles with newer catalog versions or changed digests
   - catalog roles that are not installed locally
   - installed roles whose catalog source disappeared
6. Update already installed roles when a newer catalog entry exists and the
   tool/trust policy allows it.
7. Show newly available roles with id, version, name, and short description.
8. Ask whether to install newly available roles into the local role store.
9. Report project locks that may now be stale, but do not update project locks
   automatically.

The managed GitHub cache is consumption-only. Role package edits belong in the
upstream `agent-roles-spec` repository and should arrive through a pull request,
then be picked up by CCB after the cache refreshes.

## Interactive Behavior

When stdin/stdout are TTYs:

- Ask before installing newly available roles.
- Prefer a concise numbered selection when there are multiple new roles.
- Offer an "all" choice only after showing the role ids and descriptions.
- Show command equivalents such as `ccb roles install <role-id>` for skipped
  roles.

When non-interactive:

- Do not install newly available roles.
- Print a compact summary of new roles and follow-up install commands.
- Updating already installed roles may proceed only under an explicit
  non-interactive policy flag or environment setting.

## Installed Role Updates

Updating an installed role should:

- install the new catalog version into the local system role store
- run declared update/tool hooks only under the approved policy
- update `install.json`
- preserve older installed versions until cleanup policy says otherwise
- not edit any project `.ccb/role-lock.json`
- report bound projects as stale when CCB can discover them cheaply

## New Role Installation

Installing a newly available role during `ccb update` should behave like:

```bash
ccb roles install <role-id>
```

It must not automatically bind that role to the current project. Project
binding remains explicit through:

```bash
ccb roles add <role-id>:<provider>
```

## Diagnostics Output

The update summary should include:

- catalog path and status
- installed roles updated
- installed roles skipped or failed
- new roles available
- new roles installed by user choice
- stale project locks when known
- follow-up commands for skipped roles or stale locks

## Migration From Source-Tree Roles

The former CCB source-tree `roles/ccb.archi` package was a migration artifact.
Now that catalog install works:

1. role content lives in `agent-roles-spec`
2. tests use `agent-roles-spec` fixtures or temporary fixtures
3. production role discovery does not use `script_root / "roles"`
4. source-tree role package content is removed from CCB release artifacts
