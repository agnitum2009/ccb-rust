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
