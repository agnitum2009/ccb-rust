# Implementation Plan: py2rust parity matrix audit and quick wins

## Ordered Checklist

### Phase 1 — Audit and matrix update

1. **`management_cleanup`**
   - [x] Inspect Python `test_management_cleanup.py` and Rust `claude_home_cleanup.rs` tests.
   - [x] Confirm both test functions (`test_cleanup_command_docs_cover_wrapper_registries`, `test_cleanup_permissions_cover_wrapper_registries`) are parity-mapped.
   - [x] Mark cluster `complete` in parity matrix.

2. **`cli_entrypoint`**
   - [x] Inspect Python tests and existing Rust `cli_*_tests.rs` files.
   - [x] Identify which mapped Rust tests already cover the referenced Python behavior.
   - [x] Keep genuinely missing install/update areas `partial`; existing coverage notes remain accurate.

3. **Other `partial` clusters**
   - [x] Review each remaining `partial` row in `plans/rust-python-test-parity-matrix.md`.
   - [x] For clusters where Rust tests/implementation already parity-map the Python reference, mark `complete`.
   - [x] For clusters with larger remaining work, keep `partial` with updated notes.

### Phase 2 — Small gap closure (if any found during audit)

4. [x] No small gaps requiring code changes were found during this audit.
5. [x] Ran targeted tests for all candidate crates; all pass.

### Phase 3 — Validation and commit

6. [x] Run `cargo fmt --all -- --check`.
7. [x] No crates required clippy because no code was modified; earlier crate test runs were clean.
8. [x] Ran `cargo test -p <crate> -- --test-threads=1` for candidate crates; all pass.
9. [x] Update `plans/rust-python-test-parity-matrix.md` with final statuses and notes.
10. [x] Record a list of remaining larger tasks in task notes.
11. [ ] Commit changes and archive task.

## Validation Commands

```bash
cargo test -p ccb-cli -- --test-threads=1
cargo fmt --all -- --check
cargo clippy -p ccb-cli --tests
```

## Risky Files / Rollback Points

- `plans/rust-python-test-parity-matrix.md` — only status/text changes, no logic risk.
- If any audit reveals a need to change ccbd protocol / tmux namespace / provider hook injection, stop and escalate rather than editing.
