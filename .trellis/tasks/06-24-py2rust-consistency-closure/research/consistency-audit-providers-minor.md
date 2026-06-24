# Consistency Audit — Providers Minor Unmapped Sub-features (Python ↔ Rust)

> Trellis artifact for task `06-24-py2rust-consistency-closure` §2.
> Date: 2026-06-24.
> Scope: `claude_registry`, `opencode` sub-features, `provider_execution/active_resume`.

## Headline

**All three focus areas are genuine functional gaps.** Python has real implementations and dedicated tests for each; Rust has only 1:1 alignment stubs or partial canonical implementations that do **not** cover the asserted behavior. These are secondary to the callbacks subsystem, but they are not already implemented elsewhere.

| Area | Python Status | Rust Status | Verdict |
|---|---|---|---|
| `claude_registry` | Real registry/cache/events/log-binding/discovery code + 4 test files | `registry_runtime/*` and `registry_support` equivalent are 3-line stubs; only top-level `registry.rs` has basic session loading | **GAP — implement** |
| `opencode` (comm_sqlite, session_ensure_pane, communicator_state) | Real SQLite log reader, pane lifecycle, communicator init + 3 test files | `comm.rs`, `runtime/communicator.rs`, `runtime/session_lookup*.rs` are stubs; `reader.rs` is file-only (no SQLite); `session.rs` has no `ensure_pane` | **GAP — implement** |
| `provider_execution/active_resume` | Generic `resume_active_submission` helper + 3 tests | `active_runtime/resume.rs` and `models.rs` are stubs; provider-specific `resume` methods exist but do not restore reader/backend/completion_dir | **GAP — implement** |

---

## 1. `claude_registry`

### Python source / test files examined

**Source**
- `lib/provider_backends/claude/registry.py` — public facade re-export.
- `lib/provider_backends/claude/registry_runtime/state.py` — `SessionEntry`, `WatcherEntry`, `RegistryRuntimeState`.
- `lib/provider_backends/claude/registry_runtime/cache_runtime/loading.py` — `register_session`, `load_and_cache`.
- `lib/provider_backends/claude/registry_runtime/cache_runtime/lookup.py` — `get_session` (mtime reload).
- `lib/provider_backends/claude/registry_runtime/cache_runtime/mutation.py` — `invalidate`, `remove`.
- `lib/provider_backends/claude/registry_runtime/events_runtime/{global_logs,project_logs,sessions_index,common}.py` — log-file event handlers.
- `lib/provider_backends/claude/registry_support/logs_runtime/binding.py` — `refresh_claude_log_binding`.
- `lib/provider_backends/claude/registry_support/logs_runtime/discovery.py` + `discovery_runtime/{extract,lookup,scan}.py` — log discovery helpers.
- `lib/provider_backends/claude/registry_support/logs_runtime/{indexing,meta}.py` — sessions-index parsing and log meta reading.

**Tests**
- `test/test_claude_registry_cache.py`
- `test/test_claude_registry_events.py`
- `test/test_claude_registry_logs_binding.py`
- `test/test_claude_registry_logs_discovery.py`

### Rust files examined

- `rust/crates/ccb-providers/src/claude/registry.rs` — basic `ClaudeSessionRegistry` with `register/get/unregister/refresh/list`; has inline tests for register/get/refresh. **Does not** implement cache mtime reload, watcher lifecycle, events, or log binding refresh.
- `rust/crates/ccb-providers/src/claude/registry_runtime/{cache,events,state,binding_runtime}.rs` — 3-line stubs.
- `rust/crates/ccb-providers/src/claude/registry_runtime/cache_runtime/{mod,loading,lookup,mutation}.rs` — 3-line stubs.
- `rust/crates/ccb-providers/src/claude/registry_runtime/events_runtime/{common,global_logs,project_logs,sessions_index}.rs` — 3-line stubs.
- `rust/crates/ccb-providers/src/claude/registry_runtime/session_updates.rs` / `session_updates_runtime.rs` — 3-line stubs.
- No `registry_support/` equivalent exists in Rust.

### Per-test behavior and coverage

