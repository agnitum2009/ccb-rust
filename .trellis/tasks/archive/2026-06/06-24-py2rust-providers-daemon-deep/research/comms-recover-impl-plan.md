# Implementation Plan — close `comms_recover` gap (1:1 with Python)

> Trellis artifact for task `06-24-py2rust-providers-daemon-deep`. Date: 2026-06-24.
> Follows `consistency-audit-daemon-lifecycle.md` §1 (CONFIRMED gap).

## 0. Scope reality (read first)

Python `comms_recover` hangs off the **rich `JobDispatcher`** (`lib/ccbd/services/dispatcher.py`).
The 12 `test_ccbd_comms_recover.py` cases exercise a full machine: `submit/tick/complete/cancel/
retry/get`, `_job_store`, `_message_bureau_control.{_inbound_store,_attempt_store,_lease_store,
_mailbox_kernel}`, `AttemptStore` `retry_index`/lineage, `prepare_reply_deliveries` (auto-creates
reply_delivery jobs on tick), and `ProjectViewService` comms-recoverable marking.

Rust split this into a **simplified `services/dispatcher.rs::JobDispatcher`** (placeholder
`comms_recover`/`retry`/`resubmit`) + real `submission_service.rs` + `ccb-mailbox` (real stores,
accessed via `app.mailbox_control`, NOT via the simplified JobDispatcher). The simplified
JobDispatcher lacks `complete`, real `cancel`, real `retry`, lineage, reply-delivery-job creation.

⇒ Faithful 1:1 closure requires **building the rich-dispatcher recovery/retry/cancel/lineage/
reply-delivery machinery in Rust first** — effectively D1–D3 depth. This is a **multi-session
sub-project**, not a single-turn placeholder swap.

## 1. Architecture decision (choose before coding)

- **Option A — enrich `services/dispatcher.rs::JobDispatcher`** to own the mailbox stores +
  real `submit/complete/cancel/retry/get` (mirror Python's rich JobDispatcher), then port
  `comms_recover.py` nearly verbatim. Pro: structural 1:1, tests port directly. Con: large
  refactor; duplicates/competes with the existing `app.mailbox_control` real path; divergence risk.
- **Option B — implement `comms_recover` against the real Rust path** (`app.mailbox_control` +
  `submission_service` + `ccb-mailbox` stores), achieving **behavioral** parity without mirroring
  Python's JobDispatcher structure. Pro: builds on the real, tested path. Con: test harness
  differs from Python (can't port tests verbatim); must map each Python store op to its
  ccb-mailbox equivalent.

**Recommendation: Option B** (behavioral parity on the real path), because the real
`app.mailbox_control`/`ccb-mailbox` path is already green-tested for submit/inbox/ack/trace and
owns the stores; duplicating it into the simplified JobDispatcher (Option A) creates divergence
risk. The 12 Python tests get re-expressed as Rust integration tests against `CcbdApp`/dispatcher
asserting the SAME observable outcomes (status/noop_reason/cancelled_old/retried_job/next_started/
attempt retry_index/mailbox states).

## 2. Dependency map (Python → Rust)

| Python symbol | Rust target (Option B) |
|---------------|------------------------|
| `dispatcher.get_job` | `app.dispatcher.get` / mailbox job store |
| `dispatcher.retry(target)` | **NEW** real retry → new job, `retry_index` bump, attempt record |
| `dispatcher.cancel(job_id, record_reply=False)` | **NEW** real cancel → CancelReceipt, status CANCELLED |
| `dispatcher.complete(job, decision)` | existing completion path (mailbox.record_terminal) |
| `_message_bureau_control._inbound_store` | `ccb-mailbox` inbound store |
| `_attempt_store` (retry_index, lineage) | `ccb-mailbox` attempt store (**may need retry_index field**) |
| `_lease_store`, `_mailbox_kernel` | `ccb-mailbox` lease + kernel |
| `prepare_reply_deliveries`, `is_reply_delivery_job`, `rewrite_reply_head`, `reply_delivery_inbound_event_id/reply_id` | `reply_delivery_runtime/*` (real) + `reply_delivery.rs` |
| `tick_jobs` | `dispatcher.tick` |
| `normalized_runtime_health`, `RECOVERABLE_RUNTIME_HEALTHS` | runtime_recovery_policy (check Rust equiv) |
| `ProjectViewService` comms marking | daemon project_view (out of comms_recover core; test 12) |

## 3. Phasing (each slice = TDD + commit + Trellis update)

- **Slice 1 — recoverability + noop paths.** Port `comms_recoverability_for_job`, `_audit`,
  `_clean_running_hint`, `_running_stale_reason`, `_audit_changed`, payload parsing, and the
  `comms_recover` main flow's noop branches. Covers tests 1 (healthy running → noop
  not_recoverable) and 5 (unknown hint → noop not_recoverable). No retry/cancel needed.
- **Slice 2 — terminal retry.** Implement real `retry` (new job + retry_index + attempt) +
  `_recover_terminal_retry` + lineage (`_lineage_for_job`, `_is_latest_attempt`, `_can_retry_job`,
  `_already_retried_job_id`, `_release_lineage_head_if_blocking`). Covers idempotency +
  terminal-recovery cases.
- **Slice 3 — stale-running recovery.** Implement real `cancel` + `_recover_stale_running`.
  Covers tests 2/3/4 (provider_prompt hints) and 6 (stale running + waiting jobs).
- **Slice 4 — reply-delivery recovery.** `_recover_reply_delivery` + reply-head rewrite +
  `prepare_reply_deliveries`. Covers tests 9/10/11 (reply-delivery race / failed / idempotent).
- **Slice 5 — mailbox-head release + project-view.** Test 8 (targeted head release) + test 12
  (project_view recoverable marking). Wire `recoverability_after` + project_view comms field.

## 4. Risks

- `ccb-mailbox` attempt store may lack `retry_index` / lineage-by-message APIs Python assumes →
  may require extending the mailbox crate (cross-crate change; check stop-rule re: mailbox kernel).
- Reply-delivery job auto-creation on tick (`prepare_reply_deliveries`) must exist in Rust tick
  path; verify before Slice 4.
- Two-dispatcher divergence (simplified `services/dispatcher.rs` vs real `app.mailbox_control`) —
  Option B keeps comms_recover on the real path; the simplified JobDispatcher's placeholder
  methods should eventually be removed or wired to the real path to avoid confusion.

## 5. Acceptance

- 12 `test_ccbd_comms_recover.py` behaviors reproduced as Rust tests (observable-outcome parity).
- `cargo test -p ccb-daemon -- --test-threads=1` green; clippy 0 errors; fmt clean.
- `plans/rust-python-test-parity-matrix.md` daemon_lifecycle row updated (comms_recover → complete).
