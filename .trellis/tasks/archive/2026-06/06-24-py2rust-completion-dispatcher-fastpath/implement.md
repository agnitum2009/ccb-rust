# Implementation Plan: completion dispatcher / fastpath parity

## Phase 1 — Scaffold completion tracker wiring

1. `rust/crates/ccb-daemon/src/adapters/completion.rs`
   - Add `to_completion_job_record(job: &daemon::JobRecord) -> ccb_completion::JobRecord`.
2. `rust/crates/ccb-daemon/src/services/dispatcher.rs`
   - Add `pub fn complete(&mut self, job_id: &str, decision: CompletionDecision) -> Option<&JobRecord>`
     (maps `CompletionStatus` to daemon `JobStatus`, calls `update_job_status`,
     returns the updated job reference).
3. `rust/crates/ccb-daemon/src/app.rs`
   - Add `completion_tracker: CompletionTrackerService<ProviderCatalog>` to `CcbdApp`.
   - Build the catalog + tracker in `with_backend`.
   - In `heartbeat`:
     - Start trackers for newly-running jobs after `dispatcher.tick()`.
     - For each `ExecutionUpdate`, ingest items, tick, read view.
     - Choose effective terminal decision (tracker first, then adapter).
     - Update dispatcher, persist to mailbox, finish tracker on terminal.

## Phase 2 — Completion tracker tests

- Add `rust/crates/ccb-daemon/tests/completion_dispatcher_integration_tests.rs`:
  - Fake `ExecutionAdapter` that emits `CompletionItem`s but no decision.
  - Assert `heartbeat` drives the dispatcher job to `Completed` via the tracker.
  - Assert terminal decision is persisted to mailbox.
- Add unit tests in `rust/crates/ccb-daemon/src/services/dispatcher.rs` for
  `complete`.

## Phase 3 — Message bureau fastpath test

- `rust/crates/ccb-mailbox/tests/integration.rs`
  - Add `test_record_submission_does_not_refresh_mailbox`.
  - Assert attempt/event counts, `mailbox.queue_depth == 1`,
    `pending_reply_count == 0`, `mailbox_state == "blocked"`,
    `agent_queue["queue_depth"] == 1`, `agent_queue["mailbox_state"] == "blocked"`.

## Phase 4 — Reply-delivery start-completion

- `rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery_runtime/decisions.rs`
  - Implement `reply_delivery_completed_decision`.
  - Implement `reply_delivery_failed_decision`.
- `rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery_runtime/start_completion.rs`
  - Implement `complete_reply_delivery_after_start`.
  - Define `ReplyDeliveryDispatcher` trait.
  - Implement `ReplyDeliveryDispatcher` for `JobDispatcher`.
- Add inline `#[cfg(test)]` tests in `start_completion.rs`:
  - defer when `reply_delivery_complete_on_dispatch == true`.
  - failed decision when submission mode is `error`/`passive`.
  - completed decision for normal active mode.

## Phase 5 — Verification & matrix update

1. Run targeted tests:
   ```bash
   cargo test -p ccb-completion -p ccb-jobs -p ccb-mailbox -p ccb-daemon -- --test-threads=1
   ```
2. Run formatter:
   ```bash
   cargo fmt --all -- --check
   ```
3. Run clippy:
   ```bash
   cargo clippy -p ccb-completion -p ccb-jobs -p ccb-mailbox -p ccb-daemon --tests
   ```
4. Update `plans/rust-python-test-parity-matrix.md`:
   - In the `completion` row, add mappings for the new
     `ccb-daemon/tests/completion_dispatcher_integration_tests.rs` and
     `reply_delivery_start_completion_tests.rs` tests.
   - Note that per-provider adapter parity remains deferred.
   - Update `daemon_lifecycle` or add a focused `reply_delivery` note as
     appropriate.

## Stop / escalation boundaries

- Do not modify the ccbd socket protocol or mailbox kernel contracts.
- Do not change tmux namespace / pane identity logic.
- Do not expand into per-provider execution adapter implementations.
