# Explicit Workspace Path And Group

Date: 2026-06-04

## Context

Users need a simpler way to point one or more agents at an existing worktree, or
to have several agents share one CCB-managed worktree. A broader design that
auto-initializes Git repositories or repairs arbitrary worktree state would
increase risk and make startup behavior harder to predict.

## Decision

Add two explicit configuration fields:

- `workspace_path`: an exact external workspace path. CCB validates that it is a
  usable Git workspace for the project repository, but does not manage its
  lifecycle.
- `workspace_group`: an internal CCB-managed shared worktree. Agents with the
  same group use `.ccb/workspaces/groups/<group>` and branch
  `ccb/group/<group>`.

The fields are mutually exclusive and require `workspace_mode = "git-worktree"`.
If neither field is configured, the existing per-agent behavior remains
unchanged.

## Consequences

- Multiple agents can share a cwd only when the user opts into it.
- Provider runtime state remains per-agent even when the workspace is shared.
- Reconcile and retirement logic must not remove a group worktree while another
  configured agent still references it.
- Binding files remain backward compatible for this slice; a future owner-based
  binding schema can be added only if needed.
