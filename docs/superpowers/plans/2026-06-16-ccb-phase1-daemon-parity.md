# CCB Rust 1:1 Compatibility — Phase 1: ccb-daemon Control-Plane Parity

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the functional parity gaps in `ccb-daemon` that block end-to-end `ccbr start → ask → multi-window UI` and `ccbr reload` hot-swap, by classifying stubs against Python `lib/ccbd/` and implementing the B-class (missing behavior) stubs.

**Architecture:** This phase uses the "second-draft" methodology from `docs/rust-second-draft-plan.md`: classify every `ccb-daemon` stub as A (already covered by real code elsewhere), B (needs translation), or C (not applicable in Rust); then implement B-class stubs in priority order (P0 end-to-end UI first, P1 control-plane second). File-level 1:1 alignment is already scaffolded; we focus on behavior parity.

**Tech Stack:** Rust 2021, ccb-daemon crate, Python `lib/ccbd/` as truth source, codegraph for structural verification.

---

## File Structure (this phase)

| File/Directory | Responsibility |
|---|---|
| `rust/crates/ccb-daemon/src/` | 430 stub `.rs` files mirroring Python `lib/ccbd/` (many are currently flat while Python is nested). |
| `rust/crates/ccb-daemon/src/lib.rs` | Module declarations; must be updated if files are moved, added, or removed. |
| `rust/crates/ccb-daemon/src/app.rs` | `CcbdApp` control plane; owns handlers and services. |
| `rust/crates/ccb-daemon/src/handlers/start.rs` | RPC handler for `ccbr start`. |
| `rust/crates/ccb-daemon/src/handlers/project_reload.rs` | RPC handler for `ccbr reload`. |
| `rust/crates/ccb-daemon/src/start_flow_runtime_service.rs` | Real-code partial implementation of start flow (503 LOC). |
| `rust/crates/ccb-daemon/src/start_preparation.rs` | Partial implementation of agent preparation before start. |
| `rust/crates/ccb-daemon/src/supervision/` | Supervision loop and runtime health. |
| `rust/crates/ccb-daemon/src/services/project_namespace_runtime/` | Namespace/patch runtime operations. |
| `docs/rust-second-draft-plan.md` | Stub classification methodology. |
| `docs/python-rust-1to1-map.md` | File alignment reference. |

---

## Task 0: Realign ccb-daemon stub directory structure to match Python

> **Status:** Deferred per user decision. The initial inventory revealed that bulk realignment is high-blast-radius (150 duplicate flat files, 2 files with real code in flat locations, undeclared directory modules, and conflicts like `start_flow/` vs `start_flow_runtime/`). Realignment will be performed incrementally for B-class stubs during implementation, using the mapping in `docs/superpowers/plans/2026-06-16-ccb-daemon-stub-mapping.md`.

---

## Task 1.1: Establish stub inventory

**Files:** none (analysis only)

- [x] **Step 1: Count and list all stub files**

Run:
```bash
cd /home/agnitum/ccb
grep -rl "1:1 file alignment stub" rust/crates/ccb-daemon/src/ | sort > /tmp/ccb-daemon-stubs.txt
wc -l /tmp/ccb-daemon-stubs.txt
```

Actual: 430 stub files. See `docs/superpowers/plans/2026-06-16-ccb-daemon-stub-mapping.md`.

- [ ] **Step 2: Map stub files to Python source files**

For each stub path in `/tmp/ccb-daemon-stubs.txt`, derive the corresponding Python file by replacing `rust/crates/ccb-daemon/src/` with `lib/ccbd/` and `.rs` with `.py`.

Run:
```bash
sed 's|rust/crates/ccb-daemon/src/|lib/ccbd/|; s|\.rs$|.py|' /tmp/ccb-daemon-stubs.txt > /tmp/ccb-daemon-python-counterparts.txt
```

- [ ] **Step 3: Verify Python counterparts exist**

Run:
```bash
while read -r py; do
  if [[ ! -f "$py" ]]; then echo "MISSING: $py"; fi
done < /tmp/ccb-daemon-python-counterparts.txt | head -50
```

Actual: 387 stubs map to nested Python files (directory misalignment), 41 map to flat Python files, and 2 are C-class candidates. Recorded in `docs/superpowers/plans/2026-06-16-ccb-daemon-stub-mapping.md`.

---

## Task 1.2: Classify stubs with parallel subagents

**Files:** all files in `/tmp/ccb-daemon-stubs.txt`

- [ ] **Step 1: Split inventory into batches**

