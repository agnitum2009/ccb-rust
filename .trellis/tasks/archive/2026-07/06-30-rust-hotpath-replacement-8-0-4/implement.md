# Implementation Plan: Module-level Rust replacement for CCB Python runtime

## Step 0 — Sync and analyze (this session)

- [ ] 0.1 Update `/home/agnitum/ccb-git` to `v8.0.4`.
  - Command: `cd /home/agnitum/ccb-git && git fetch upstream --tags && git checkout v8.0.4` (or merge into working branch).
  - Risk: local branch has uncommitted files `.codegraph/`, `o13-ccb-ops-unit/`. Clean or stash first.
- [ ] 0.2 Generate subsystem diff matrix.
  - For each subsystem (daemon, cli, agents, terminal, mailbox, heartbeat, storage, jobs, provider_backends, provider_execution, provider_core), run:
    - `git diff --stat v7.5.2..v8.0.4 -- <python-path>`
    - Compare with Rust crate path in `ccb-legacy`.
  - Output: `.trellis/tasks/06-30-rust-hotpath-replacement-8-0-4/research/subsystem-diff-matrix.md`
- [ ] 0.3 Validate `ccb-legacy` build.
  - `cd /home/agnitum/ccb && git checkout ccb-legacy && cd rust && cargo build --release`
  - Confirm binaries: `ccb`, `ccbd`, `ask`, `autonew`, `ctx-transfer`, `ccb-provider-activity-hook`, `ccb-provider-finish-hook`, `ccb-cleanup`.

## Step 1 — Low-risk subsystem replacement

- [ ] 1.1 Replace heartbeat engine.
  - Switch daemon heartbeat store writes to use Rust `ccb-heartbeat`.
  - Keep JSON/JSONL schema identical; run `cargo test -p ccb-heartbeat`.
- [ ] 1.2 Replace mailbox/message bureau stores.
  - Use `ccb-mailbox` / `ccb-message-bureau` JSONL stores.
  - Validate with existing mailbox tests.
- [ ] 1.3 Remove `tee` logging subprocesses.
  - Change `lib/terminal_runtime/tmux_logs.py` or equivalent launcher to let `ccb-terminal` write pane logs directly.
  - Verify no orphaned `tee` processes after agent start.
- [ ] 1.4 Run integration smoke.
  - Start a single agent, confirm daemon/heartbeat/mailbox/pane logs functional.
  - Measure RSS before/after.

## Step 2 — Codex provider bridge pilot

- [ ] 2.1 Backport 8.0.4 bridge gaps to Rust.
  - `PersistentFifoReader` keepalive-fd pattern.
  - `CodexDiagnosticLogFilterInstaller`.
  - Deferred switch-scan signature caching.
- [ ] 2.2 Change launcher to not spawn Python bridge.
  - Modify `lib/provider_backends/codex/launcher_runtime/bridge.py::spawn_codex_bridge` (Python side) or equivalent Rust launcher to skip bridge.
  - Ensure `ccbd` registers the Codex adapter for the agent.
- [ ] 2.3 Pick one pilot agent.
  - Recommended: `ccb_self` or `mn_c` (low criticality).
  - Stop agent, replace launcher logic, restart agent.
- [ ] 2.4 Validate pilot.
  - Send ask, confirm reply.
  - Confirm session rebind after Codex resume.
  - Measure bridge process RSS (should be 0) and daemon incremental RSS.
- [ ] 2.5 Roll out to remaining Codex agents one by one.

## Step 3 — Daemon / CLI / keeper consolidation

- [ ] 3.1 Build `ccb-legacy` release.
  - `python scripts/build_release.py` (or direct `cargo build --release`).
- [ ] 3.2 Deploy `ccbd` and `ccb` binaries.
  - Copy to `/root/.local/share/codex-dual/bin/` (or production install path).
  - Backup Python wrappers.
- [ ] 3.3 Stop Python `ccbd`/`keeper`/`ccb.py`, start Rust `ccbd`.
  - Confirm project namespace, tmux session, agent panes survive.
- [ ] 3.4 Regression test.
  - dynamic layout, mobile, reload-drain features (if used).
  - `ask`, `restart`, `reload`, `autonew` CLI commands.

## Step 4 — Verification and finish

- [ ] 4.1 Memory measurement report.
  - `ps -eo pid,comm,rss,args | grep -E 'ccbd|ccb\b|bridge|tee'`
  - Compare with baseline captured in Step 0.
- [ ] 4.2 Run Rust workspace gate.
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo fmt --manifest-path rust/Cargo.toml --all --check`
  - `cargo test --workspace -- --test-threads=1`
- [ ] 4.3 Update parity matrix.
  - `plans/rust-python-test-parity-matrix.md` and `.trellis/spec/migration-roadmap.md`.
- [ ] 4.4 Owner receipt.
  - Attach subsystem owner approvals and non-claims to task receipt.
- [ ] 4.5 Archive task.
  - `python3 ./.trellis/scripts/task.py archive rust-hotpath-replacement-8-0-4`

## Validation commands

```bash
# Build
cd /home/agnitum/ccb && git checkout ccb-legacy
cd rust && cargo build --release

# Test targeted crates
cargo test -p ccb-daemon -- --test-threads=1
cargo test -p ccb-providers -- --test-threads=1
cargo test -p ccb-terminal -- --test-threads=1
cargo test -p ccb-mailbox -p ccb-heartbeat -p ccb-storage -p ccb-jobs

# Full gate
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --manifest-path rust/Cargo.toml --all --check
```

## Risky files / rollback points

- `rust/crates/ccb-providers/src/providers/codex.rs` — bridge replacement core.
- `rust/crates/ccb-daemon/src/socket_server/` — control-plane protocol.
- `rust/crates/ccb-terminal/src/tmux_backend.rs` — pane identity / namespace.
- `lib/provider_backends/codex/launcher_runtime/bridge.py` — Python launcher change.
- `bin/ccb`, `bin/ccbd` — wrapper fallback; keep Python versions during rollout.

## Sub-agent dispatch plan

Use `trellis channel` or parallel `Agent` calls for independent research tasks:

1. **Subsystem diff agent**: compare v8.0.4 Python vs Rust ccb-legacy.
2. **Bridge backport agent**: identify exact 8.0.4 bridge changes to port.
3. **Low-risk module agent**: implement heartbeat/mailbox/tee replacement.
4. **Memory measurement agent**: capture before/after RSS and produce report.

Each agent reads this `implement.md` and the PRD, writes findings to `research/`.
