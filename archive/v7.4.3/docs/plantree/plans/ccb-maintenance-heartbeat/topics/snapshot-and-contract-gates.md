# Snapshot And Contract Gates

Date: 2026-06-10

## Purpose

Capture the contract updates and read-path mapping required before implementing
the CCB maintenance heartbeat runtime.

## Namespace Rules

Use a dedicated namespace for maintenance heartbeat scheduling and status:

- `.ccb/ccbd/maintenance-heartbeat/`

Do not store maintenance heartbeat schedule or lock state under:

- `.ccb/ccbd/heartbeats/`

That existing namespace is diagnostics evidence for non-lease subject
heartbeats, such as running jobs. The implementation and contracts must keep
three concepts separate:

- daemon lease heartbeat;
- subject/job heartbeat diagnostics evidence;
- maintenance heartbeat scheduling, locks, follow-ups, and status.

## Required Contract Updates

P0 before runtime implementation:

- [../../../../ccb-config-layout-contract.md](../../../../ccb-config-layout-contract.md):
  define heartbeat config fields, defaults, validation, and reload behavior.
- [../../../../ccbd-diagnostics-contract.md](../../../../ccbd-diagnostics-contract.md):
  define `.ccb/ccbd/maintenance-heartbeat/`, `schedule.json`, lock/status
  files, support-bundle inclusion, and the three heartbeat concepts.

P1 before startup integration:

- [../../../../ccbd-startup-supervision-contract.md](../../../../ccbd-startup-supervision-contract.md):
  define startup ensure semantics, optional failure reporting, and interaction
  with `ccb kill`.

Consider a future standalone contract if the surface grows:

- `docs/ccb-maintenance-heartbeat-contract.md`

## Snapshot Read-Path Map

The runner should reuse existing diagnostics and communication read paths
where possible. Any required gap must be explicitly added to the diagnostics
contract before implementation.

| Snapshot field | Preferred source | Status |
| --- | --- | --- |
| Project anchor, config source, configured agents | effective config / `ccb ps` / doctor | Known surface |
| ccbd mounted/alive state, generation | `ccb ps`, `ccb ping ccbd`, lifecycle/lease diagnostics | Known surface |
| Agent provider, mounted/bound state, runtime health | `ccb ps` / project view diagnostics | Known surface |
| Queue depth and active job age | `ccb queue --detail` / queue observer | Known but degraded observer cases must remain visible |
| Pending inbox and pending reply counts | `ccb pend --inbox --detail`, queue/mailbox summary | Known but mailbox summary consistency may be incomplete |
| Callback or parent-waiting state | `ccb trace <id>` / job lineage diagnostics | Need exact bounded summary source |
| Recent terminal failures | trace, queue/job ledgers, lifecycle/diagnostics events | Need exact retention/window rule |
| Fault-injection rules | `ccb fault list` | Known surface |
| Mailbox summary consistency | diagnostics contract mailbox summary comparison | Contract says should; implementation status must be verified |
| Artifact references for large diagnostic payloads | `.ccb/ccbd/artifacts/text/` metadata | Known surface |
| Last heartbeat result and follow-up streaks | `.ccb/ccbd/maintenance-heartbeat/` | New contract required |

## V1 Design Defaults To Decide

- Built-in default: likely disabled unless project config enables heartbeat.
- Healthy interval: choose before implementation.
- Minimum interval: choose before implementation.
- Unknown cap: current planning default is three consecutive unknowns before
  user escalation/backoff.
- Startup ensure: v1 should refresh schedule state and run or arrange one-shot
  due ticks, not introduce a long-lived supervised runner without a startup
  lifecycle contract.

## Implementation Gate

Do not implement runtime writes for heartbeat schedule, locks, follow-ups, or
status until the config, diagnostics, and startup contracts above are updated
or the implementation is explicitly scoped as an isolated prototype.
