# CCB Rust 1:1 Compatibility — Phase 0: Fix Build & Establish Baseline

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the current `ccb-cli` compilation errors so the entire Rust workspace compiles, then re-establish a clean `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` baseline before any further 1:1 alignment work begins.

**Architecture:** This phase is purely defensive baseline recovery. The root cause is a type-signature mismatch between `ccb-cli/src/services/daemon_runtime/facade.rs` and `keeper.rs` around injected keeper-state closures. We will unify the closure signatures, remove dead/stub code that triggers warnings, and verify with the standard CCB toolchain commands.

**Tech Stack:** Rust 2021, cargo workspace, `serde_json::Value`, closure-based dependency injection in `services/daemon_runtime/`.

---

## File Structure (this phase)

| File | Responsibility |
|---|---|
| `rust/crates/ccb-cli/src/services/daemon_runtime/keeper.rs` | Low-level keeper operations (`wait_for_keeper_ready`, `wait_for_keeper_exit`, `keeper_pid`, `ensure_keeper_started`). |
| `rust/crates/ccb-cli/src/services/daemon_runtime/facade.rs` | Thin facade that re-exports keeper functions with policy defaults. |
| `rust/crates/ccb-cli/src/services/daemon_runtime/shutdown.rs` | Daemon/keeper shutdown orchestration; currently has unused params. |
| `rust/crates/ccb-cli/src/services/daemon_runtime/lease.rs` | Lease helpers used by shutdown. |
| `rust/crates/ccb-cli/src/services/daemon_runtime/models.rs` | Error types (`CcbdServiceError`, `KillSummary`). |

---

## Task 0.1: Diagnose exact closure-type mismatch

**Files:**
- Read: `rust/crates/ccb-cli/src/services/daemon_runtime/keeper.rs`
- Read: `rust/crates/ccb-cli/src/services/daemon_runtime/facade.rs`
- Read: `rust/crates/ccb-cli/src/services/daemon_runtime/shutdown.rs`

- [ ] **Step 1: Reproduce the failure**

Run:
```bash
cd /home/agnitum/ccb/rust
cargo build -p ccb-cli 2>&1 | tee /tmp/ccb-cli-build.log
```

Expected: two `error[E0308]: mismatched types` messages, both in `services/daemon_runtime/`.

- [ ] **Step 2: Record signatures**

From the compiler output and source, record:
1. `keeper::wait_for_keeper_exit<F>` signature: takes one type parameter `F: Fn(&Value) -> bool` for **both** closures.
2. `facade::wait_for_keeper_exit<L, F>` signature: declares two **different** type parameters `L` and `F`, then passes them positionally to keeper.
3. `keeper::wait_for_keeper_ready<F>` signature: same single `F` for both closures.
4. `facade`/`shutdown` call sites pass mismatched closures (`Fn(f64) -> bool` vs `Fn(&Value) -> bool`).

---

## Task 0.2: Unify keeper closure signatures

**Files:**
- Modify: `rust/crates/ccb-cli/src/services/daemon_runtime/keeper.rs:158-210`
- Modify: `rust/crates/ccb-cli/src/services/daemon_runtime/facade.rs:44-70`
- Modify: `rust/crates/ccb-cli/src/services/daemon_runtime/shutdown.rs:114-167` (if needed)

- [ ] **Step 1: Decide a single canonical closure shape**

Use Rust semantics (no Python-style `*args`). Canonical shape for keeper state predicates:

```rust
Fn(&Value) -> bool
```

where the `&Value` is the keeper state object loaded by `keeper_state_load_fn`.

- [ ] **Step 2: Change `wait_for_keeper_ready` to a single type parameter**

In `keeper.rs`, ensure:

```rust
pub fn wait_for_keeper_ready<F>(
    timeout_s: f64,
    keeper_state_load_fn: F,
    keeper_is_running_fn: F,
) -> bool
where
    F: Fn(&Value) -> bool,
```

The implementation already matches this; only the signature text matters.

- [ ] **Step 3: Change `wait_for_keeper_exit` to a single type parameter**

In `keeper.rs`, ensure:

```rust
pub fn wait_for_keeper_exit<F>(
    timeout_s: f64,
    keeper_state_load_fn: F,
    keeper_is_running_fn: F,
) -> bool
where
    F: Fn(&Value) -> bool,
```

