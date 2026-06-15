# Keep Routing Explicit In Skill

Date: 2026-06-07

## Context

`--callback` and `--silence` describe task relationship. `--callback` means a
child result is required before the current active task can finish. `--silence`
means independent execution where success should not interrupt the caller.

The current runtime rejects plain nested asks from active tasks unless the
caller chooses callback or silence.

## Decision

Keep routing as an explicit skill decision. The skill must teach agents to
choose callback or silence from task relationship before choosing artifact
flags.

This plan does not add automatic callback routing to `ccbd` or the CLI.

## Consequences

- Skill text remains aligned with current runtime behavior.
- Agents must decide whether child work is a dependency or independent
  execution.
- Future automatic callback behavior, if desired, needs a separate runtime plan.

## Follow-Up

Decision 004 changes the primary selector from task relationship to result
intent. This does not make callback or silence automatic; it only narrows plain
ask and makes `--silence`, `--compact`, and `--artifact-reply` more proactive.
