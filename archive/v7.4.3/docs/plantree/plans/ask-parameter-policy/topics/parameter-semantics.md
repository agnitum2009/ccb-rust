# Parameter Semantics

Date: 2026-06-07

## Stable Meaning

`ask` without route flags is only for short questions or short handoffs where
inline text is enough. It should not be the broad default for execution,
consultation, analysis, or report-style work.

`--callback` is for a child ask submitted from an active CCB task when the
current task cannot finish correctly until that child result is available. CCB
records a callback edge and later submits the continuation automatically after
the child result is terminal.

`--silence` is for publish/execute work where a successful result is not needed.
Failures, blockers, risks, and required next actions still need to surface.

`--compact` is for work where the caller wants a result, but only distilled
findings, status, risks, blockers, or next actions.

`--artifact-request` preserves exact request text by storing it in a CCB text
artifact and sending the target a file reference.

`--artifact-reply` preserves the final reply as a CCB text artifact path. Use it
for consultation, analysis, reports, generated documents, structured findings,
and other full-text results.

`--artifact-io` applies both request and reply artifact behavior.

## Axes

Result intent:

- no successful result needed: `--silence`
- short distilled result wanted: `--compact`
- full text result wanted: `--artifact-reply`
- short inline text is enough: plain `ask`

Dependency:

- active-task child dependency: add `--callback`
- callback can combine with `--compact`, `--artifact-reply`, or
  `--artifact-io`
- callback submit stops the current turn until CCB delivers continuation

Content preservation:

- exact transient request text: `--artifact-request`
- both: `--artifact-io`

## Non-Goals

These semantics do not make `--callback` and `--silence` syntactically
exclusive. They are an intent conflict in normal use: one says the current task
needs the result, while the other says successful completion does not need to
interrupt the caller.

`--silence --artifact-reply` is also normally discouraged: silence says the
caller does not need a successful result, while artifact-reply preserves a
result for later reading.