- [ ] **Step 4: Update `facade::wait_for_keeper_exit` to one type parameter**

In `facade.rs`, replace:

```rust
pub fn wait_for_keeper_exit<L, F>(
    timeout_s: f64,
    keeper_state_load_fn: L,
    keeper_is_running_fn: F,
) -> bool
where
    L: Fn(&serde_json::Value) -> bool,
    F: Fn(&serde_json::Value) -> bool,
```

with:

```rust
pub fn wait_for_keeper_exit<F>(
    timeout_s: f64,
    keeper_state_load_fn: F,
    keeper_is_running_fn: F,
) -> bool
where
    F: Fn(&serde_json::Value) -> bool,
```

The body stays identical.

- [ ] **Step 5: Update `facade::keeper_pid` if the compiler reports issues**

`keeper_pid` already uses two different closure types (`L: Fn(&Value) -> Value`, `F: Fn(&Value) -> bool`). Keep it as-is; it is independent of the error unless call sites pass wrong arguments.

- [ ] **Step 6: Fix `shutdown.rs` call sites**

The `shutdown_daemon` function takes `wait_for_keeper_exit_fn: E where E: Fn(f64) -> bool`. This is a different shape from the keeper-state predicate. Two options (choose one and apply consistently):

**Option A (recommended — keep facade thin):** Make `shutdown_daemon` expect the same `Fn(&Value) -> bool` predicate and have the caller wrap the timeout. This aligns with keeper's internal shape.

**Option B (keep shutdown simple):** Make `shutdown.rs` pass a closure that ignores state and only uses timeout:

```rust
|_state| wait_for_keeper_exit_fn(shutdown_timeout_s)
```

This requires changing `shutdown.rs` signature to accept `wait_for_keeper_exit_fn` as `Fn(f64) -> bool` and internally adapt to keeper's `Fn(&Value) -> bool`.

For minimal change, apply **Option B**:

In `shutdown.rs` at `_wait_for_keeper_shutdown`:

```rust
if !wait_for_keeper_exit_fn(shutdown_timeout_s) { ... }
```

And at the `shutdown_daemon` call site for `_wait_for_keeper_shutdown`, pass:

```rust
|timeout| wait_for_keeper_exit_fn(timeout),
```

Wait — the current `shutdown_daemon` already has `E: Fn(f64) -> bool` and passes it that way. The mismatch is only in `facade.rs` and `keeper.rs`. Re-check after Step 4; do not over-edit.

- [ ] **Step 7: Fix `keeper.rs:262-266` `wait_for_keeper_ready` call**

The current call:

```rust
wait_for_keeper_ready(
    ready_timeout_s,
    |_| true,
    |_| true,
)
```

matches `Fn(&Value) -> bool` because `|_| true` has type `Fn(&Value) -> bool` when the expected type is that. If the compiler still complains, add explicit type ascription:

```rust
wait_for_keeper_ready(
    ready_timeout_s,
    |_state: &Value| true,
    |_state: &Value| true,
)
```

- [ ] **Step 8: Build and verify**

Run:
```bash
cd /home/agnitum/ccb/rust
cargo build -p ccb-cli 2>&1 | tail -30
```

Expected: `Finished dev [unoptimized + debuginfo] target(s) in ...` with only warnings, no errors.

---

## Task 0.3: Clean up `ccb-cli` warnings

**Files:**
- Modify: `rust/crates/ccb-cli/src/services/daemon_runtime/shutdown.rs:16-46`
- Modify: `rust/crates/ccb-cli/src/services/daemon_runtime/facade.rs:14`
- Modify: any other `ccb-cli` files with warnings reported by the build

- [ ] **Step 1: Prefix or remove unused parameters**

For `shutdown.rs`:

```rust
fn _request_shutdown_or_mark_unmounted<C>(
    inspection: &Value,
    _manager: Value,
    _force: bool,
    client_factory: C,
) -> Result<(), CcbdServiceError>
```

and:

```rust
fn _wait_for_daemon_shutdown<W, T, A>(
    daemon_pid: i64,
    inspection: &Value,
    _manager: Value,
    shutdown_timeout_s: f64,
    ...
)
```

