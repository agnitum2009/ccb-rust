# Explicit Reload Before Watchers

Date: 2026-05-29

## Context

Automatic file watching could make config changes feel live, but it would add a
steady-state polling or watcher path to a daemon that already has reported CPU
and memory pressure. It also makes failed or partial config edits harder to
reason about.

## Decision

The first reload entrypoint is explicit:

- `ccb reload --dry-run` for classification and validation;
- later `ccb reload` for accepted mutation classes.

File watching and automatic foreground prompts are deferred.

## Consequences

Reload has no steady-state CPU cost and no background config parse path. Users
get deterministic reload timing, and tests can assert that config changes do
not mutate runtime state until the command is invoked.

Related topics:

- [execution-plan.md](../topics/execution-plan.md)
- [performance-baseline-and-gates.md](../topics/performance-baseline-and-gates.md)
