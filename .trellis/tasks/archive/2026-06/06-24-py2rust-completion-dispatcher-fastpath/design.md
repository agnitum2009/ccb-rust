# Design: completion dispatcher / fastpath parity

## 1. ccb-completion tracker integration into ccb-daemon

### Current state

`CcbdApp::heartbeat` (in `rust/crates/ccb-daemon/src/app.rs`) already:

1. Ticks the dispatcher.
2. Feeds pane text into active execution submissions.
3. Polls `ExecutionService`, which returns `Vec<ExecutionUpdate>`.
4. Maps `update.decision.status` directly to daemon `JobStatus`.
5. Persists terminal decisions to the mailbox.

There is no `CompletionTrackerService` in the daemon yet.

### Target flow

```
Heartbeat tick
  ├─ dispatcher.tick() → newly running jobs
  │    └─ start CompletionTracker for each running job without a tracker
  ├─ feed_active_pane_text_to_execution()
  ├─ execution.poll() → Vec<ExecutionUpdate>
  │    ├─ for each item in update.items: tracker.ingest(job_id, item)
  │    ├─ tracker.tick(job_id, now)
  │    └─ read tracker.current(job_id) → CompletionTrackerView
  ├─ effective_decision = tracker_decision if terminal
  │                    else update.decision if terminal
  │                    else none
  ├─ map CompletionStatus → daemon JobStatus
  ├─ dispatcher.update_job_status(job_id, status, decision_record)
  └─ if terminal: mailbox.record_terminal(...) + tracker.finish(job_id)
```

### New types / conversions

- Add `completion_tracker: CompletionTrackerService<ccb_provider_core::catalog::ProviderCatalog>`
  to `CcbdApp`.
- Build the catalog with
  `ccb_provider_core::catalog::build_default_provider_catalog(false, false)`.
- Add a small adapter module `rust/crates/ccb-daemon/src/adapters/completion.rs`
  to convert a daemon `JobRecord` into a `ccb_completion::models::JobRecord`
  (job_id, agent_name, provider, request body/message_type, provider_options).
- Add `JobDispatcher::complete` (or a `ReplyDeliveryDispatcher` trait) that:
  - updates the job status,
  - stores the terminal decision,
  - returns a `&JobRecord`.

### Tracker lifecycle

- **Start:** after `dispatcher.tick()` promotes a job to `Running`, call
  `tracker.start(completion_job, started_at)`. Also start trackers for any
  already-running jobs on heartbeat that do not yet have one (restore path).
- **Ingest/tick:** for every `ExecutionUpdate` for a tracked job, feed items in
  order, then call `tick`. Read the current view.
- **Finish:** when a terminal decision is applied (from tracker or from the
  execution adapter), call `tracker.finish(job_id)`.

### Decision precedence

1. Tracker terminal decision (highest priority — the orchestrator has settled).
2. Execution adapter terminal decision (fallback when tracker has not yet
   converged).
3. No terminal decision → job remains `Running`.

## 2. Message bureau fastpath tests

The production code in `ccb-mailbox/src/facade_recording.rs` already matches
Python:

- `record_submission` writes an attempt + inbound event and calls
  `mailbox_kernel.apply_incremental_summary_update(agent, +1, 0, ...)`,
  which leaves the mailbox `Blocked` when `queue_depth > 0`.
- `record_retry_attempt` adds another inbound event with `+1, 0`.
- `record_reply` only calls `queue_reply_delivery` when
  `mailbox_actor(state, &job.request.from_actor)` resolves to a known mailbox;
  `user`/`system`/etc. return `None`.

Add one new integration test in `rust/crates/ccb-mailbox/tests/integration.rs`
that mirrors `test_record_submission_does_not_refresh_mailbox`.

## 3. Reply-delivery start-completion

Implement the Python `start_completion.py` logic in:

- `rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery_runtime/start_completion.rs`
- `rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery_runtime/decisions.rs`

### Signature sketch

```rust
pub fn complete_reply_delivery_after_start<D>(
    dispatcher: &mut D,
    job: &JobRecord,
    started_at: &str,
    submission: Option<&ProviderSubmission>,
) -> Option<&JobRecord>
where
    D: ReplyDeliveryDispatcher,
{
    // ...
}
```

`ReplyDeliveryDispatcher` trait:

```rust
pub trait ReplyDeliveryDispatcher {
    fn complete(
        &mut self,
        job_id: &str,
        decision: CompletionDecision,
    ) -> Option<&JobRecord>;
}
```

Implement it for `JobDispatcher` using `update_job_status`.

### Behavior

1. If `submission` is `None` or `runtime_state.mode` is `"error"`/`"passive"`:
   call `dispatcher.complete(job_id, reply_delivery_failed_decision(...))`.
2. If `runtime_state.reply_delivery_complete_on_dispatch` is `true`:
   return `Some(job)` without calling `dispatcher.complete`.
3. Otherwise call `dispatcher.complete(job_id, reply_delivery_completed_decision(...))`
   with `provider_turn_ref` derived from `request_anchor`, `pane_id`, or job id.

### Tests

- Unit test in `start_completion.rs` mirroring
  `test_reply_delivery_start_completion.py` (defer-on-dispatch case).
- Additional unit tests for the error/passive path and the active completion path.
