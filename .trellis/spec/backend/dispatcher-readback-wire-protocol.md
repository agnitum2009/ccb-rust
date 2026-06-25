# Dispatcher Readback Wire Protocol

## Scenario: Python clients inspect and control dispatcher jobs

### 1. Scope / Trigger

- Trigger: any change to `get`, `watch`, `queue`, `trace`, `cancel`, `resubmit`, `retry`, dispatcher job records, visible replies, or mailbox trace integration.
- Reference owner: Python `backup/python-reference/lib/ccbd/handlers/{get,watch,queue,trace,cancel,resubmit,retry}.py` and `ccbd/services/dispatcher_runtime/**`.
- Runtime owner: Rust `JobDispatcher`, mailbox control/facade, and daemon handlers.

### 2. Signatures

- `get`: `{ "job_id": "job_x" }` or `{ "agent_name": "agent1" }`.
- `watch`: `{ "target": "agent1|job_x", "cursor": 0 }`.
- `queue`: `{ "target": "all|agent1", "detail": true|false|null }`.
- `trace`: `{ "target": "job_x|msg_x|att_x|rep_x|sub_x|all|agent1" }`.
- `cancel`: `{ "job_id": "job_x" }`.
- `resubmit`: `{ "message_id": "msg_x" }`.
- `retry`: `{ "target": "attempt/job/message id" }`.

### 3. Contracts

- `get` must fail with `job not found` for unknown jobs/agents; it must not return `status: unknown`.
- `get` response includes Python-visible fields: `job_id`, `agent_name`, `target_kind`, `target_name`, `provider_instance`, `provider`, `status`, `job`, `snapshot`, `reply`, `completion_reason`, `completion_confidence`, `updated_at`, `visible_reply_source`, `visible_reply_id`, `message_id`, and daemon `generation`.
- `watch` consumes the Python `cursor` field and rejects negative cursors.
- `watch` may also accept legacy Rust `start_line` as a compatibility alias.
- `queue` should use mailbox-control read models, with dispatcher runtime active-job fields layered where needed.
- `trace` must use mailbox-control trace only. Python rejects `all` / agent-name trace targets; Rust must not return a dispatcher-local fallback for the Python `trace` op.
- `cancel` must interrupt active provider panes best-effort, terminalize dispatcher state, and record mailbox terminal state.
- `resubmit` must resolve the original message from the message bureau, require terminal latest attempts for all target agents, enqueue fresh dispatcher jobs, record a new message with `origin_message_id`, and return `accepted_at`, `original_message_id`, new `message_id`, `submission_id`, and accepted job receipts.
- `retry` must resolve target as Python does: first `attempt_id`, then `job_id`. It must reject active attempts, reject completed attempts, require the latest attempt for the same message/agent, enqueue one retry job, record a retry attempt, and return `accepted_at`, `target`, `message_id`, `original_attempt_id`, `attempt_id`, `job_id`, `agent_name`, and `status`.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|-----------|-------------------|
| `get` missing both `job_id` and `agent_name` | Error contains `get requires job_id or agent_name` |
| `get` unknown id | Error contains `job not found` |
| `get` terminal job with reply | Response includes visible reply and terminal completion fields |
| `watch` missing target | Error contains `watch requires target` |
| `watch` negative cursor | Error contains `watch cursor cannot be negative` |
| `watch` cursor beyond current lines | Empty lines and cursor preserved |
| `queue` concrete agent | Response includes actual queue depth and active job id |
| `trace` concrete job/mailbox id | Response resolves the concrete trace owner |
| `trace` `all` or agent-name target | Error contains `trace requires <submission_id|message_id|attempt_id|reply_id|job_id>` |
| `trace` missing concrete job id | Error contains `job not found in message bureau` |
| `cancel` running job | Job becomes cancelled and mailbox trace records terminal cancellation |
| `resubmit` unknown message | Error contains `message not found` |
| `resubmit` active latest attempt | Error contains `message still has active attempts` |
| `retry` missing target | Error contains `retry requires target` |
| `retry` active attempt | Error contains `attempt is still active` |
| `retry` failed latest attempt | Response includes new attempt/job ids and Python lifecycle fields |

### 5. Good / Base / Bad Cases

- Good: Python CLI asks `get job_x` and sees a structured job record plus visible reply, not a Rust-only summary.
- Base: `watch` with `cursor=1` resumes from line 1 and returns a stable cursor.
- Bad: `watch` ignores `cursor` because the Rust handler only reads `start_line`.

### 6. Tests Required

- Unit: `handlers::get::tests`.
- Unit: `handlers::watch::tests`.
- Unit: `handlers::resubmit::tests`.
- Unit: `handlers::retry::tests`.
- Unit: `handlers::trace::tests`.
- Integration: `test_watch_returns_activity_lines_for_target`.
- Integration: `test_queue_returns_actual_per_agent_state`, `test_trace_returns_job_history`, `test_cancel_updates_mailbox_state`.
- Package: `cargo test -p ccbr-mailbox trace -- --test-threads=1`.
- Package: `cargo test -p ccbr-daemon -- --test-threads=1`.
