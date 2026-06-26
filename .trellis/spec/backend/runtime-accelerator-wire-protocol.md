# Runtime Accelerator Wire Protocol

## Scenario: ccb-legacy runtime accelerator sidecar RPC

### 1. Scope / Trigger

- Trigger: changes to `ccb-runtime-accelerator` RPC methods, request/response fields, or Python fallback boundaries.
- Owner: `ccb-legacy` Python `.ccb` runtime accelerator, not `ccbrd` and not `.ccbr` state.
- Hard rule: do not disable, remove, skip, or mask Codex hooks for performance.

### 2. Signatures

- Transport: Unix domain socket.
- Framing: one JSON request per line; one JSON response per line.
- Request envelope: `{"method": string, "params": object}`.
- Response envelope: `{"ok": true, "result": object}` or `{"ok": false, "error": string}`.
- Methods:
  - `ping`
  - `capabilities`
  - `baseline_snapshot`
  - `codex_observe`
- Environment keys:
  - `CCB_RUNTIME_ACCELERATOR_CODEX`: Codex polling adapter gate. Default unset/enabled. Disable values: `0`, `false`, `no`, `off`, `disabled`.
  - `CCB_RUNTIME_ACCELERATOR_SOCKET`: optional Unix socket override. Default: `<workspace>/.ccb/runtime-accelerator/accelerator.sock`.
  - `CCB_RUNTIME_ACCELERATOR_TIMEOUT_S`: optional sidecar call timeout in seconds. Default: `0.2`.
  - `CCB_RUNTIME_ACCELERATOR_BIN`: optional sidecar binary override. Default lookup: `ccb-runtime-accelerator` on `PATH`, installed runtime `bin/`, then repo-local `rust/target/{release,debug}`.
  - `CCB_RUNTIME_ACCELERATOR_STARTUP_TIMEOUT_S`: optional ccbd sidecar startup wait in seconds. Default: `0.5`.
  - `CCB_CCBD_IDLE_FULL_HEARTBEAT_INTERVAL_S`: optional idle full-maintenance interval. Default: `30.0`.
  - `CCB_CCBD_HEARTBEAT_WRITE_INTERVAL_S`: optional lease heartbeat write debounce interval. Default: `5.0`; `0` restores every-tick writes.
  - `CCB_KEEPER_STATE_WRITE_INTERVAL_S`: optional keeper state write debounce interval for `last_check_at`-only updates. Default: `5.0`; `0` restores every-tick keeper state writes.
  - `CCB_PROJECT_VIEW_IDLE_TTL_MS`: optional ProjectView cache TTL when no dispatcher work or busy agent state is pending. Default: `5000`; `1000` restores the legacy 1s idle sidebar cadence.
  - `CCB_PROJECT_VIEW_TTL_MS`: optional ProjectView cache TTL when active work is pending. Default: `1000`.
  - `CCB_BRIDGE_IDLE_SLEEP`: Codex bridge FIFO wait timeout. Default: `1.0`; legacy `0.05` may be set explicitly for diagnostics, but default bridge idle wait must not hot-poll.
  - `CCB_CODEX_BIND_POLL_INTERVAL`: Codex binding/session-follow poll interval. Default: `5.0`; lower values are diagnostic/latency overrides, not the idle default.

### 3. Contracts

