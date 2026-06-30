# PRD: Heartbeat PyO3 Production PoC

## Goal

Validate that the Rust `ccb-py-heartbeat` extension module can replace the
in-process Python `lib/heartbeat/` implementation in the production CCB instance
without regressing behavior, and measure the memory/CPU impact.

## Scope

1. Build a release-quality `ccb_py_heartbeat` shared library from `ccb-legacy`.
2. Stage it into the production Python import path for the running CCB v8.0.4
   instance (`/home/agnitum/o13`).
3. Switch `lib/ccbd/services/job_heartbeat_runtime/tick.py` (or the narrowest
   possible call site) to import from `ccb_py_heartbeat` instead of
   `heartbeat`.
4. Run the PoC for a bounded observation window.
5. Capture:
   - `ccb` / `ccbd` / `keeper` RSS before and after
   - per-tick CPU cost (wall time if measurable)
   - any exceptions or behavioral drift in heartbeat-driven events
6. Provide a one-command rollback path.

## Out of scope

- Replacing `mailbox_kernel` or `message_bureau` (separate PoCs).
- Replacing `ccbd` / `keeper` / `ccb.py` themselves.
- Changing heartbeat policy semantics.
- Windows/WSL or other non-production environments.

## Acceptance criteria

- [ ] `ccb_py_heartbeat` imports and runs in production Python without import-time
      or tick-time errors.
- [ ] `JobHeartbeatService.tick()` produces the same `HeartbeatAction` decisions
      for the same inputs as the Python implementation.
- [ ] No new exceptions in `ccbd` logs during the observation window.
- [ ] Memory/CPU delta is recorded, even if neutral or negative.
- [ ] Rollback was tested and restores the Python implementation.
- [ ] User reviews the evidence and explicitly approves or rejects further rollout.

## Constraints

- Must not break production ask/reply/completion flows.
- Must be revertible in under 60 seconds.
- Must not require restarting `ccbd` if possible; hot-reload the module via
  Python import swap.
- Must not modify files in `/home/agnitum/ccb-git` directly unless those changes
  are intended to be committed later.

## Rollback trigger conditions

Rollback immediately if any of the following occurs:

- `ccbd` log shows `HeartbeatError`, `ValueError`, or `AttributeError` from
  `ccb_py_heartbeat`.
- Memory usage increases by >20 MB sustained.
- CPU usage per tick increases by >20% sustained.
- Any agent fails to receive a reply or job completion within normal latency.

## References

- `/home/agnitum/ccb/ccb-legacy/rust/crates/ccb-py-heartbeat/`
- `/home/agnitum/ccb-git/lib/heartbeat/`
- `/home/agnitum/ccb-git/lib/ccbd/services/job_heartbeat_runtime/tick.py`
- `/home/agnitum/ccb-git/lib/ccbd/services/job_heartbeat.py`
