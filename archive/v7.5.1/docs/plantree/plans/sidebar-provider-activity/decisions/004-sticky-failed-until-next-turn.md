# Sticky Failed Until Next Turn

Date: 2026-05-27

## Context

Provider failures such as invalid auth, unavailable model, API outage, or stream
disconnect can disappear visually as soon as the provider returns to a prompt.
That makes sidebar status misleading because the user misses the last terminal
failure.

## Decision

Provider activity `failed` state should remain visible on the agent row until a
new provider turn starts or the owning runtime changes.

Clear sticky failed on:

- next provider `UserPromptSubmit` or equivalent turn-start event;
- next provider session/runtime generation;
- pane id change;
- provider family or agent ownership mismatch;
- explicit project/runtime restart that invalidates the old activity artifact.

Do not clear sticky failed only because pane text shows an idle prompt.

## Consequences

- Users can see the previous failure until they start new work.
- Failed state needs different freshness rules from `active`/`tool`.
- Tests must assert `failed + idle prompt` stays failed, while
  `failed + next turn` changes to active.
- Sticky failed is display/activity evidence; it must not by itself schedule a
  Comms retry.
