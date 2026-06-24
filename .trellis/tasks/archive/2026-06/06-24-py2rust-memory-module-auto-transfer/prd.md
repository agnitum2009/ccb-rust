# py2rust memory module and auto-transfer parity

## Goal
Add Rust parity for `ccb-memory` conversation deduper/formatter/parser and auto-transfer orchestration so that `test_memory_module.py` and `test_memory_auto_transfer.py` behaviors are covered.

## Requirements

1. **Deduper parity** (`test_memory_module.py`):
   - Strip `CCB_REQ_ID`, `CCB_BEGIN`, `CCB_DONE`, and `[CCB_ASYNC_SUBMITTED ...]` protocol markers.
   - Strip `<system-reminder>`, `<env>`, and `<!-- CCB_CONFIG -->` system noise.
   - Deduplicate consecutive duplicate messages while preserving distinct messages.
   - Collapse tool calls into summaries, including mixed tool kinds.

2. **Formatter parity** (`test_memory_module.py`):
   - `estimate_tokens` divides text length by 4.
   - `truncate_to_limit` keeps the newest conversation pairs that fit.
   - Markdown/Plain/JSON formatters produce expected headers.

3. **Session parser parity** (`test_memory_module.py`):
   - Tolerate corrupted JSONL lines and still parse valid entries.
   - `get_session_info` returns file stem as session id and absolute path string.

4. **Auto-transfer parity** (`test_memory_auto_transfer.py`):
   - `maybe_auto_transfer` is controlled by `CCB_CTX_TRANSFER_ON_SESSION_SWITCH` (default true).
   - Starts transfer at most once per deduplication key.
   - Skips transfers when `work_dir` does not match the current working directory.

## Acceptance Criteria
- `cargo test -p ccb-memory -- --test-threads=1` passes.
- `cargo fmt --all -- --check` passes.
- `cargo clippy -p ccb-memory -p ccb-storage --tests` produces no new warnings.
- `plans/rust-python-test-parity-matrix.md` updated.
