# Dynamic Unload And Replace

Date: 2026-05-29

## Summary

Dynamic unload and replace are more dangerous than additive load because they
remove or change runtime authority. They must be implemented only after
service-graph replacement, handler routing, dry-run diffing, and bounded
draining are stable.

## Unload Model

Deletion from `[windows]` should become a planned unload operation, not an
immediate process kill.

States:

- `pending_unload`: reload has requested removal, but no mutation has started.
- `draining`: agent is no longer eligible for new work; existing work may
  finish, cancel, or time out.
- `retiring`: daemon is removing runtime authority and managed pane bindings.
- `retired`: agent is no longer configured; history remains as residue/audit
  evidence.

Rules:

- New jobs targeting a draining or retired agent are rejected with a stable
  configured-agent error.
- Busy unload must have a timeout and a user-visible status.
- Pending unload records must have a queue bound and age bound.
- Force unload must be explicit and must record that it may interrupt provider
  work.
- `.ccb/agents/<agent>` is preserved unless a separate cleanup command removes
  safe residue.

Phase 4 landed only the bounded state foundation:

- pending unloads are represented as drain intents with initial
  `pending_unload`, then pure transitions to `draining`, `retiring`, or
  terminal `retired`/`timed_out`;
- bounds are `max_pending`, `timeout_s`, and `max_age_s`;
- busy/idle is provided by an injectable predicate, not by a direct comms or
  provider-execution dependency;
- `retired` is a terminal state marker only in Phase 4 and does not remove
  panes, mutate runtime authority, or publish a new graph.

## Replace Model

Provider, workspace mode, model, key, URL, and runtime-home changes are replace
operations, not simple metadata updates.

Idle replacement:

1. Mark old runtime `retiring`.
2. Stop or detach the old managed pane according to provider policy.
3. Advance runtime authority epoch.
4. Mount the new runtime in the same logical slot.
5. Publish the new service graph and config signature.

Busy replacement:

1. Mark `pending_replace`.
2. Reject duplicate replace requests for the same slot unless they supersede
   the pending plan within the configured bound.
3. Wait for the current work to finish or timeout.
4. Execute the idle replacement path.

Rules:

- Replacement must never claim provider session continuity unless
  provider-specific resume authority proves it.
- Codex and Claude session-boundary checks must stay provider-specific.
- Pending replacement must not block unrelated additive reloads forever.
- Pending replacement must have a maximum age and maximum queue length.

Phase 4 represents replacement as `pending_replace` drain intent records sharing
the same bounded queue as unload. It does not yet execute the replacement path
or supersede duplicate replacement requests.

## Failure Handling

Failures must be explicit and recoverable:

- `reload_rejected_busy_agent`: busy agent cannot drain within policy.
- `reload_rejected_pending_limit`: too many pending unload/replace operations.
- `reload_rejected_stale_plan`: config changed since dry-run or plan creation.
- `reload_patch_failed`: namespace mutation failed before publish.
- `reload_publish_failed`: service graph or signature publish failed.

If mutation fails before publishing the new graph, the old graph remains
current. If mutation succeeds but publish fails, the daemon must record a
repairable degraded reload event and refuse additional reload until repair or
restart.

## Project View Behavior

Project view should distinguish:

- configured active agent;
- draining configured agent;
- pending replace;
- retired residue.

Retired agents should not count as configured agents. Whether they remain
visible briefly for audit is an open question in
[../open-questions.md](../open-questions.md).
