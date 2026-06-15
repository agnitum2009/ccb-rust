# Use Result Intent As Primary Selector

Date: 2026-06-08

## Context

The earlier policy started from task relationship and left plain `ask` as the
default for many top-level requests. In practice, this was too broad: task
publication should often be silent-on-success, execution with a desired result
should often be compact, and consultation or analysis should usually preserve a
full text reply.

## Decision

Make result intent the first ask-parameter selector:

- `--silence` when successful completion does not need to interrupt the caller.
- `--compact` when the caller wants only a distilled result.
- `--artifact-reply` when the caller wants full text such as analysis, reports,
  findings, or generated documents.
- plain `ask` only for short questions or short handoffs where inline text is
  enough.

`--callback` remains the dependency selector for active parent tasks and
combines with the result-intent choice when the child result is required.

## Consequences

- No-flag ask is intentionally narrower.
- Agents should use `--silence`, `--compact`, and `--artifact-reply` more
  actively.
- Artifact policy remains orthogonal: request artifacts preserve input, reply
  artifacts preserve output, and callback still only models dependency.