| Python Test | Behavior Asserted | Rust Coverage | Verdict |
|---|---|---|---|
| `test_claude_registry_cache.py::test_get_session_reloads_when_session_file_mtime_changes` | `get_session` detects session-file mtime change and reloads via injected `load_and_cache_fn` | Not covered. Rust `registry.rs` has no mtime-aware cache reload logic. | GAP |
| `test_claude_registry_cache.py::test_register_session_stores_valid_entry_and_ensures_watchers` | `register_session` stores a valid `SessionEntry` and calls watcher setup | Not covered. Stubs only. | GAP |
| `test_claude_registry_cache.py::test_load_and_cache_returns_none_for_unhealthy_session_but_caches_entry` | Unhealthy session (`ensure_pane` fails) is cached as invalid, returns `None` | Not covered. | GAP |
| `test_claude_registry_cache.py::test_invalidate_and_remove_release_watchers` | `invalidate`/`remove` set validity/delete entry, log, and release watchers | Not covered. | GAP |
| `test_claude_registry_cache.py::test_registry_resolves_named_workspace_session_file` | `ClaudeSessionRegistry._find_claude_session_file` / `_load_claude_session` resolve workspace-bound sessions | Partial: top-level `registry.rs::register` loads `.claude-session`, but named-instance / workspace-binding resolution is not tested or fully implemented. | GAP |
| `test_claude_registry_events.py::test_handle_new_log_file_global_updates_session_and_session_file` | Global log watcher updates session binding and session file | Not covered. | GAP |
| `test_claude_registry_events.py::test_handle_new_log_file_marks_pending_when_unscoped_log_does_not_update` | Project-scoped unbound log is marked pending | Not covered. | GAP |
| `test_claude_registry_events.py::test_handle_sessions_index_updates_matching_registry_entries` | `sessions-index.json` updates matching registry entries | Not covered. | GAP |
| `test_claude_registry_logs_binding.py::test_refresh_claude_log_binding_prefers_intended_resume_log` | `refresh_claude_log_binding` prefers log derived from `--resume <sid>` start cmd | Not covered. | GAP |
| `test_claude_registry_logs_binding.py::test_refresh_claude_log_binding_respects_index_without_forced_scan` | Respects `sessions-index` without scan when not forced | Not covered. | GAP |
| `test_claude_registry_logs_binding.py::test_refresh_claude_log_binding_uses_scan_when_forced` | Falls back to filesystem scan when `force_scan=True` | Not covered. | GAP |
| `test_claude_registry_logs_discovery.py::test_extract_session_id_from_start_cmd_finds_uuid` | UUID extracted from `claude --resume <uuid>` | Not covered. | GAP |
| `test_claude_registry_logs_discovery.py::test_find_log_for_session_id_returns_newest_match` | Finds newest file matching session id under root | Not covered. | GAP |
| `test_claude_registry_logs_discovery.py::test_scan_latest_log_for_work_dir_skips_sidechain_and_returns_matching_log` | Scan skips sidechain logs and returns latest matching work-dir log | Not covered. | GAP |

### Recommended action

**`implement`** — lower priority than callbacks, but a real gap. The 4 Python test files give a ready-made TDD suite.

### Smallest Rust file set to touch

1. `rust/crates/ccb-providers/src/claude/registry_runtime/state.rs` — define `SessionEntry`, `WatcherEntry`, `RegistryRuntimeState`.
2. `rust/crates/ccb-providers/src/claude/registry_runtime/cache_runtime/loading.rs` — implement `register_session`, `load_and_cache`.
3. `rust/crates/ccb-providers/src/claude/registry_runtime/cache_runtime/lookup.rs` — implement mtime-aware `get_session`.
4. `rust/crates/ccb-providers/src/claude/registry_runtime/cache_runtime/mutation.rs` — implement `invalidate`, `remove`.
5. `rust/crates/ccb-providers/src/claude/registry_runtime/events_runtime/{global_logs,project_logs,sessions_index,common}.rs` — implement event handlers.
6. `rust/crates/ccb-providers/src/claude/registry_runtime/session_updates.rs` / `session_updates_runtime.rs` — log meta reading / session file direct update.
7. New module `rust/crates/ccb-providers/src/claude/registry_support/logs_runtime/` with `binding.rs`, `discovery.rs`, `discovery_runtime/{extract,lookup,scan}.rs`, `indexing.rs`, `meta.rs`.
8. `rust/crates/ccb-providers/src/claude/registry.rs` — extend facade to wire cache/events/binding refresh; add workspace-binding resolution tests.

### Suggested tests

