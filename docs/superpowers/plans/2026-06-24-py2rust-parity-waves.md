# CCB Python→Rust Parity Migration — Wave Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining Python→Rust parity gaps across 4 dependency-ordered waves, update the parity matrix and migration roadmap, and leave the Rust workspace with zero unaddressed `TODO: align with Python` stubs (except intentionally recorded out-of-scope items).

**Architecture:** The migration follows the dependency chain defined in `.trellis/spec/migration-roadmap.md`: Wave 1 implements the missing `Phase2Services` concrete impl so the CLI dispatch chain becomes end-to-end testable; Wave 2 fills runtime launch, completion, heartbeat, and job-store parity; Wave 3 reduces the bulk of `ccb-providers` and `ccb-daemon` stubs; Wave 4 adds end-to-end session recovery, terminal namespace/pane identity, and maps the 27 unmatched Python tests to Rust equivalents while explicitly retiring out-of-scope items.

**Tech Stack:** Rust 2021 edition, workspace crates (`ccb-cli`, `ccb-daemon`, `ccb-providers`, `ccb-completion`, `ccb-jobs`, `ccb-heartbeat`, `ccb-terminal`, `ccb-runtime-env`, `ccb-mcp-server`), `cargo test -- --test-threads=1`, `cargo clippy --workspace --all-targets`, `cargo fmt --check`.

---

## File structure

| Path | Responsibility |
|------|----------------|
| `.trellis/tasks/06-24-py2rust-remaining-parity/` | Parent Trellis task: scope, design, execution order, final acceptance criteria. |
| `.trellis/tasks/06-24-py2rust-cli-services-impl/` | Wave 1: CLI `Phase2Services` implementation plan and context. |
| `.trellis/tasks/06-24-py2rust-core-parity/` | Wave 2: runtime launch + completion/heartbeat + CLI maintenance plan. |
| `.trellis/tasks/06-24-py2rust-providers-daemon-deep/` | Wave 3: `ccb-providers` and `ccb-daemon` deep stub reduction plan. |
| `.trellis/tasks/06-24-py2rust-e2e-terminal-edge/` | Wave 4: e2e recovery, terminal namespace, and 27 unmatched Python test triage. |
| `plans/rust-python-test-parity-matrix.md` | Source-backed cluster mapping and current parity status. |
| `.trellis/spec/migration-roadmap.md` | Migration baseline, constraints, done criteria, out-of-scope items. |

---

## Task 1: Wave 1 — CLI Phase2Services 架构解锁

**Files:**
- Modify: `rust/crates/ccb-cli/src/phase2_services.rs`
- Modify: `rust/crates/ccb-cli/src/entry.rs` (if v2 routing needs adjustment)
- Test: `rust/crates/ccb-cli/tests/phase2_services_tests.rs` (new)
- Plan: `.trellis/tasks/06-24-py2rust-cli-services-impl/implement.md`

- [ ] **Step 1: Start the Wave 1 Trellis task**

  ```bash
  cd /home/agnitum/ccb
  python3 ./.trellis/scripts/task.py start 06-24-py2rust-cli-services-impl
  ```

- [ ] **Step 2: Implement the concrete `Phase2Services`**

  Follow `.trellis/tasks/06-24-py2rust-cli-services-impl/implement.md` Task 1–4. The core change is to replace the `"not yet implemented"` / `"error"` returns in `DaemonPhase2Services` with real daemon RPC calls (`ping`, `watch`, `stop-all`, `start`, `submit`, `logs`, `maintenance_tick`, `project_reload_config`, etc.).

- [ ] **Step 3: Add integration tests for the core commands**

  Add Rust integration tests covering `ccb ps`, `ccb ping`, `ccb wait`, `ccb kill`, `ccb start`, `ccb ask`, `ccb restart`, `ccb logs`, `ccb maintenance`, `ccb reload`. Each test asserts that `dispatch → Phase2Services impl → render` output matches Python `cli.phase2` behavior.

- [ ] **Step 4: Run targeted tests**

  ```bash
  cd /home/agnitum/ccb/rust
  cargo check --workspace
  cargo test -p ccb-cli -- --test-threads=1
  cargo clippy --workspace --all-targets
  cargo fmt --check
  ```

- [ ] **Step 5: Update the parity matrix**

  Edit `plans/rust-python-test-parity-matrix.md` and move `cli_entrypoint` toward `complete` with the new test mappings.

