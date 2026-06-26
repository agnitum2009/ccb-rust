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

### 3. Contracts

- `capabilities.hot_loop_replacement_active=false` until Python daemon/provider integration actually delegates hot-loop work.
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
| Assistant event before request anchor | no completion item |
| `task_complete.last_agent_message` exists | emit `turn_boundary` with cleaned final text |

### 5. Good / Base / Bad Cases

- Good: Python passes only running Codex jobs; sidecar emits `anchor_seen`, `assistant_chunk`, and `turn_boundary`.
- Base: sidecar is unavailable; Python fallback keeps existing polling behavior.
- Bad: sidecar scans all agents or idle sessions on its own.

### 6. Tests Required

- Unit: `codex_observe` emits anchor, assistant chunk, and task boundary from a fixture JSONL.
- Unit: assistant text before anchor does not emit completion items.
- Unit: missing session path stays per-job.
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
