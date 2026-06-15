# Treat Artifact As Content Transport

Date: 2026-06-07

## Context

Artifact request and reply flags are often discussed together with callback and
silence, but they solve a different problem. Callback and silence route work.
Artifacts preserve request or reply text.

Automatic spill over 4 KiB prevents oversized inline transport, but agents may
summarize long material before submission and therefore never trigger the
fallback.

## Decision

Treat artifact flags as content-transport choices that are orthogonal to route
flags. The skill should instruct agents to use artifact transport proactively
when exact transient input or exact final output matters.

## Consequences

- Long logs, external diffs, structured data, and complete reports are handled
  by policy, not only by byte threshold.
- Repo-readable files should normally be passed by path instead of copied into
  artifact text.
- Common combinations such as `--callback --artifact-reply` and `--silence
  --artifact-request` remain valid.
