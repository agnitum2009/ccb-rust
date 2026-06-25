# Wave 5: daemon restart job continuity

## Goal

Daemon restart must reload persisted running jobs into execution/heartbeat so trace shows them and heartbeat can drive them to completion.

## Requirements

- Persist `Running` job state (job id, agent, message, attempt) before daemon shutdown.
- On daemon startup, reload persisted running jobs into `JobDispatcher` / `ExecutionService`.
- Heartbeat must continue polling reloaded jobs and drive them to terminal completion.
- Reply events already persisted in mailbox must remain valid and route correctly.

## Acceptance Criteria

- [ ] `ccbr trace <agent>` shows a running job immediately after `ccbr shutdown` + restart ccbrd + `ccbr start`.
- [ ] The job eventually reaches `completed`/`failed`/`cancelled` without being re-submitted.
- [ ] Unit/integration tests cover reload path; `cargo test -p ccbr-daemon` passes.
- [ ] Live evidence captured in task `research/`.

## Notes

- Python reference: `lib/ccbd/services/dispatcher_runtime/` reload/restore concepts, `test_v2_daemon_startup_wait.py`.
- Rust target: `crates/ccbr-daemon/src/app.rs` startup, `crates/ccbr-jobs` store, `crates/ccbr-daemon/src/services/dispatcher.rs`.
- Part of Wave 5 parity audit gap #1.

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.