- `capabilities.hot_loop_replacement_active=true` once Python daemon/provider integration default-enables Codex active polling delegation.
- Python Codex polling calls `codex_observe` only after normal active-submission preparation succeeds.
- Python owns active-job selection. The adapter sends exactly the current active job descriptor; the sidecar must not discover or scan idle agents.
- When `CCB_RUNTIME_ACCELERATOR_CODEX` is explicitly disabled, Codex polling must use the existing Python reader path.
- When the sidecar communication fails, the observation is malformed, a per-job observation has `error`, or an item kind is unknown, Codex polling must fall back to the existing Python reader path.
- When the sidecar succeeds with no new items and no reader-state change, Codex polling must return no update without calling the Python reader fallback. Otherwise the opt-in path still pays the old polling cost.
- Python `ccbd` starts the sidecar by default unless `CCB_RUNTIME_ACCELERATOR_CODEX` is explicitly disabled.
- Sidecar lifecycle startup failure is non-fatal: `ccbd` continues and provider polling uses Python fallback.
- `ccbd` may unlink the accelerator socket only for a sidecar process it started. Missing-binary/fallback handles must not delete a manually supplied socket.
- Python `ccbd` idle heartbeat refreshes the lease but skips full health/supervision/dispatcher maintenance until active work appears or the idle full-maintenance interval elapses.
- Python `ccbd` must still validate the current lease holder on every heartbeat tick, but may skip rewriting the lease file until the heartbeat write interval elapses.
- Python keeper must not rewrite `lifecycle.json` for an already-mounted lifecycle when owner/socket/config/namespace fields are unchanged.
- Python keeper may skip `keeper.json` rewrites when only `last_check_at` changes before the keeper state write interval elapses.
- Python ProjectView responses may use a longer cache TTL only while dispatcher queues/active jobs and busy agent states are absent; active work stays on the short TTL.
- Python ProjectView cache freshness must be automatic: dispatcher job/event mutations increment an in-memory revision, and cached ProjectView responses are reusable only while that revision is unchanged. TTL values are rollback/review safety windows, not user-tuned correctness controls.
- Python runtime registry must not rewrite per-agent runtime JSON when the only changed field is `last_seen_at`; freshness can update in memory, while the next material runtime state change persists the freshest timestamp.
- Python Codex bridge keeps the FIFO reader and ack/forwarding process alive, but defaults to event-waiting with a 1s idle timeout instead of 0.05s hot polling.
- The Python-compatible implementation baseline for bridge/runtime accelerator changes is `ccb-legacy`; Python latest can receive the patch only after legacy validation.
- Python Codex binding tracker keeps session-follow capability but defaults to a 5s idle interval instead of 0.5s repeated log/session scans.
- Python Codex binding tracker must not reparse unchanged ambiguous session sets on idle ticks; once a switched-unbound decision is recorded with no running job anchors, it may reuse a cheap session-root signature until files or the bound log mtime change.
- After session-switch detection finds no switchable candidate, Python Codex binding tracker must reuse the existing bound log path when it still exists; it must not perform a second workspace-wide `current_log_path` scan on the same idle tick.
- Python Codex binding tracker must also cache unchanged `bound` / no-new-candidate session roots; idle ticks must not rerun the full session-switch resolver until the bound log or session set signature changes.
- `codex_observe.params.jobs[]` fields:
  - `job_id`
  - `session_path`
  - `request_anchor`
  - `state.offset`, `state.next_seq`, `state.anchor_seen`, reply/binding fields
