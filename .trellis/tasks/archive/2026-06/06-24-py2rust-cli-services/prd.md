# py2rust-cli-services-parity

## Goal

Complete Rust/Python test parity for the CLI service layer so that the six
partial clusters in `plans/rust-python-test-parity-matrix.md` can be marked
`complete`:

- `ask_service`
- `kill_service`
- `kill_runtime_agent_cleanup`
- `cleanup_service`
- `doctor_runtime`
- `diagnostics_bundle`

## Confirmed Facts

- `cleanup_service`, `diagnostics_bundle`, and `kill_runtime_agent_cleanup` are
  already well covered by Rust tests; only small assertion/edge-case gaps
  remain.
- `doctor_runtime` has runtime-identity logic and tests, but the top-level
  `services/doctor.rs` orchestration is a stub.
- `kill_service` has the main orchestrator but several stage functions are
  stubs or lack integration tests (shutdown-intent record, PID snapshot
  ordering, shutdown report store, start-policy clearing, runtime PID file
  termination).
- `ask_service` has submit/guidance/sender coverage but is missing
  daemon-socket retry/reconnect during shutdown, watch reconnect/timeout, and
  persisted terminal-watch payload tests.
- Several planned changes touch the ccbd socket interface and daemon lifecycle,
  which `AGENTS.md` flags as escalation points.

## Requirements

1. Fill the small remaining gaps in `cleanup_service`,
   `diagnostics_bundle`, and `kill_runtime_agent_cleanup` and add the missing
   assertions/edge-case tests.
2. Implement the `services/doctor.rs` orchestration so the full doctor payload
   can be assembled and rendered, with tests matching
   `test_doctor_runtime_identity.py` behavior.
3. Implement the missing `kill_service` stage functions and add integration
   tests for shutdown ordering, report persistence, start-policy clearing, and
   runtime PID termination.
4. For `ask_service`, add tests and implementation only for the
   non-socket-protocol surfaces (legacy role alias, output/exit-code, sender
   resolution, relocated runtime actor). Any work that changes the daemon
   socket retry/reconnect contract or watch endpoint must be escalated before
   implementation.
5. Update `plans/rust-python-test-parity-matrix.md` to mark the six clusters
   `complete` and document remaining out-of-scope items.

## Acceptance Criteria

- [ ] `cargo test -p ccb-cli -- --test-threads=1` passes with new/expanded tests
      for all six clusters.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `plans/rust-python-test-parity-matrix.md` shows the six target clusters
      as `complete`.
- [ ] No changes to the ccbd control-plane socket protocol, mailbox kernel
      submit envelope, or tmux namespace/pane identity logic unless explicitly
      escalated and approved.
- [ ] If a blocker requires changing an escalation-guarded surface, the task
      stops and reports the blocker instead of forcing a pass.

## Out of Scope

- Live provider CLI integration tests (intentionally mocked in Rust).
- Changes to the ccbd socket protocol or mailbox kernel contracts; these must
  be escalated.
- Windows/WSL-specific CLI bootstrap or path utilities.
- Refactoring unrelated CLI commands or provider backends.

## Open Questions / Blockers

- None at planning time; escalation will happen if implementation reveals a
  need to change daemon socket retry semantics or mailbox submit behavior.
