# Implementation Plan: py2rust memory provider/real-context parity

## Ordered Checklist

1. **Provider transfer extraction (`ccb-memory`)**
   - [ ] Implement `extract_from_codex` in `rust/crates/ccb-memory/src/transfer_runtime/providers.rs` (or `transfer.rs`).
   - [ ] Implement `extract_from_opencode` with session capture and `SessionNotFoundError` fallback.
   - [ ] Re-export functions from `ccb-memory/src/lib.rs` if needed for tests.
   - [ ] Add unit/integration tests mirroring `test_memory_transfer_providers.py`.

2. **Unified managed memory renderer (`ccb-memory`)**
   - [ ] Add `render_managed_agent_memory` in `rust/crates/ccb-memory/src/project_memory/renderer.rs` (or new module).
   - [ ] Include project memory, agent private memory, filtered source-home memory, provider label, and runtime coordination rules.
   - [ ] Re-use existing `filter_memory_source` to strip CCB install/roles marker blocks.

3. **Provider materialization alignment**
   - [ ] Update `ccb-providers/src/claude/launcher_runtime/home.rs::materialize_claude_memory` to use the unified renderer.
   - [ ] Update `ccb-provider-profiles/src/codex_home_config.rs::materialize_provider_memory_file` to use the unified renderer.
   - [ ] Update `ccb-providers/src/opencode/launcher.rs::materialize_opencode_memory_config` to use the unified renderer and set instructions/env.
   - [ ] Ensure Gemini path via `materialize_runtime_memory_bundle` already produces compatible output; adjust if needed.

4. **Real-context integration test (`ccb-memory`)**
   - [ ] Add `test_realistic_provider_memory_context_composes_each_provider_bundle` in `rust/crates/ccb-memory/tests/integration_tests.rs`.
   - [ ] Replicate Python fixture: project memory, agent private memories, source homes, workspace, and assertions for Claude/Codex/OpenCode/Gemini.

5. **Validation & documentation**
   - [ ] Run `cargo test -p ccb-memory -- --test-threads=1`.
   - [ ] Run `cargo fmt --all -- --check`.
   - [ ] Run `cargo clippy -p ccb-memory -p ccb-storage --tests`.
   - [ ] Update `plans/rust-python-test-parity-matrix.md` memory row.
   - [ ] Archive task and commit.

## Validation Commands

```bash
cargo test -p ccb-memory -- --test-threads=1
cargo fmt --all -- --check
cargo clippy -p ccb-memory -p ccb-storage --tests
```

## Risky Files / Rollback Points
- `rust/crates/ccb-provider-profiles/src/codex_home_config.rs` — memory header/format changes may affect existing provider-profile tests.
- `rust/crates/ccb-providers/src/claude/launcher_runtime/home.rs` — same as above for Claude.
- `rust/crates/ccb-providers/src/opencode/launcher.rs` — same for OpenCode.
