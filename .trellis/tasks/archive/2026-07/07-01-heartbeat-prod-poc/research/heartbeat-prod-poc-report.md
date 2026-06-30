# Heartbeat PyO3 Production PoC Report

## Scope

Validate that `ccb-py-heartbeat` (Rust PyO3 extension) can replace the Python
`heartbeat` implementation used by the production CCB instance at
`/home/agnitum/o13` without behavioral drift, and measure the runtime impact.

## What was staged

| File | Purpose |
|------|---------|
| `/root/.local/share/codex-dual/lib/ccb_py_heartbeat.cpython-311-x86_64-linux-gnu.so` | Rust extension module (release build from `ccb-legacy`) |
| `/root/.local/share/codex-dual/lib/heartbeat/__init__.py` | Env-gated shim that routes `evaluate_heartbeat` to Rust when `CCB_HEARTBEAT_RUST=1` |
| `/root/.local/share/codex-dual/lib/heartbeat/__init__.py.python-backup` | Backup of the original `__init__.py` for one-command rollback |

Rollback command:

```bash
cp /root/.local/share/codex-dual/lib/heartbeat/__init__.py.python-backup \
   /root/.local/share/codex-dual/lib/heartbeat/__init__.py
```

## Non-disruptive correctness check

Ran the same 20 state-transition cases through both Python and Rust backends
using the production Python path (`/root/.local/share/codex-dual/lib`) and the
same `heartbeat` API that `ccbd` uses.

```text
All 20 cases matched between Python and Rust.
```

A summary of the exercised paths:

- `IDLE` before silence threshold
- `ENTER` when silence threshold crossed
- `REPEAT` when repeat interval elapsed
- `RESET` when progress advances
- max-notice-count limit -> `IDLE`
- states with/without prior `HeartbeatState`

The Rust shim converts Rust results back to the original Python dataclasses, so
consumers such as `job_heartbeat_runtime/tick.py` continue to use
`decision.action.value`, `state.to_record()`, etc.

## Memory / CPU microbenchmark

Measured in isolated Python 3.11 subprocesses using the production library path.

### Initial shim (JSON round-trip conversion)

| Metric | Python backend | Rust backend | Delta |
|--------|---------------|--------------|-------|
| Import RSS | 23,212 kB | 23,000 kB | -212 kB (within noise) |
| Per-tick wall time | 5,378 ns | 11,034 ns | +5,656 ns (+105%) |

### Optimized shim (direct attribute conversion)

| Metric | Python backend | Rust backend | Delta |
|--------|---------------|--------------|-------|
| Import RSS | 23,204 kB | 22,940 kB | -264 kB (within noise) |
| Per-tick wall time | 5,551 ns | 4,169 ns | -1,382 ns (-24.9%) |

The optimization removed the `to_record()` / `from_record()` JSON round-trip
for state and policy objects. The Rust backend is now measurably faster than the
Python backend while remaining behaviorally identical.

## Live `ccbd` observation

`ccbd` was restarted and is now running the Rust heartbeat backend. The Rust
shared object is loaded in the `ccbd` main process.

| Time (local) | ccbd PID | ccbd RSS (kB) | keeper RSS (kB) |
|--------------|----------|---------------|-----------------|
| 2026-07-01T02:31:55+08:00 | 546528 | 57,816 | 37,748 |
| 2026-07-01T02:32:55+08:00 | 546528 | 57,916 | 37,748 |
| 2026-07-01T02:33:55+08:00 | 546528 | 57,932 | 37,748 |
| 2026-07-01T02:34:55+08:00 | 546528 | 57,936 | 37,748 |
| 2026-07-01T02:35:55+08:00 | 546528 | 57,940 | 37,748 |
| 2026-07-01T02:36:55+08:00 | 546528 | 57,948 | 37,748 |
| 2026-07-01T02:37:55+08:00 | 546528 | 57,952 | 37,748 |
| 2026-07-01T02:38:55+08:00 | 546528 | 57,952 | 37,748 |
| 2026-07-01T02:39:55+08:00 | 546528 | 57,956 | 37,748 |
| 2026-07-01T02:40:55+08:00 | 546528 | 57,988 | 37,748 |

Observation window: ~9 minutes (user stopped the 20-minute sampler early).

- ccbd RSS drift: +172 kB over 9 minutes (effectively flat).
- keeper RSS: unchanged.
- `ccb ping ccbd`: healthy / mounted / desired_state=running.
- No `HeartbeatError`, `ValueError`, `AttributeError`, or `ccb_py_heartbeat`
  mentions in `ccbd.stderr.log`, `lifecycle.jsonl`, or `supervision.jsonl`.

## Rollback test

Verified that restoring the original `heartbeat/__init__.py` returns the module
to the pure-Python implementation and that `import heartbeat` succeeds without
errors. The rollback script is at:

```bash
/home/agnitum/ccb/.trellis/tasks/07-01-heartbeat-prod-poc/research/rollback_heartbeat_python.sh
```

## Risk items

1. **PR #237 exists prematurely**: It was opened before the PoC rule was
   clarified. It should remain draft/experimental until the user approves the
   PoC.
2. **Live observation was shortened**: The user stopped the sampler at ~9
   minutes instead of the planned 10–30 minutes. The trend is stable, but a
   longer window would increase confidence.

## Recommendation

The optimized Rust backend passes the production PoC:

- Behavior: identical decisions on 20 representative cases.
- Memory: no meaningful change in live ccbd (~+172 kB over 9 min, within noise).
- CPU: ~25% faster per tick in microbenchmarks.
- Stability: no heartbeat-related errors during live observation.

## Post-approval actions

User approved the PoC on 2026-07-01. The production-ready shim was committed to
`ccb-git` and pushed to the PR branch:

- Branch: `feat/rust-py-subsystems`
- Commit: `4ae5bc66`
- Files changed:
  - `lib/heartbeat/__init__.py` — env-gated Rust shim with dev fallback
  - `test/test_heartbeat_shim.py` — backend selection tests
- All heartbeat tests pass with both Python and Rust backends.

The production `ccbd` instance at `/home/agnitum/o13` remains on the Rust
heartbeat backend and is healthy. Rollback is still available via:

```bash
bash /home/agnitum/ccb/.trellis/tasks/07-01-heartbeat-prod-poc/research/rollback_heartbeat_python.sh
```
