# Implementation Plan: py2rust memory module and auto-transfer parity

## Ordered Checklist

1. **Read** `test_memory_module.py` and `test_memory_auto_transfer.py`.
2. **Implement** `ccb-memory/src/auto_transfer.rs`:
   - Static deduplication map with TTL pruning.
   - `maybe_auto_transfer` and `maybe_auto_transfer_with`.
   - `is_current_work_dir` using normalized current working directory.
3. **Add integration tests** in `ccb-memory/tests/integration_tests.rs` for deduper markers, mixed tool collapse, dedupe edge cases, formatter estimate/truncate, parser corruption tolerance, session info, and auto-transfer start-once/foreign-dir.
4. **Run** `cargo test -p ccb-memory`, `cargo fmt --check`, `cargo clippy -p ccb-memory -p ccb-storage --tests`.
5. **Update** `plans/rust-python-test-parity-matrix.md` and commit.

## Validation Commands

```bash
cargo test -p ccb-memory -- --test-threads=1
cargo fmt --all -- --check
cargo clippy -p ccb-memory -p ccb-storage --tests
```

## Risky Files / Rollback Points
- `ccb-memory/src/auto_transfer.rs` — uses a process-global static map; tests must call `clear_seen()` to avoid cross-test contamination.
