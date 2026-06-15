# Preserve Silence As Independent Execution

Date: 2026-06-07

## Context

`--silence` can be misread as "the result is unimportant" or "the target job
finishes immediately." Neither interpretation matches current behavior.

Silent tasks are still real active jobs. They are useful for release execution,
smoke checks, cleanup, sync, notification, and background work where success is
routine.

## Decision

Document `--silence` as independent execution with silent-on-success delivery.
The target should still surface failures, blockers, risks, and required next
actions.

## Consequences

- `A --silence -> B` does not decide B-to-C routing.
- B uses `--callback` for C only when B needs C's result to finish B's current
  task.
- B uses `--silence` for C when C is independent execution and success should
  not interrupt B.