- `codex_observe` reads only explicit `session_path` values from active job descriptors.
- `codex_observe` returns per-job `state`, `items[]`, `reached_terminal`, and optional per-job `error`.
- A missing session file is a per-job error, not a whole-batch RPC failure.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
| --- | --- |
| Malformed envelope JSON | `ok=false` with `invalid request` |
| Unknown method | `ok=false` with `unknown method` |
| Missing Codex session file | `ok=true`; affected observation has `error` |
| Unset `CCB_RUNTIME_ACCELERATOR_CODEX` | Python uses Rust accelerator path when sidecar is available |
| Disabled `CCB_RUNTIME_ACCELERATOR_CODEX` | Python uses existing reader path |
| Sidecar unavailable or timeout | Python uses existing reader path |
| Sidecar binary missing at ccbd startup | startup continues; report action contains `runtime_accelerator_fallback:missing_binary` |
| ccbd started sidecar and shuts down | terminate sidecar and remove owned socket |
| ccbd did not start sidecar | do not remove socket path |
| Observation has per-job `error` | Python uses existing reader path |
| Successful observation with no changes | Python returns no provider update and does not invoke reader fallback |
| Idle `ccbd` heartbeat before idle interval | refresh lease only; skip full agent maintenance |
| Active execution or queued dispatcher work exists | run full `ccbd` maintenance |
| Mounted lease heartbeat before write interval | validate holder; return existing lease without JSON rewrite |
| `CCB_CCBD_HEARTBEAT_WRITE_INTERVAL_S=0` | write heartbeat on every tick |
| Stable mounted lifecycle tick | do not rewrite `lifecycle.json` |
| Keeper state update changes only `last_check_at` before write interval | do not rewrite `keeper.json` |
| `CCB_KEEPER_STATE_WRITE_INTERVAL_S=0` | write keeper state on every tick |
| ProjectView requested while idle | cache response for `CCB_PROJECT_VIEW_IDLE_TTL_MS` |
| ProjectView requested while active work exists | use `CCB_PROJECT_VIEW_TTL_MS` short TTL |
| Dispatcher job/event changes while ProjectView cache is still within TTL | rebuild ProjectView immediately; do not wait for TTL expiry |
| Stable full idle heartbeat changes only runtime `last_seen_at` | update in-memory runtime freshness without per-agent JSON rewrite |
| Later material runtime field changes after skipped freshness refreshes | persist the material change plus freshest in-memory `last_seen_at` |
| Unchanged ambiguous Codex session candidates on idle bridge tick | skip full session-switch resolver and preserve the existing switched-unbound diagnostic |
| Session-switch detection has no switchable candidate and current bound log exists | reuse bound log path; skip workspace-wide current-log scan |
| Bound Codex session root unchanged on idle bridge tick | skip full session-switch resolver until bound log or session-set signature changes |
| Assistant event before request anchor | no completion item |
| `task_complete.last_agent_message` exists | emit `turn_boundary` with cleaned final text |

### 5. Good / Base / Bad Cases

- Good: Python passes only running Codex jobs; sidecar emits `anchor_seen`, `assistant_chunk`, and `turn_boundary`.
- Base: sidecar is unavailable; Python fallback keeps existing polling behavior.
- Base: sidecar returns no new items; Python treats it as handled no-change, not as a reason to run the reader fallback.
- Base: sidecar lifecycle is enabled but binary is absent; `ccbd` starts normally and records fallback.
- Bad: sidecar scans all agents or idle sessions on its own.

### 6. Tests Required

- Unit: `codex_observe` emits anchor, assistant chunk, and task boundary from a fixture JSONL.
- Unit: assistant text before anchor does not emit completion items.
- Unit: missing session path stays per-job.
- Unit: `CCB_RUNTIME_ACCELERATOR_CODEX=0` does not call the sidecar.
- Unit: sidecar communication failure returns `None` so the Python reader fallback can run.
- Unit: successful no-change observation short-circuits Python reader fallback.
- Unit: sidecar lifecycle explicit-disable does not spawn a process.
- Unit: missing sidecar binary preserves fallback and does not remove a manually supplied socket.
- Unit: owned sidecar shutdown terminates the process and removes the owned socket.
- Unit: idle `ccbd` heartbeat skips heavy maintenance between full ticks.
- Unit: active execution still runs heavy maintenance.
- Unit: ProjectView cache invalidates immediately when dispatcher job/event revision changes, even if TTL has not expired.
- Unit: registry skips disk writes for `last_seen_at`-only runtime refreshes but persists the freshest timestamp with the next material change.
- Unit: stable full idle heartbeat reports zero runtime-store writes after one-time authority adoption has settled.
- Unit: repeated ambiguous Codex session candidates do not call the full switch resolver again until the session-root signature changes.
- Unit: bound Codex session refresh does not call workspace-wide current-log scanning after switch detection has already found no switch candidate.
- Unit: repeated bound/no-new-candidate Codex session roots do not call the full switch resolver again until the session-root signature changes.
- Smoke: real Unix socket request/response for `codex_observe`.

### 7. Wrong vs Correct

#### Wrong

```text
codex_observe scans .ccb/agents/* and polls idle sessions.
```

#### Correct

```text
Python owns active-job selection; sidecar reads only passed descriptors.
```

#### Wrong

```text
Enabled sidecar returns no changes, so polling falls through into the Python
reader path anyway.
```

#### Correct

```text
Enabled sidecar success is authoritative for that active poll tick. Empty
observations return no provider update and do not run the Python reader.
```
