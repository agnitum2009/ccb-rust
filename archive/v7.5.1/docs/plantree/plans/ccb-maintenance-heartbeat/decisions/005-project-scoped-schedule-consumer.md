# Project-Scoped Schedule Consumer

Date: 2026-06-11

## Context

The landed heartbeat v1 can run a bounded one-shot tick, persist
`schedule.json`, dispatch a silent diagnostic activation to the configured
assessor, and let `ccb_self` request a later follow-up through
`ccb maintenance schedule`.

Real validation in `/home/bfly/yunwei/test_ccb2` showed the missing runtime
piece: after `ccb_self` updated `next_run_at`, no background consumer invoked
the next tick when the schedule became due. The one-shot runner and assessor
path work, but the persisted schedule is passive without a project-scoped
consumer.

## Decision

Add a CCB-owned project-scoped maintenance heartbeat schedule consumer helper.

The helper is started or ensured by normal `ccb` project startup when
`[maintenance.heartbeat].enabled = true`, `startup_ensure = true`, and the
configured assessor exists. It reads
`.ccb/ccbd/maintenance-heartbeat/schedule.json`, waits until the schedule is
due, and invokes the existing one-shot `ccb maintenance tick` service path.

The existing one-shot tick remains the only component that classifies health,
writes heartbeat status/schedule/activation records, and submits assessor
activations. The consumer owns only wake timing, singleton process state,
schedule consumption, and graceful exit.

The first slice should implement a project-local CCB helper, not automatic host
OS scheduler installation. It is outside provider context and must not become a
provider-side loop. It is also not a replacement for ccbd's normal configured
agent supervision.

## Consequences

- Startup ensure changes from "run one due tick only" to "ensure the schedule
  consumer, with one-shot tick as a fallback when runner start fails or is
  explicitly requested."
- `ccb kill` and project shutdown need a clear stop/exit rule for the helper.
- `ccb maintenance status` should expose runner state so users can distinguish
  "enabled but no consumer" from "consumer alive and waiting."
- The helper needs its own state record, stale-pid handling, bounded logs, and
  tests for singleton behavior.
- Host-level cron/systemd/launchd/Task Scheduler installation remains deferred
  until there is a cross-platform installer contract.
