# py2rust completion dispatcher / fastpath parity

## Goal

Close a focused slice of Rust parity for completion-orchestrator integration
into the daemon dispatcher and for reply-delivery / message-bureau fastpath
behaviors. This slice intentionally does **not** implement per-provider
execution adapters.

## In Scope

1. **ccb-completion orchestrator integration into ccb-daemon**
   - Ingest `ExecutionUpdate.items` as `CompletionItem`s into a daemon-wide
     `CompletionTrackerService`.
   - Use the tracker to produce terminal `CompletionDecision`s for running
     dispatcher jobs.
   - Map `CompletionStatus` back to daemon `JobStatus` and update the
     `JobDispatcher`.
   - Persist terminal decisions to `ccb-mailbox` via `record_terminal`.

2. **Message bureau fastpath parity**
   - Ensure Rust tests mirror Python `test_message_bureau_submission_fastpath.py`
     exactly:
     - `record_submission` creates one attempt + one inbound event and leaves
       the mailbox `blocked` without refreshing it.
     - `record_retry_attempt` increments queue depth and leaves the mailbox
       `blocked` without refreshing it.
     - `record_reply(..., deliver_to_caller=True)` skips creating a `user`
       mailbox entry for non-mailbox callers.
   - The implementation in `ccb-mailbox/src/facade_recording.rs` already
     behaves correctly; the work is test/assertion alignment.

3. **Reply-delivery start-completion parity**
   - Implement `complete_reply_delivery_after_start` in
     `ccb-daemon/src/services/dispatcher_runtime/reply_delivery_runtime/`.
   - When `submission.runtime_state["reply_delivery_complete_on_dispatch"]`
     is `true`, return the job unchanged and do **not** call
     `dispatcher.complete`.
   - Provide the helper decision constructors
     `reply_delivery_completed_decision` and `reply_delivery_failed_decision`.

## Out of Scope

- Per-provider execution adapter parity
  (`test_*_execution_polling.py`, provider-specific detection heuristics).
- Reply-delivery formatting, claims, preparation, terminal handling beyond
  `start_completion`.
- Changes to the ccbd control-plane protocol or socket interface.

## Acceptance Criteria

- [ ] `cargo test -p ccb-completion -p ccb-jobs -p ccb-mailbox -p ccb-daemon -- --test-threads=1` passes.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy -p ccb-completion -p ccb-jobs -p ccb-mailbox -p ccb-daemon --tests` introduces no new warnings.
- [ ] `plans/rust-python-test-parity-matrix.md` is updated with the new Rust test mappings.
- [ ] All new code follows existing crate conventions and includes focused
      unit/integration tests.