- [ ] **Step 6: Commit and close Wave 1**

  ```bash
  git add rust/crates/ccb-cli/src/phase2_services.rs rust/crates/ccb-cli/src/entry.rs rust/crates/ccb-cli/tests/phase2_services_tests.rs
  git commit -m "feat(cli): implement concrete Phase2Services and core command dispatch parity"
  python3 ./.trellis/scripts/task.py finish 06-24-py2rust-cli-services-impl
  ```

---

## Task 2: Wave 2 — 核心 parity（runtime launch + completion/heartbeat + CLI maintenance）

**Files:**
- Modify: `rust/crates/ccb-daemon/src/start_runtime/ensure_agent_runtime.rs`
- Modify: `rust/crates/ccb-daemon/src/start_runtime/agent_runtime_binding.rs`
- Modify: `rust/crates/ccb-daemon/src/start_runtime/agent_runtime_models.rs`
- Modify: `rust/crates/ccb-cli/src/services/maintenance.rs`, `rust/crates/ccb-cli/src/commands.rs`, `rust/crates/ccb-cli/src/render_runtime/ops_views_basic.rs`
- Test: `rust/crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs`, `rust/crates/ccb-completion/tests/integration_tests.rs`, `rust/crates/ccb-jobs/tests/store_integration.rs`, `rust/crates/ccb-heartbeat/tests/integration.rs`, `rust/crates/ccb-cli/tests/cli_maintenance_tests.rs`
- Plan: `.trellis/tasks/06-24-py2rust-core-parity/implement.md`

- [ ] **Step 1: Start the Wave 2 Trellis task**

  ```bash
  python3 ./.trellis/scripts/task.py start 06-24-py2rust-core-parity
  ```

- [ ] **Step 2: Implement runtime launch orchestration**

  Add detached fallback, pane minimum-size checks, foreign-binding rejection, and tmux namespace limit handling in `ensure_agent_runtime.rs` and related files. Detailed steps and test code are in the Wave 2 child plan.

- [ ] **Step 3: Fill completion/heartbeat/job-store parity**

  Add `SessionRotate` selector reset test, clean the `ccb-heartbeat/src/classifier.rs` stub, and add job-store record-type filtering test.

- [ ] **Step 4: Implement CLI maintenance orchestration**

  Extend `ccb-cli/src/services/maintenance.rs` with `status`, `tick`, `schedule`, and `runner`; route `commands.rs` to the new service; extend `render_maintenance` for the new output fields.

- [ ] **Step 5: Run targeted tests**

  ```bash
  cd /home/agnitum/ccb/rust
  cargo test -p ccb-daemon -- --test-threads=1
  cargo test -p ccb-completion -- --test-threads=1
  cargo test -p ccb-jobs -- --test-threads=1
  cargo test -p ccb-heartbeat -- --test-threads=1
  cargo test -p ccb-cli -- --test-threads=1
  cargo clippy --workspace --all-targets
  cargo fmt --check
  ```

- [ ] **Step 6: Update the parity matrix**

  Update `runtime_launch`, `completion`, `heartbeat`, and `jobs` rows in `plans/rust-python-test-parity-matrix.md`.

- [ ] **Step 7: Commit and close Wave 2**

  ```bash
  git add rust/crates/ccb-daemon/src/start_runtime rust/crates/ccb-completion rust/crates/ccb-heartbeat rust/crates/ccb-jobs rust/crates/ccb-cli/src/services/maintenance.rs rust/crates/ccb-cli/src/commands.rs rust/crates/ccb-cli/src/render_runtime/ops_views_basic.rs rust/crates/ccb-cli/tests/cli_maintenance_tests.rs
  git commit -m "feat(core): runtime launch orchestration, completion/heartbeat parity, CLI maintenance"
  python3 ./.trellis/scripts/task.py finish 06-24-py2rust-core-parity
  ```

---

## Task 3: Wave 3 — stub 削减（ccb-providers + ccb-daemon 深度 parity）

