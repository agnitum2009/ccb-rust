# Workspace Sharing Roadmap

Date: 2026-06-04

## Done

- Decided not to implement automatic Git initialization or broad worktree repair
  as part of this slice.
- Defined `workspace_path` as external linked workspace behavior: CCB validates
  the path but does not create, remove, prune, copy, or switch branches there.
- Defined `workspace_group` as internal managed shared worktree behavior:
  `.ccb/workspaces/groups/<group>` on branch `ccb/group/<group>`.

## In Progress

- Implement config/model/planner/materializer/validator/reconcile support for
  external workspace paths and internal workspace groups.
- Add focused tests for config parsing, external validation, group reuse,
  binding compatibility, and shared-worktree retirement safety.

## Next

1. Run the focused Python workspace/config tests.
2. Run broader start/reconcile/runtime tests that exercise persisted agent specs.
3. Update the public config contract once behavior is verified.

## Deferred

- Owner-based workspace binding schema migration.
- Shared-workspace UI affordances in the sidebar.
- Automatic file overlay/sync between project root and worktree.
- Interactive worktree conflict resolution.