- Port the 4 Python test files into `rust/crates/ccb-providers/tests/claude_registry_{cache,events,logs_binding,logs_discovery}_tests.rs`.
- Alternatively, keep unit tests inline in the new modules (matching existing crate style) and add one integration test file per Python test file.

---

## 2. `opencode`

### Python source / test files examined

**Source**
- `lib/provider_backends/opencode/comm.py` — re-exports `OpenCodeCommunicator`, `OpenCodeLogReader`.
- `lib/provider_backends/opencode/session.py` — session facade.
- `lib/provider_backends/opencode/session_runtime/model.py` — `OpenCodeProjectSession`.
- `lib/provider_backends/opencode/session_runtime/lifecycle.py` — `ensure_pane` (delegates to `pane_log_support.lifecycle`).
- `lib/provider_backends/opencode/runtime/communicator.py` — `initialize_state`, `check_session_health`, `send_message`, `ask_async`, `ask_sync`.
- `lib/provider_backends/opencode/runtime/log_reader_facade_runtime/reader.py` / `timeline.py` / `state.py` / `storage.py` — log reader facade.
- `lib/provider_backends/opencode/runtime/storage_reader.py` — `read_messages`, `read_parts`, `get_latest_session*`.
- `lib/provider_backends/opencode/runtime/message_reader.py` — DB + file message/part reading.
- `lib/provider_backends/opencode/runtime/session_lookup.py` / `session_lookup_runtime/{common,db,files}.py` — latest session resolution (DB first, then files).
- `lib/opencode_runtime/storage.py` — `OpenCodeStorageAccessor` with SQLite `fetch_opencode_db_rows`.

**Tests**
- `test/test_opencode_comm_sqlite.py`
- `test/test_opencode_session_ensure_pane.py`
- `test/test_opencode_communicator_state.py`
- `test/test_opencode_comm_fields.py` (related session loading fields)

### Rust files examined

- `rust/crates/ccb-providers/src/opencode/comm.rs` — 3-line stub.
- `rust/crates/ccb-providers/src/opencode/session.rs` — loads session JSON, exposes accessors; **no `ensure_pane`**.
- `rust/crates/ccb-providers/src/opencode/runtime/communicator.rs` — 3-line stub.
- `rust/crates/ccb-providers/src/opencode/runtime/session_runtime.rs` — 3-line stub.
- `rust/crates/ccb-providers/src/opencode/runtime/session_lookup.rs` — 3-line stub.
- `rust/crates/ccb-providers/src/opencode/runtime/session_lookup_runtime/db.rs` — 3-line stub.
- `rust/crates/ccb-providers/src/opencode/reader.rs` — file-based `OpenCodeLogReader` with `try_get_message`/`capture_state`; **no SQLite path**.
- `rust/crates/ccb-providers/src/opencode/storage.rs` — `OpenCodeStorageAccessor` with DB path resolution; **no `fetch_opencode_db_rows`**.
- `rust/crates/ccb-providers/src/opencode/replies.rs` — text/reply extraction (already covered).
- `rust/crates/ccb-providers/tests/opencode_tests.rs` — covers manifest, launcher, storage accessor basics, file-based reader, execution adapter; **does not** cover SQLite, `ensure_pane`, or communicator init.

### Per-test behavior and coverage