**Files:**
- Modify: `rust/crates/ccb-providers/src/providers/{codex,claude,gemini,droid,agy,opencode}.rs` and supporting modules
- Modify: `rust/crates/ccb-daemon/src/services/dispatcher_runtime/`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/`, `rust/crates/ccb-daemon/src/reload_*.rs`, `rust/crates/ccb-daemon/src/supervision/`
- Test: `rust/crates/ccb-providers/tests/provider_{codex,claude,gemini,droid,agy,opencode}_tests.rs`, `rust/crates/ccb-daemon/tests/daemon_integration_tests.rs`, `rust/crates/ccb-daemon/tests/reload_tests.rs`, `rust/crates/ccb-daemon/tests/project_namespace_*_tests.rs`
- Plan: `.trellis/tasks/06-24-py2rust-providers-daemon-deep/implement.md`

- [ ] **Step 1: Start the Wave 3 Trellis task**

  ```bash
  python3 ./.trellis/scripts/task.py start 06-24-py2rust-providers-daemon-deep
  ```

- [ ] **Step 2: Triage stubs**

  Run the stub count and classify each `TODO: align with Python` as implement/delete/defer. Record decisions in `.trellis/tasks/06-24-py2rust-providers-daemon-deep/stub-triage.md`.

- [ ] **Step 3: Implement provider adapters (codex, claude, gemini, droid, agy, opencode)**

  Follow the Wave 3 child plan P2–P8. Each provider gets a failing test first, then the adapter implementation, then a passing test.

- [ ] **Step 4: Implement daemon subsystems (dispatcher, namespace, reload, supervision)**

  Follow the Wave 3 child plan D1–D7. Stabilize `ProviderPollResult` / `CompletionItem` shapes first, then build dispatcher lifecycle/polling, submission/routing, finalization/reply delivery, namespace materialization, config reload, and supervision recovery.

- [ ] **Step 5: Run crate and workspace tests**

  ```bash
  cd /home/agnitum/ccb/rust
  cargo check --workspace
  cargo test -p ccb-providers -- --test-threads=1
  cargo test -p ccb-daemon -- --test-threads=1
  cargo test --workspace -- --test-threads=1
  cargo clippy --workspace --all-targets
  cargo fmt --check
  ```

- [ ] **Step 6: Update the parity matrix**

  Move `providers` and `daemon_lifecycle` clusters toward `complete` in `plans/rust-python-test-parity-matrix.md`.

- [ ] **Step 7: Commit and close Wave 3**

  ```bash
  git add rust/crates/ccb-providers rust/crates/ccb-daemon .trellis/tasks/06-24-py2rust-providers-daemon-deep/stub-triage.md
  git commit -m "feat(providers,daemon): deep stub reduction and parity for adapters, dispatcher, namespace, reload, supervision"
  python3 ./.trellis/scripts/task.py finish 06-24-py2rust-providers-daemon-deep
  ```

---

## Task 4: Wave 4 — 端到端恢复与边缘 parity

**Files:**
- Create: `rust/crates/ccb-daemon/tests/e2e_session_recovery_tests.rs`, `rust/tools/ccb-mcp-server/tests/integration_tests.rs`, `rust/crates/ccb-cli/tests/sidebar_click_tests.rs`, `rust/crates/ccb-cli/tests/sidebar_resize_sync_tests.rs`, `rust/crates/ccb-cli/tests/ask_cli_edge_tests.rs`, `rust/crates/ccb-providers/tests/codex_log_reader_stability_tests.rs`
- Modify: `rust/crates/ccb-cli/src/management_runtime/install.rs`, `rust/crates/ccb-cli/src/sidebar_click.rs`, `rust/crates/ccb-cli/src/sidebar_resize_sync.rs`, `rust/crates/ccb-daemon/tests/tmux_runtime_namespace_tests.rs`, `rust/crates/ccb-terminal/src/identity.rs`
- Plan: `.trellis/tasks/06-24-py2rust-e2e-terminal-edge/implement.md`

- [ ] **Step 1: Start the Wave 4 Trellis task**

  ```bash
  python3 ./.trellis/scripts/task.py start 06-24-py2rust-e2e-terminal-edge
  ```

- [ ] **Step 2: Add e2e session recovery tests**

  Add keeper-state, lifecycle, reload handoff, socket round-trip, mount ownership, and supervision recovery tests in `rust/crates/ccb-daemon/tests/e2e_session_recovery_tests.rs`.

- [ ] **Step 3: Add terminal namespace / pane identity tests**

  Extend `tmux_runtime_namespace_tests.rs` and `ccb-terminal/src/identity.rs` inline tests to assert pane identity options (`@ccb_project_id`, `@ccb_role`, etc.) are written and survive reload.

- [ ] **Step 4: Map in-scope unmatched Python tests**

  Implement or add tests for:
  - install parity (`safe_extract_tar`, CRLF normalization)
  - MCP delegation server handlers
  - sidebar click/resize sync
  - active runtime polling
  - ask/restart CLI edge cases
  - runtime env control plane
  - stability regressions

  Detailed code and file names are in the Wave 4 child plan.

- [ ] **Step 5: Record out-of-scope decisions**

  Update `plans/rust-python-test-parity-matrix.md` and `.trellis/spec/migration-roadmap.md` with the 12 retired out-of-scope tests and rationale.

- [ ] **Step 6: Run final workspace validation**

  ```bash
  cd /home/agnitum/ccb/rust
  cargo check --workspace
  cargo test --workspace -- --test-threads=1
  cargo clippy --workspace --all-targets
  cargo fmt --check
  ```

- [ ] **Step 7: Commit and close Wave 4**

  ```bash
  git add rust/crates/ccb-daemon/tests/e2e_session_recovery_tests.rs rust/crates/ccb-daemon/tests/tmux_runtime_namespace_tests.rs rust/crates/ccb-terminal/src/identity.rs rust/crates/ccb-cli/src/management_runtime/install.rs rust/crates/ccb-cli/src/sidebar_click.rs rust/crates/ccb-cli/src/sidebar_resize_sync.rs rust/tools/ccb-mcp-server/tests/integration_tests.rs rust/crates/ccb-cli/tests/sidebar_click_tests.rs rust/crates/ccb-cli/tests/sidebar_resize_sync_tests.rs rust/crates/ccb-cli/tests/ask_cli_edge_tests.rs rust/crates/ccb-providers/tests/codex_log_reader_stability_tests.rs plans/rust-python-test-parity-matrix.md .trellis/spec/migration-roadmap.md
  git commit -m "feat(e2e,edge): session recovery, terminal namespace, and unmatched Python test parity"
  python3 ./.trellis/scripts/task.py finish 06-24-py2rust-e2e-terminal-edge
  ```

---

## Task 5: 父任务收尾与矩阵最终更新

**Files:**
- Modify: `.trellis/tasks/06-24-py2rust-remaining-parity/implement.md`
- Modify: `plans/rust-python-test-parity-matrix.md`, `.trellis/spec/migration-roadmap.md`

- [ ] **Step 1: Verify all child tasks are archived**

  ```bash
  python3 ./.trellis/scripts/task.py list 06-24-py2rust-remaining-parity
  ```
  Expected: all four child tasks show `archived`.

- [ ] **Step 2: Run final workspace validation**

  ```bash
  cd /home/agnitum/ccb/rust
  cargo check --workspace
  cargo test --workspace -- --test-threads=1
  cargo clippy --workspace --all-targets
  cargo fmt --check
  ```

- [ ] **Step 3: Update parent implement.md checklist**

  Mark all Phase 1/Phase 2/Phase 3 checkboxes in `.trellis/tasks/06-24-py2rust-remaining-parity/implement.md` as done.

- [ ] **Step 4: Update migration roadmap current state**

  Edit `.trellis/spec/migration-roadmap.md` to reflect:
  - `Phase2Services` impl exists.
  - workspace stub count is at or near 0 (only intentionally out-of-scope stubs remain).
  - parity matrix clusters are `complete` or explicitly out-of-scope.

- [ ] **Step 5: Archive parent task and finish work**

  ```bash
  python3 ./.trellis/scripts/task.py finish 06-24-py2rust-remaining-parity
  ```

---

## Self-review

1. **Spec coverage:** Every requirement in `.trellis/tasks/06-24-py2rust-remaining-parity/prd.md` is covered by one of Tasks 1–5. Wave 1 maps to the architecture gap; Wave 2–4 map to the remaining partial clusters and unmatched tests; Task 5 handles the final matrix/roadmap updates.
2. **Placeholder scan:** No `TBD`, `TODO`, `implement later`, or `fill in details` remain. Each task names exact files, commands, and the child `implement.md` that contains detailed code steps.
3. **Type consistency:** Task names, crate names, and file paths are consistent with `migration-roadmap.md` and `rust-python-test-parity-matrix.md`.

---

## Validation commands

```bash
cd /home/agnitum/ccb/rust
cargo check --workspace
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets
cargo fmt --check
```

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-24-py2rust-parity-waves.md`.

Two execution options:

1. **Trellis task workflow (recommended for this project)** — start each wave with `python3 ./.trellis/scripts/task.py start <task-id>`, follow the child `implement.md` step-by-step, run the Trellis `check` skill at the end, and archive the task before moving to the next wave.
2. **Subagent-driven execution** — dispatch a fresh subagent per wave using `superpowers:subagent-driven-development`, with two-stage review between waves.

Which approach do you want to use?
