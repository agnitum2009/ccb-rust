# Implementation Plan: py2rust memory project memory parity

## Ordered Checklist

1. **Read** `test_project_memory_filters.py`, `test_project_memory.py`, and existing `ccb-memory` integration tests.
2. **Fix legacy-section filter** so that boundary headers are not preserved when stripping legacy collaboration sections.
3. **Add v4 legacy template hash detection** in `ccb-memory/src/project_memory/seed.rs`.
4. **Ensure seed metadata parent directory exists** before atomic write.
5. **Validate agent names** in `materialize_runtime_memory_bundle` and return early with warnings for invalid names.
6. **Align `normalize_agent_name`** in `ccb-storage/src/path_helpers.rs` with Python validation.
7. **Add integration tests** in `ccb-memory/tests/integration_tests.rs` covering filters, ensure, load, and materialize parity.
8. **Run** `cargo test -p ccb-memory`, `cargo fmt --check`, `cargo clippy -p ccb-memory -p ccb-storage --tests`, and `cargo test -p ccb-cli`.
9. **Update** `plans/rust-python-test-parity-matrix.md` and commit.

## Validation Commands

```bash
cargo test -p ccb-memory -- --test-threads=1
cargo fmt --all -- --check
cargo clippy -p ccb-memory -p ccb-storage --tests
cargo test -p ccb-cli -- --test-threads=1
```

## Risky Files / Rollback Points
- `ccb-storage/src/path_helpers.rs` — agent-name normalization now rejects invalid names; may affect callers that previously relied on permissive behavior.
- `ccb-memory/src/project_memory/materializer.rs` — invalid agent names now short-circuit without writing.