| Python Test | Behavior Asserted | Rust Coverage | Verdict |
|---|---|---|---|
| `test_opencode_comm_sqlite.py::test_opencode_log_reader_reads_messages_and_parts_from_sqlite` | `_read_messages` / `_read_parts` query SQLite `message`/`part` tables | Not covered. Rust reader is file-only. | GAP |
| `test_opencode_comm_sqlite.py::test_opencode_log_reader_falls_back_to_json_when_sqlite_has_no_matching_rows` | SQLite query empty → fall back to JSON files | Not covered. | GAP |
| `test_opencode_comm_sqlite.py::test_opencode_log_reader_stays_pinned_to_filtered_session_by_default` | `_get_latest_session_from_db` respects `session_id_filter` | Not covered. | GAP |
| `test_opencode_session_ensure_pane.py::test_ensure_pane_respawns_recorded_pane_without_marker_rebind` | `ensure_pane` respawns dead tmux pane | Not covered. `OpenCodeProjectSession` has no `ensure_pane`. | GAP |
| `test_opencode_session_ensure_pane.py::test_ensure_pane_already_alive` | Live pane returns success immediately | Not covered. | GAP |
| `test_opencode_session_ensure_pane.py::test_ensure_pane_no_backend` | Missing backend returns failure | Not covered. | GAP |
| `test_opencode_session_ensure_pane.py::test_ensure_pane_dead_no_marker` | Dead pane + no marker → failure | Not covered. | GAP |
| `test_opencode_session_ensure_pane.py::test_ensure_pane_missing_tmux_target_skips_respawn_noise` | Missing tmux target → failure without respawn | Not covered. | GAP |
| `test_opencode_communicator_state.py::test_initialize_state_populates_runtime_fields` | `initialize_state` loads session info, sets `ccb_session_id`, `runtime_dir`, `terminal`, `pane_id`, `backend`, `timeout`, `project_session_file`, `log_reader`, publishes registry | Not covered. `runtime/communicator.rs` is a stub. | GAP |
| `test_opencode_communicator_state.py::test_initialize_state_raises_when_session_missing` | Missing session → `RuntimeError` | Not covered. | GAP |
| `test_opencode_comm_fields.py::test_opencode_comm_load_session_info_backfills_project_fields` | `_load_session_info` backfills `_session_file`, `opencode_session_id`, `opencode_project_id` | Not covered. `comm.rs` is a stub. | GAP |
| `test_opencode_comm_fields.py::test_opencode_comm_find_session_file_prefers_ccb_session_file` | `_find_session_file` prefers `CCB_SESSION_FILE` env | Not covered. | GAP |

### Recommended action

**`implement`** — real gap; OpenCode runtime is not functionally complete without SQLite log reading and pane lifecycle. The 4 test files provide a clear spec.

### Smallest Rust file set to touch

1. `rust/crates/ccb-providers/src/opencode/storage.rs` — add `fetch_opencode_db_rows` (SQLite query helper). Will need a SQLite dependency (check whether one is already in workspace; if not, add `rusqlite` or equivalent to `ccb-providers/Cargo.toml`).
2. `rust/crates/ccb-providers/src/opencode/runtime/session_lookup_runtime/db.rs` — implement `get_latest_session_from_db`.
3. `rust/crates/ccb-providers/src/opencode/runtime/session_lookup.rs` — DB-first then files lookup.
4. `rust/crates/ccb-providers/src/opencode/runtime/message_reader.rs` (new) — implement `read_messages`/`read_parts` with DB + file fallback.
5. `rust/crates/ccb-providers/src/opencode/reader.rs` — add SQLite paths (`_read_messages`, `_read_parts`, `_get_latest_session_from_db`) and wire DB-first lookup in `try_get_message`.
6. `rust/crates/ccb-providers/src/opencode/session.rs` — add `ensure_pane` and backend helpers, or delegate to a shared pane-lifecycle module.
7. `rust/crates/ccb-providers/src/opencode/runtime/session_runtime.rs` — add session helpers if needed by `ensure_pane`.
8. `rust/crates/ccb-providers/src/opencode/comm.rs` — implement `_load_session_info`, `_find_session_file`, `OpenCodeCommunicator`.
9. `rust/crates/ccb-providers/src/opencode/runtime/communicator.rs` — implement `initialize_state`, `check_session_health`, `send_message`, `ask_async`, `ask_sync`.

### Suggested tests

- `rust/crates/ccb-providers/tests/opencode_comm_sqlite_tests.rs` — port the 3 SQLite cases.
- `rust/crates/ccb-providers/tests/opencode_session_ensure_pane_tests.rs` — port the 5 pane cases with a fake tmux backend.
- `rust/crates/ccb-providers/tests/opencode_communicator_state_tests.rs` — port the 2 communicator-state cases.
- `rust/crates/ccb-providers/tests/opencode_comm_fields_tests.rs` — port the 2 `_load_session_info` / `_find_session_file` cases.

---

## 3. `provider_execution/active_resume`

### Python source / test files examined

**Source**
- `lib/provider_execution/active_runtime/resume.py` — `resume_active_submission` and helpers `_active_work_dir`, `_resume_prepared_session`, `_resumed_runtime_state`.
- `lib/provider_execution/active_runtime/start.py` — `prepare_active_start`, `_session_selector_name`.
- `lib/provider_execution/active_runtime/models.py` — `PreparedActiveStart`, `PreparedActivePoll`.
- `lib/provider_execution/active.py` — public re-exports.

**Tests**
- `test/test_provider_execution_active_resume.py`

### Rust files examined

