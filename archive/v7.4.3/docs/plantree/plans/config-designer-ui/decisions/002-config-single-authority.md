# Decision 002: Single Authority Config Writing

Date: 2026-06-10

## Status

Accepted for generated config and `ccb_self` config guidance.

## Context

`version = 2` windows topology can express the configured agent set, window
placement, provider, default `inplace` workspace mode, and
`git-worktree` workspace mode directly in `[windows]` leaves:

```toml
[windows]
main = "main:codex, worker:codex(worktree)"
```

Older rich TOML examples also repeated topology-owned fields in
`[agents.<name>]`, for example `provider = "codex"` and
`workspace_mode = "git-worktree"`. That creates two places for the same
authority and makes config drift likely.

## Decision

CCB-generated config and `ccb_self` config guidance must use one canonical
writing rule:

- `[windows]` owns agent presence, provider, default workspace mode, order,
  window grouping, and split hints.
- `[agents.<name>]` is an overlay for names referenced by `[windows]`.
- Overlays must not repeat `provider`, `workspace_mode = "inplace"`, or
  `workspace_mode = "git-worktree"`.
- Default Role Pack binding should use role shorthand such as
  `agentroles.ccb_self:codex`.
- Explicit local-name role binding should keep provider in `[windows]` and put
  only `role = "<role-id>"` in the overlay.

Example:

```toml
version = 2
entry_window = "main"

[windows]
main = "main:codex, agentroles.ccb_self:codex"
work = "worker:codex(worktree)"

[agents.worker]
model = "gpt-5"
```

Custom local role name:

```toml
[windows]
main = "selfops:codex"

[agents.selfops]
role = "agentroles.ccb_self"
```

## Compatibility

The reader remains tolerant:

- legacy rich TOML that repeats matching `provider` still loads;
- legacy rich TOML that repeats matching default workspace mode still loads;
- stale `[agents.<name>]` overlays not referenced by `[windows]` remain ignored.

`ccb config validate` reports those cases as style warnings so users and tools
can clean them up without breaking existing projects.

`workspace_mode = "copy"` remains an advanced overlay-only mode until the
compact leaf grammar has a first-class copy-mode spelling.

## Consequences

- New `roles add` output must not introduce redundant overlay `provider`.
- `ccb-config` must treat style warnings as cleanup work before reload.
- Future `ccb config format` or `normalize --write` should remove redundant
  provider/default-workspace fields when it can prove they match `[windows]`.
- The long-term schema registry should define supported fields once and feed
  parser validation, docs, skill guidance, UI metadata, and formatter behavior.
