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
- `queue` and `trace` should prefer mailbox-control read models when bureau ids are supplied, with dispatcher runtime fields layered where needed.
- `cancel` must interrupt active provider panes best-effort, terminalize dispatcher state, and record mailbox terminal state.
- `resubmit` and `retry` must use message-bureau lineage before they can be marked fully closed; thin acknowledgement stubs are not enough for Python parity.

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
| `cancel` running job | Job becomes cancelled and mailbox trace records terminal cancellation |

### 5. Good / Base / Bad Cases

- Good: Python CLI asks `get job_x` and sees a structured job record plus visible reply, not a Rust-only summary.
- Base: `watch` with `cursor=1` resumes from line 1 and returns a stable cursor.
- Bad: `watch` ignores `cursor` because the Rust handler only reads `start_line`.

### 6. Tests Required

- Unit: `handlers::get::tests`.
- Unit: `handlers::watch::tests`.
- Integration: `test_watch_returns_activity_lines_for_target`.
- Package: `cargo test -p ccbr-daemon -- --test-threads=1`.

### 7. Pending surfaces

- `resubmit` needs full message-bureau resubmission payload parity:
  `accepted_at`, `original_message_id`, new `message_id`, `submission_id`, and `jobs`.
- `retry` needs full attempt-lineage validation/payload parity:
  `accepted_at`, `target`, `message_id`, `original_attempt_id`, `attempt_id`, `job_id`, `agent_name`, and `status`.