- `rust/crates/ccb-providers/src/active_runtime/resume.rs` — 3-line stub.
- `rust/crates/ccb-providers/src/active_runtime/start.rs` — 3-line stub.
- `rust/crates/ccb-providers/src/active_runtime/models.rs` — 3-line stub.
- `rust/crates/ccb-providers/src/execution/service.rs` — `restore` calls `restore_submission`.
- `rust/crates/ccb-providers/src/execution/restore.rs` — generic restore orchestrator; calls `adapter.resume()`.
- `rust/crates/ccb-providers/src/providers/codex.rs` — `resume_submission` restores workspace_path, pane_id, session_path; **does not** restore reader/backend/completion_dir.
- `rust/crates/ccb-providers/src/providers/claude.rs` — `resume` only flips `mode` to `active`; **does not** call `ensure_pane`, load reader, or restore backend.
- `rust/crates/ccb-providers/tests/execution_tests.rs` — no `resume` coverage beyond the existing codex adapter test `test_codex_execution_adapter_resume_requires_active_mode` (only checks passive-mode rejection).

### Per-test behavior and coverage

| Python Test | Behavior Asserted | Rust Coverage | Verdict |
|---|---|---|---|
| `test_provider_execution_active_resume.py::test_resume_active_submission_requires_active_workspace` | `context=None` → `None` | Partial: codex/claude adapters reject missing workspace, but the generic `resume_active_submission` helper does not exist. | GAP |
| `test_provider_execution_active_resume.py::test_resume_active_submission_skips_passive_runtime_state` | `mode != active` → `None` | Partial: codex adapter checks mode; claude adapter does not. | GAP |
| `test_provider_execution_active_resume.py::test_resume_active_submission_restores_reader_backend_and_completion_dir` | Restores `pane_id`, `backend`, `reader`, `completion_dir`, preserves `mode=active`, calls `configure_reader_fn` | Not covered. No Rust code restores all four fields via a generic helper. | GAP |

### Recommended action

**`implement`** — real gap. The generic resume helper is needed for parity with Python and is used by multiple providers (Python's `gemini` execution start calls it).

### Smallest Rust file set to touch

1. `rust/crates/ccb-providers/src/active_runtime/models.rs` — define `PreparedActiveStart` and `PreparedActivePoll`.
2. `rust/crates/ccb-providers/src/active_runtime/start.rs` — implement `prepare_active_start` and `_session_selector_name`.
3. `rust/crates/ccb-providers/src/active_runtime/resume.rs` — implement `resume_active_submission` with dependency-injected `load_session_fn`, `backend_for_session_fn`, `reader_factory`, optional `configure_reader_fn`, and `completion_dir_fn`.
4. `rust/crates/ccb-providers/src/providers/claude.rs` — update `ClaudeExecutionAdapter::resume` to use the generic helper (or implement provider-specific equivalent restoring reader/backend/completion_dir/pane_id).
5. `rust/crates/ccb-providers/src/providers/codex.rs` — optionally refactor `resume_submission` to use the generic helper.
6. `rust/crates/ccb-providers/src/providers/gemini.rs` (and others that need active resume) — wire generic helper if they currently lack full resume.

### Suggested tests

- `rust/crates/ccb-providers/tests/provider_execution_active_resume_tests.rs` — port the 3 Python cases.
- Add inline tests in `active_runtime/resume.rs` for `_active_work_dir`, `_resume_prepared_session`, and `_resumed_runtime_state`.

---

## 4. Overall Verdict

| Area | Action | Priority / Notes |
|---|---|---|
| `claude_registry` | **implement** | Lower priority than callbacks; large surface (cache + events + log binding + discovery). |
| `opencode` | **implement** | Concrete missing feature (SQLite log reading + pane lifecycle + communicator init). |
| `provider_execution/active_resume` | **implement** | Generic helper missing; blocks full active-mode resume parity across providers. |

**None of these areas are `covered`.** All three have real Python logic + tests and only Rust stubs or partial implementations. They should be closed after the callbacks subsystem (PRD §1.1) using the same TDD + per-subject commit pattern.

## 5. Post-implementation checklist

- [ ] `cargo test -p ccb-providers -- --test-threads=1` passes with new tests.
- [ ] `cargo clippy --workspace --all-targets` 0 errors.
- [ ] `cargo fmt --check` clean.
- [ ] Update `plans/rust-python-test-parity-matrix.md` providers row to reflect completed sub-features.
