# PRD: py2rust agents/heartbeat/mailbox parity extensions

## Problem
The `agents_roles`, `heartbeat`, and `mailbox` clusters in `plans/rust-python-test-parity-matrix.md` were marked `partial`. The remaining Python reference tests for these clusters needed corresponding Rust test coverage without touching high-risk daemon socket protocols or mailbox kernel contracts.

## Requirements
1. Extend `ccb-agents` store tests to mirror `test_v2_agent_store.py` full-field roundtrip.
2. Extend `ccb-heartbeat` classifier tests to cover the remaining Python `test_maintenance_heartbeat.py` classifier cases.
3. Extend `ccb-mailbox` message-bureau integration tests to cover the fastpath/retry/reply behaviors in `test_message_bureau_submission_fastpath.py`.
4. Update the parity matrix and archive the task.

## Out of Scope
- Daemon socket / `connect_mounted_daemon` / `invoke_mounted_daemon` changes.
- Mailbox kernel CAS / claim / consume contract changes.
- Provider log-reader / home-config materialization (remains under `ccb-memory`).
