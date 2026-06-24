# Consistency Audit — `daemon_lifecycle` cluster (Python ↔ Rust)

> Per author requirement: confirm Rust functional consistency vs Python source.
> Trellis artifact for task `06-24-py2rust-providers-daemon-deep`. Date: 2026-06-24.

## Headline

**NOT fully consistent.** The cluster's submit/cancel/queue/inbox/ack/trace flows are real
(via `submission_service.rs` + `ccb-mailbox`, proven by integration tests), but
**`comms_recover`, `retry`, `resubmit` are placeholder stubs** in `services/dispatcher.rs`
returning canned JSON — Python has full implementations + 12 dedicated tests. This is a
genuine functional gap. It also explains why the green test suite does NOT prove full
consistency: these RPCs are untested.

## 1. Confirmed GENUINE GAP — `comms_recover`

- **Rust:** `app.dispatcher: JobDispatcher` (`app.rs:14,75,166`) →
  `services/dispatcher.rs::JobDispatcher::comms_recover` (line 360) returns placeholder
  `{"status":"ok","payload":payload}`.
  RPC route: `handlers/comms_recover.rs::handle_comms_recover` → `app.dispatcher.comms_recover(payload)` → **placeholder**.
- **No real recovery logic anywhere in Rust** — grep for `recover_terminal_retry`,
  `recover_stale_running`, `recover_reply_delivery`, `comms_recoverability`, `_can_retry_job`,
  `lineage_for_job` across `rust/crates/` = **0 hits**.
- **No Rust test references `comms_recover`.**
- **Python:** `lib/ccbd/services/dispatcher_runtime/comms_recover.py` (418 lines, ~20 fns:
  `comms_recoverability_for_job`, `comms_recover`, `_recover_terminal_retry`,
  `_recover_stale_running`, `_recover_reply_delivery`, `_tick_after_recovery`, `_can_retry_job`,
  lineage lookups, audit, etc.).
- **Python tests (`test_ccbd_comms_recover.py`, 12 cases):**
  does_not_cancel_healthy_running_job; accepts_provider_prompt_idle_hint_for_running_job;
  accepts_provider_prompt_idle_stale_hint_for_running_job;
  accepts_provider_prompt_input_stuck_hint_for_running_job; rejects_unknown_running_hint;
  cancels_stale_running_and_starts_waiting_job; is_idempotent_after_retry;
  releases_only_targeted_mailbox_head; reply_delivery_race_is_noop_after_delivery_completes;
  failed_reply_delivery_resets_reply_head_and_schedules_delivery;
  failed_reply_delivery_is_idempotent_after_new_delivery_starts;
  project_view_marks_recoverable_and_clears_after_recovery.

**Verdict: comms_recover = GENUINE GAP.** Requires full 1:1 implementation of the recovery
state machine + the 12 tests, mirroring Python `comms_recover.py`.

## 2. Suspected gaps (same placeholder pattern — need Python-source verification)

`services/dispatcher.rs` also has canned-JSON placeholders for:
- `retry` (line 374): `{"target","status":"retried"}` — Python has real retry-resolution logic.
- `resubmit` (line 367): `{"message_id","status":"resubmitted"}` — Python has real resubmit.
- `ack_reply` (352), `mailbox_head` (345), queue/inbox (340) — BUT these are superseded by the
  real `mailbox_control`/`mailbox` path (integration tests prove inbox/ack/trace work), so the
  dispatcher.rs placeholders here are likely DEAD (real path elsewhere). **Verify retry/resubmit
  specifically** — confirm whether a real path exists or they are true gaps.

## 3. Callbacks — partial (needs deeper verification)

- Python `callbacks.py` (731 lines, 25 fns: callback-edge registration, continuation-job
  submission, callback-chain validation, timeout sweep, delegated-terminal persistence).
- Rust has: `submission_service.rs:141 validate_callback_request`, `submission_service.rs:306`
  callback-routing detection, `ccb-mailbox CallbackEdgeRecord` + `pending_callback_edges`.
- **Open:** is the full continuation-job / chain-validation / timeout-sweep / delegated-terminal
  logic implemented in mailbox/dispatcher, or only the edge model? Verify per Python fn.

## 4. Confirmed CONSISTENT areas

- **submit / cancel / queue / inbox / mailbox_head / trace / watch / ack** — real via
  `submission_service.rs` + `ccb-mailbox`; 12 `daemon_integration_tests.rs` pass.
- **reload / additive-patch apply** — real via `reload_apply_service.rs` +
  `reload_apply_namespace.rs` + `reload_transaction.rs::apply_add_agent/apply_remove_agent`;
  `reload_tests.rs` exercises add/remove agent/window apply (`applied=true`). (NOT a gap —
  earlier additive_patch stubs are mirrors; logic lives in reload_transaction.)
- **namespace materialize/ensure, supervision loop_runner, health assessment/rebind,
  api-models, fault-injection, reply-delivery formatting, client resolution** — mapped per
  parity matrix, real + tested.

## 5. Verdict — `daemon_lifecycle` cluster

**PARTIAL — one confirmed genuine gap (`comms_recover`), 2 suspected (`retry`, `resubmit`),
1 partial (`callbacks`), rest consistent.** The comms_recover gap is the most material: it is a
whole recovery subsystem (418 Python lines + 12 tests) with only a placeholder in Rust.

## 6. Recommended action

1. Implement `comms_recover` recovery 1:1 with Python `comms_recover.py`, behind the 12
   `test_ccbd_comms_recover.py` cases ported to Rust (TDD). Wire `JobDispatcher::comms_recover`
   to the real logic (replace placeholder).
2. Verify + implement `retry` / `resubmit` if Python-backed.
3. Complete `callbacks` verification; implement missing continuation/chain/timeout logic.
4. Update `plans/rust-python-test-parity-matrix.md` daemon_lifecycle row.
