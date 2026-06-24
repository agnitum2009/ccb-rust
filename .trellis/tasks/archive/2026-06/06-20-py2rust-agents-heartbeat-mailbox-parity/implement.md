# Implementation: py2rust agents/heartbeat/mailbox parity extensions

## Goal
Close remaining low-risk parity gaps for the `agents_roles`, `heartbeat`, and `mailbox` clusters by extending Rust tests to match the Python reference tests already mapped in `plans/rust-python-test-parity-matrix.md`.

## Changes

### `ccb-agents`
- `rust/crates/ccb-agents/src/models.rs`
  - Derived `PartialEq` and `Eq` for `AgentSpec` and `AgentRuntime` so full-field roundtrip assertions can mirror Python `==` checks.
- `rust/crates/ccb-agents/tests/store_tests.rs`
  - Added `test_agent_stores_full_field_roundtrip` covering `AgentApiSpec`, `model`, `branch_template`, runtime binding/reconcile fields, and `RestoreStatus::Provider`.

### `ccb-heartbeat`
- `rust/crates/ccb-heartbeat/tests/integration.rs`
  - Added `maintenance_classifier_keeps_active_comms_without_current_job_healthy`.
  - Added `maintenance_classifier_flags_active_degraded_activity_evidence`.

### `ccb-mailbox`
- `rust/crates/ccb-mailbox/tests/integration.rs`
  - Added `test_record_retry_attempt_increments_queue_without_refreshing_mailbox`.
  - Added `test_record_reply_delivery_skips_non_mailbox_caller`.

### Documentation
- `plans/rust-python-test-parity-matrix.md`
  - Updated `agents_roles`, `heartbeat`, and `mailbox` rows to `complete` for the mapped Python tests, with notes about deferred higher-level integration.

## Validation Commands

```bash
cargo test -p ccb-agents -p ccb-heartbeat -p ccb-memory -p ccb-mailbox -p ccb-message-bureau -- --test-threads=1
cargo fmt --all -- --check
cargo clippy -p ccb-cli --tests
```

All passed.
