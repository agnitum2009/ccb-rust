# v7.2.9 Role Pack Update Failure - 2026-06-04

## Symptom

During update from `v7.2.3` to `v7.2.9`, the core update succeeded:

```text
Updated: v7.2.3 a647470 2026-06-03 -> v7.2.9 9a5149f 2026-06-04
```

The post-update Role Pack prompt was accepted, then failed:

```text
roles_status: failed
error: unknown builtin role: ccb.archi
Role Pack update failed: ccb.archi
```

## Root Cause

`v7.2.9` moved production role content out of `ccb_source` and into
`agent-roles-spec` under the canonical id `agentroles.archi`.

The updater process that began in `v7.2.3` continued running after installing
the `v7.2.9` files. Its post-update Role Pack logic still saw or attempted to
refresh legacy installed state named `ccb.archi`. The new release layout no
longer contains a source-tree builtin role at `roles/ccb.archi`, so the Role
Pack refresh failed.

This was an optional post-update provisioning failure, not a core CCB update
failure.

## Product Decision

New usage should always use:

```text
agentroles.archi
```

`ccb.archi` remains only as an input compatibility alias for old configs,
commands, and installed metadata.

## Required Fix Direction

- Run post-update provisioning with the newly installed `ccb` entrypoint.
- Canonicalize installed legacy role ids before deciding what to update.
- If installed metadata has a stale `source_path`, fall back to the canonical
  catalog source when available.
- Treat optional Role Pack provisioning failure as a warning with retry command
  after the core update is complete.
- Add regression coverage for update from a legacy `ccb.archi` installed store.

## User Recovery

After the update, the user can manually run:

```bash
ccb roles install agentroles.archi
ccb roles doctor agentroles.archi
```

If stale metadata still points at a removed old source path, use an explicit
catalog source path or wait for the migration fix:

```bash
ccb roles install agentroles.archi --path ~/.cache/ccb/role-catalogs/agent-roles-spec/roles/archi
```
