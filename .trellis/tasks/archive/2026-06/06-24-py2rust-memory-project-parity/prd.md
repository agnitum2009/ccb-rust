# py2rust memory project memory parity

## Goal
Add Rust parity tests and fixes for `ccb-memory` project-memory behavior so that the Python reference tests `test_project_memory_filters.py` and `test_project_memory.py` are covered.

## Requirements

1. **Filter parity** (`test_project_memory_filters.py`):
   - Confirm `filter_memory_source` strips CCB config/roles/rubrics/review/gemini-inspiration marker blocks.
   - Strip legacy collaboration sections in English and Chinese.
   - Preserve paragraph spacing, isolated markers, unrelated text, and only apply filters to `provider_user_memory` sources.

2. **Project memory lifecycle** (`test_project_memory.py`):
   - `ensure_project_memory` creates the v5 template and seed metadata.
   - Does not overwrite existing memory files.
   - Ignores legacy root `CCB.md`.
   - Backfills missing seed for unedited template.
   - Upgrades unedited v4 seeded templates and unedited v4 legacy templates without seed.
   - Does not upgrade edited old seeds.

3. **Load/materialize parity** (`test_project_memory.py`):
   - `load_memory_sources` reads from project root, supports `include_provider_native_project`, and skips provider-native for duplicate-loading providers (Claude/Codex/OpenCode).
   - `materialize_runtime_memory_bundle` writes generated bundle with workspace path, skips unchanged writes, and rejects invalid agent names without writing.

4. **Agent name normalization**:
   - Align `ccb-storage::path_helpers::normalize_agent_name` with Python `agents.models_runtime.names.normalize_agent_name` (pattern `^[a-zA-Z][a-zA-Z0-9_-]{0,31}$`, reserved names).

## Acceptance Criteria
- `cargo test -p ccb-memory -- --test-threads=1` passes.
- `cargo fmt --all -- --check` passes.
- `cargo clippy -p ccb-memory -p ccb-storage --tests` produces no new warnings.
- `cargo test -p ccb-cli -- --test-threads=1` still passes (regression check).
- `plans/rust-python-test-parity-matrix.md` updated.