Divide the 430 stub files into 10 batches of ~43 files each by Python functional area (using the mapping file, because many Rust flat files map to nested Python packages):
1. `services/dispatcher_runtime/` (reply delivery, finalization, submission, restore)
2. `services/project_namespace_runtime/` (additive patch, ensure, materialize topology)
3. `supervision/` + `models_runtime/lifecycle_runtime/`
4. `start_flow_runtime/` + `start_runtime/` + `stop_flow_runtime/`
5. `services/health_monitor_runtime/` + `services/health_assessment/` + `services/job_heartbeat_runtime/`
6. `reload_*.rs` flat files + `runtime_runtime/` + `services/runtime_*`
7. `app_runtime/` + `keeper_runtime/` + `client_runtime/` + `socket_*_runtime/`
8. `handlers/` + `project_view/` + `project_focus/`
9. `api_models_runtime/` + `supervisor_runtime/` + flat top-level files (`keeper.rs`, `supervisor.rs`, `system.rs`, etc.)
10. remaining flat files (`metrics.rs`, `lifecycle_report_store.rs`, `restore_report_store.rs`, `startup_policy.rs`, etc.)

- [ ] **Step 2: Dispatch classification subagents**

For each batch, launch an `explore` subagent with this prompt:

```
You are classifying ccb-daemon Rust stubs against Python reference files.
Batch: <list of ~40 rust files>

For each rust file:
1. Read the stub (usually 3-10 lines).
2. Read the corresponding Python file using `docs/superpowers/plans/2026-06-16-ccb-daemon-stub-mapping.md` (many flat Rust files map to nested Python packages; do not assume same relative path).
3. Determine class:
   - A: The Python logic is already implemented elsewhere in the Rust workspace (e.g., in start_flow_runtime_service.rs, handlers/start.rs, services/dispatcher.rs). Add a comment with the Rust location.
   - B: The Python logic is genuinely missing in Rust. Summarize the public functions/classes and estimated complexity (Small/Medium/Large).
   - C: Not applicable in Rust (Windows-only, deprecated, or architecturally replaced). Mark with reason.

Output a markdown table with columns: rust_file, py_file, class, notes, complexity.
Do not modify any files.
```

- [ ] **Step 3: Collect and merge classifications**

Wait for all subagents. Merge their outputs into a single classification table at `docs/superpowers/plans/2026-06-16-ccb-daemon-stub-classification.md`.

- [ ] **Step 4: Verify with codegraph**

Use `mcp__codegraph__codegraph_explore` on the top 20 most-referenced Python symbols from the B-class stubs to confirm whether real Rust implementations exist under different names.

---

## Task 1.3: Prioritize B-class stubs for P0/P1

**Files:** `docs/superpowers/plans/2026-06-16-ccb-daemon-stub-classification.md`

- [ ] **Step 1: Identify P0 stubs (end-to-end start → ask → multi-window)**

From the classification, select stubs whose Python functions are called by:
- `lib/ccbd/start_flow.py`
- `lib/ccbd/start_preparation.py`
- `lib/ccbd/handlers/start.py`
- `lib/ccbd/supervision/mount_runtime/starting.py`
- `lib/ccbd/reload_append_layout.py`

Use codegraph_callers on key Python functions to confirm call chains.

- [ ] **Step 2: Identify P1 stubs (reload hot-swap)**

Select stubs called by:
- `lib/ccbd/handlers/project_reload.py`
- `lib/ccbd/reload_apply_service.py`
- `lib/ccbd/reload_transaction_service.py`
- `lib/ccbd/reload_runtime_mount_service.py`

- [ ] **Step 3: Create ranked backlog**

Produce a ranked list of B-class stubs with:
- File path
- Python LOC
- Priority (P0 / P1 / P2)
- Risk (Low/Medium/High)
- Dependencies on other B-class stubs

Save to `docs/superpowers/plans/2026-06-16-ccb-daemon-implementation-backlog.md`.

---

## Task 1.4: Implement P0 start-topology stubs

**Files:** depends on backlog; likely includes:
- Modify: `rust/crates/ccb-daemon/src/start_preparation.rs`
- Modify: `rust/crates/ccb-daemon/src/reload_append_layout.rs`
- Modify: `rust/crates/ccb-daemon/src/supervision/mount_runtime/starting.rs`
- Modify: `rust/crates/ccb-daemon/src/handlers/start.rs`
- Modify: `rust/crates/ccb-daemon/src/app.rs` (if needed to wire new functions)

- [ ] **Step 1: Translate `start_preparation.py` behavior**

Read `lib/ccbd/start_preparation.py`. Implement the equivalent Rust logic in `rust/crates/ccb-daemon/src/start_preparation.rs`, preserving public function names.

- [ ] **Step 2: Translate `reload_append_layout.py` behavior**

Implement multi-window tmux layout expansion based on `.ccb/ccb.config` `[windows]` section. Wire into start flow.

- [ ] **Step 3: Wire `start_flow_runtime_service.rs` to use real preparation/layout**

Replace any hard-coded single-session paths with calls to `start_preparation` and `reload_append_layout`.