If `_force`/`_manager` are intended to be used in a follow-up phase, keep them prefixed with `_`. Do not delete them unless they are clearly dead.

- [ ] **Step 2: Address `START_TIMEOUT_S` warning**

`pub const START_TIMEOUT_S: f64 = 0.0; // Placeholder...` triggers a dead-code warning if unused. Either:
- Prefix: `pub const _START_TIMEOUT_S: f64 = 0.0;` (if truly placeholder), or
- Use it inside `spawn_ccbd_process` or `policy::startup_transaction_timeout_s()` wrapper.

Recommended: use it as a fallback inside `spawn_ccbd_process`:

```rust
pub fn spawn_ccbd_process<S>(
    spawn_fn: S,
) -> Result<(), CcbdServiceError>
where
    S: Fn(f64) -> Result<(), String>,
{
    let timeout = policy::startup_transaction_timeout_s();
    let timeout = if timeout > 0.0 { timeout } else { START_TIMEOUT_S };
    processes::spawn_ccbd(spawn_fn, timeout)
}
```

- [ ] **Step 3: Re-build and count warnings**

Run:
```bash
cargo build -p ccb-cli 2>&1 | grep -c "warning:"
```

Target: 0 warnings in `ccb-cli`. Other crates may still have warnings; we address only `ccb-cli` in this phase.

---

## Task 0.4: Verify full workspace build

**Files:** none (verification only)

- [ ] **Step 1: Build entire workspace**

Run:
```bash
cd /home/agnitum/ccb/rust
cargo build --workspace 2>&1 | tail -20
```

Expected: `Finished dev [unoptimized + debuginfo] target(s) in ...` for all workspace members.

- [ ] **Step 2: If other crates fail, triage**

If a different crate fails, read its error output and either:
- Fix it in-place if it is a trivial signature/type mismatch (same PR), or
- Add a TODO note to the plan and stop for review if it requires design decisions.

---

## Task 0.5: Run workspace tests

**Files:** none (verification only)

- [ ] **Step 1: Run tests with single thread (CCB convention)**

Run:
```bash
cd /home/agnitum/ccb/rust
cargo test --workspace -- --test-threads=1 2>&1 | tail -40
```

Expected: all tests pass. Record the count: `test result: ok. N passed; 0 failed`.

- [ ] **Step 2: If tests fail, triage**

For each failure:
1. Read the failing test file.
2. Determine if it is a real regression introduced by the signature change or a pre-existing issue.
3. Fix signature-related regressions in this phase.
4. Pre-existing issues go into the Phase 1 follow-up plan.

---

## Task 0.6: Run clippy and format checks

**Files:** none (verification only)

- [ ] **Step 1: Clippy strict pass**

Run:
```bash
cd /home/agnitum/ccb/rust
cargo clippy --workspace -- -D warnings 2>&1 | tail -30
```

Expected: `Finished dev [unoptimized + debuginfo] target(s) in ...` with no warnings promoted to errors.

- [ ] **Step 2: Format check**

Run:
```bash
cargo fmt --check --workspace 2>&1 | tail -20
```

Expected: empty output (no formatting changes needed).

- [ ] **Step 3: If either fails, fix in this phase**

Run `cargo fmt --workspace` to auto-fix formatting. For clippy, fix lints in touched files first; file crate-wide clippy clean-up as a Phase 1 task if it is large.

---

## Task 0.7: Document baseline and commit

**Files:**
- Modify: `docs/superpowers/plans/2026-06-16-ccb-phase0-fix-build.md` (copy of this plan)
- Create or update: `rust/BUILD_STATUS.md` (optional — only if project convention requires)

- [ ] **Step 1: Record baseline metrics**

Capture and save:
```bash
cargo test --workspace -- --test-threads=1 2>&1 | grep "test result" > /tmp/ccb-test-baseline.txt
cargo clippy --workspace -- -D warnings 2>&1 | tail -5 > /tmp/ccb-clippy-baseline.txt
```

- [ ] **Step 2: Commit**

```bash
cd /home/agnitum/ccb
git add rust/crates/ccb-cli/src/services/daemon_runtime/
git commit -m "fix(ccb-cli): unify keeper closure signatures and restore workspace build

- Align facade::wait_for_keeper_exit with keeper::wait_for_keeper_exit single-type-param shape
- Prefix unused shutdown parameters
- Restore cargo build --workspace / cargo test --workspace"
```

> **Note:** Do not run `git push` without explicit user approval.

---

## Task 0.8: Sync codegraph index

**Files:** none

- [ ] **Step 1: Sync local codegraph**

Run:
```bash
/root/.local/bin/codegraph sync
```

Expected: `Already up to date` or successful sync.

- [ ] **Step 2: Verify the fixed symbols are indexed**

Run a quick codegraph search:
```bash
/root/.local/bin/codegraph search wait_for_keeper_exit
```

Expected: results point to `rust/crates/ccb-cli/src/services/daemon_runtime/keeper.rs` and `facade.rs` without stale errors.

---

## Master Roadmap (post-Phase 0)

After this phase, the project will have a clean build/test/clippy baseline. The remaining 1:1 alignment work should be planned as separate, per-subsystem plans:

| Phase | Goal | Primary Crates |
|---|---|---|
| **Phase 1** | 1:1 file alignment for consolidated crates | `ccb-daemon` (reload subsystem flat files), `ccb-providers` (backend/execution/runtime layers), `ccb-cli` (subdirectory structure) |
| **Phase 2** | P0 functional gaps: end-to-end `start → ask → multi-window UI` | `ccb-daemon`, `ccb-terminal`, `ccb-providers`, `ccb-cli` |
| **Phase 3** | P1 functional gaps: control plane parity | `ccb-daemon` (reload hot-swap, keeper integration, ownership lease persistence, runtime adoption) |
| **Phase 4** | P2 functional gaps: CLI parity | `ccb-cli` (`watch` poll loop, `update/uninstall/reinstall`, `tools install/doctor`) |
| **Phase 5** | Full verification | `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, semantic diff against Python `lib/` using codegraph |

---

## Execution Results (2026-06-16)

| Check | Command | Result |
|---|---|---|
| ccb-cli build | `cargo build -p ccb-cli` | ✅ passes, 0 warnings in ccb-cli |
| workspace build | `cargo build --workspace` | ✅ passes (warnings remain in ccb-terminal/ccb-daemon) |
| workspace tests | `cargo test --workspace -- --test-threads=1` | ✅ **1393 passed, 0 failed** |
| ccb-cli clippy | `cargo clippy -p ccb-cli --no-deps -- -D warnings` | ✅ passes |
| workspace clippy | `cargo clippy --workspace -- -D warnings` | ❌ pre-existing debt: ~90 errors in ccb-terminal, 29 warnings in ccb-daemon |
| format | `cargo fmt --check` | ✅ passes |

### Notes discovered during execution

1. **Closure signature fix refined:** The original plan proposed a single `F` type parameter. During compilation the internal call `wait_for_keeper_ready(ready_timeout_s, |_| true, |_| true)` failed because two closures have distinct types. The implemented fix uses two type parameters `F1, F2` for both `wait_for_keeper_ready` and `wait_for_keeper_exit`, matching `facade.rs`'s existing `L, F` declaration.
2. **Helper binary delegation:** `ask`/`autonew`/`ctx-transfer` were looking for a `ccb` binary in `target/debug/`; the workspace builds `ccbr`. Updated to prefer `ccbr` and fall back to `ccb`, and updated `helper_binaries_tests.rs` to expect `ccbr 7.5.2`.
3. **Workspace clippy debt:** Not addressed in this phase because it spans untouched crates (`ccb-terminal`, `ccb-daemon`) and would significantly expand scope. Filed as Phase 1 cleanup work.

---

## Self-Review

1. **Spec coverage:** This plan covers the immediate blocker (build failure) and establishes the baseline required before any Phase 1+ alignment work.
2. **Placeholder scan:** No `TBD`, `TODO`, or vague "add error handling" steps. Each step has an exact file path, command, or code snippet.
3. **Type consistency:** `wait_for_keeper_ready` and `wait_for_keeper_exit` both use type parameters `F1, F2: Fn(&Value) -> bool`. `facade.rs` mirrors this with `L, F`. `shutdown.rs` retains its own `Fn(f64) -> bool` adapter shape.
4. **Gaps:** The master roadmap is intentionally high-level; detailed plans for Phase 1–5 will be written after Phase 0 approval.