- [ ] **Step 4: Add tests**

For each implemented function, add a unit test in `rust/crates/ccb-daemon/tests/` mirroring Python test behavior.

- [ ] **Step 5: Verify**

Run:
```bash
cd /home/agnitum/ccb/rust
cargo test -p ccb-daemon -- --test-threads=1
cargo build --workspace
```

Expected: ccb-daemon tests pass, workspace builds.

---

## Task 1.5: Implement P1 reload hot-swap stubs

**Files:** depends on backlog; likely includes:
- Modify: `rust/crates/ccb-daemon/src/reload_apply_service.rs`
- Modify: `rust/crates/ccb-daemon/src/reload_transaction_service.rs`
- Modify: `rust/crates/ccb-daemon/src/reload_runtime_mount_service.rs`
- Modify: `rust/crates/ccb-daemon/src/handlers/project_reload.rs`

- [ ] **Step 1: Translate `reload_apply_service.py` skeleton**

Implement the reload apply orchestration: plan → transaction → publish.

- [ ] **Step 2: Translate `reload_transaction_service.py` skeleton**

Implement transaction lifecycle: preflight → signature → records → results → publish.

- [ ] **Step 3: Translate `reload_runtime_mount_service.py` skeleton**

Implement additive runtime mount/unmount for config changes that affect running agents.

- [ ] **Step 4: Wire `project_reload.rs` handler**

Ensure the handler calls the real reload service instead of returning a stub response.

- [ ] **Step 5: Add tests**

Add ccb-daemon tests for dry-run reload and live reload scenarios.

- [ ] **Step 6: Verify**

Run:
```bash
cd /home/agnitum/ccb/rust
cargo test -p ccb-daemon -- --test-threads=1
cargo test --workspace -- --test-threads=1 2>&1 | grep -E "FAILED|test result: ok"
```

Expected: no FAILED lines; total passed count increases.

---

## Task 1.6: Clean up ccb-daemon clippy debt

**Files:** all modified ccb-daemon files

- [ ] **Step 1: Run clippy on ccb-daemon**

```bash
cd /home/agnitum/ccb/rust
cargo clippy -p ccb-daemon --no-deps -- -D warnings 2>&1 | tee /tmp/ccb-daemon-clippy.log
```

- [ ] **Step 2: Fix lints in touched files**

Address clippy warnings only in files modified during Phase 1. Do not expand scope to untouched files.

- [ ] **Step 3: Document remaining debt**

If untouched files still have warnings, append them to `docs/superpowers/plans/2026-06-16-ccb-daemon-stub-classification.md` under "Known Debt".

---

## Task 1.7: Update codegraph and verify structural parity

**Files:** none (verification)

- [ ] **Step 1: Sync codegraph**

```bash
/root/.local/bin/codegraph sync
```

- [ ] **Step 2: Spot-check structural parity**

Use `mcp__codegraph__codegraph_explore` to compare 5 critical Python functions (e.g., `ensure_daemon_started`, `build_reload_plan`, `apply_reload`) with their Rust counterparts. Confirm call graphs are aligned.

- [ ] **Step 3: Record parity score**

Document the number of B-class stubs remaining and the number converted to real code in this phase.

---

## Task 1.8: Commit and handoff

**Files:** `docs/superpowers/plans/2026-06-16-ccb-daemon-stub-classification.md`, `docs/superpowers/plans/2026-06-16-ccb-daemon-implementation-backlog.md`

- [ ] **Step 1: Stage changes**

```bash
cd /home/agnitum/ccb
git add rust/crates/ccb-daemon/src/ docs/superpowers/plans/
```

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(ccb-daemon): P0/P1 control-plane parity

- Classified ~280 daemon stubs against Python lib/ccbd/
- Implemented P0 start-topology stubs (start_preparation, reload_append_layout, ...)
- Implemented P1 reload hot-swap stubs (reload_apply_service, reload_transaction_service, ...)
- Added corresponding unit tests
- cargo test -p ccb-daemon passes; cargo build --workspace passes"
```

> Do not `git push` without explicit user approval.

---

## Self-Review

1. **Spec coverage:** This plan covers the ccb-daemon functional gaps identified in `docs/rust-second-draft-plan.md` P0/P1 and `ccb-daemon/README.md` known limitations.
2. **Placeholder scan:** No `TBD` implementation steps. Stub classification and backlog creation are concrete; actual code tasks reference specific files and Python counterparts.
3. **Type consistency:** All tasks preserve existing Rust type/module conventions; new public functions mirror Python names.
4. **Gaps:** This plan intentionally scopes Phase 1 to `ccb-daemon` only. `ccb-providers` and `ccb-cli` consolidation debt will be planned as Phase 2 and Phase 3 after this phase is approved/executed.
