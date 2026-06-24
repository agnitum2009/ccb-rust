# Wave 3 Handoff — `ccb-providers` + `ccb-daemon` Deep Stub Reduction

> Prepared for: `glm5.2` (next implementer)  
> Handoff written by: current session  
> Base branch: `python-rust/rolepacks-versioning-translation`  
> Last commit: `9be27c74` (Wave 2 core parity closed)

## 1. Context

Wave 1 (`py2rust-cli-services-impl`) and Wave 2 (`py2rust-core-parity`) are **complete and committed**.  
Wave 3 is the next dependency-ordered wave: it reduces the remaining `TODO: align with Python` stubs in `ccb-providers` and `ccb-daemon`.

Current stub inventory:

| Crate | Stub files (`TODO: align with Python`) |
|---|---|
| `rust/crates/ccb-providers/src/` | **368** |
| `rust/crates/ccb-daemon/src/` | **345** |
| **Total** | **713** |

## 2. Goal

Implement parity for:

- **Provider execution adapters**: Codex, Claude, Gemini, Droid, AGY, OpenCode.
- **Shared provider infrastructure**: serialization, pane-log support, helper cleanup, instance resolution, session paths, workspace preparation.
- **Daemon dispatcher runtime**: submission/routing, lifecycle/polling, finalization/reply delivery.
- **Daemon subsystems**: project namespace runtime materialization, config reload, supervision/recovery.
- **Daemon top-level stubs**: triage and implement/delete the remaining `src/*.rs` stubs.

Success criteria from `prd.md`:

- `ccb-providers` stubs reduced from **368 → ≤ 50**.
- `ccb-daemon` stubs reduced from **345 → ≤ 50**.
- All provider/daemon targeted tests pass.
- `cargo test --workspace -- --test-threads=1` passes.
- `cargo clippy --workspace --all-targets` has 0 errors.
- `cargo fmt --check` clean.
- `plans/rust-python-test-parity-matrix.md` updated.

## 3. Execution Order (do not reorder)

The canonical plan is `implement.md`. The high-level order is:

1. **P0 Baseline + stub triage** — produce `stub-triage.md` before writing code.
2. **P1 Provider adapter surface** — registry wiring, delete stale `src/agy/` duplicate, remove orphaned `src/mod.rs`.
3. **P2–P7 Provider adapters** — Codex → Claude → Gemini → Droid → AGY → OpenCode.
4. **P8 Shared provider infrastructure** — serialization, pane log support, helper cleanup.
5. **D1–D3 Dispatcher runtime** — submission/routing → lifecycle/polling → finalization/reply delivery.
6. **D4 Namespace runtime** — ensure + topology materialization.
7. **D5 Config reload** — plan/apply/mount transaction.
8. **D6 Supervision** — loop + mount + recovery.
9. **D7 Daemon top-level stubs** — implement or delete triaged stubs.
10. **Z Final validation** — workspace check/test/clippy/fmt + matrix update.

## 4. Where to Start

**Start with P0 triage.**

Run:

```bash
cd /home/agnitum/ccb
echo "ccb-providers stubs: $(grep -rln 'TODO: align with Python' rust/crates/ccb-providers/src/ | wc -l)"
echo "ccb-daemon stubs:    $(grep -rln 'TODO: align with Python' rust/crates/ccb-daemon/src/ | wc -l)"
```

For every stub file, decide:

- **implement** — has a Python reference and is exercised by parity tests.
- **delete** — empty alignment stub, no Python reference, no caller.
- **defer** — Windows/WSL, live CLI integration, or explicitly out-of-scope.

Record decisions in `.trellis/tasks/06-24-py2rust-providers-daemon-deep/stub-triage.md` and commit before coding.

## 5. Key Files to Read First

- `.trellis/tasks/06-24-py2rust-providers-daemon-deep/prd.md` — requirements and acceptance criteria.
- `.trellis/tasks/06-24-py2rust-providers-daemon-deep/implement.md` — step-by-step tasks.
- `.trellis/tasks/06-24-py2rust-remaining-parity/design.md` — 4-wave dependency ordering.
- `.trellis/spec/migration-roadmap.md` — current migration state.
- `plans/rust-python-test-parity-matrix.md` — cluster mappings.

Before touching a sub-theme, read the corresponding Python reference:

- `lib/provider_backends/{codex,claude,gemini,droid,agy,opencode}/`
- `lib/ccbd/services/dispatcher_runtime/`
- `lib/ccbd/services/project_namespace_runtime/`
- `lib/ccbd/reload_apply*.py`
- `lib/ccbd/supervision/`

## 6. Testing Conventions

- Each sub-theme has a dedicated test file or integration test target. Add failing tests **before** implementation (TDD).
- Run targeted tests frequently:
  - `cargo test -p ccb-providers -- --test-threads=1`
  - `cargo test -p ccb-daemon -- --test-threads=1`
- After each sub-theme, run:
  - `cargo check --workspace`
  - `cargo clippy -p ccb-providers -p ccb-daemon --all-targets`
  - `cargo fmt -- --check`
- Final gate:
  - `cargo test --workspace -- --test-threads=1`
  - `cargo clippy --workspace --all-targets`
  - `cargo fmt --check`

## 7. Risks & Stop Rules

Stop and escalate if a change would:

- Modify the ccbd control-plane protocol or socket interface.
- Change provider hook/settings injection paths.
- Alter tmux namespace or pane identity logic beyond the provider adapter surface.
- Require redesigning `Phase2Services` or the `ExecutionService` trait contract.

Do **not** introduce new external crates (`chrono`/`regex`/`reqwest` etc.) if the codebase already has an equivalent pattern.

## 8. Human-Cost Estimate

| Scenario | Effort |
|---|---|
| Optimistic (many stubs are deletable, Python refs are simple) | 25–30 person-days |
| Moderate (normal implementation + debugging) | 35–45 person-days |
| Pessimistic (dispatcher concurrency, tmux integration, protocol edge cases) | 50–65 person-days |

Recommended: do **P0 triage first** to refine this estimate by ~30%.

## 9. Communication / Commits

- Commit per sub-theme (e.g. `feat(providers): codex execution adapter parity`).
- Keep `stub-triage.md` under version control after P0.
- Update `plans/rust-python-test-parity-matrix.md` during **Z** only, after all tests pass.

## 10. Quick Reference Commands

```bash
cd /home/agnitum/ccb/rust

# stub counts
grep -rln 'TODO: align with Python' ../rust/crates/ccb-providers/src/ | wc -l
grep -rln 'TODO: align with Python' ../rust/crates/ccb-daemon/src/ | wc -l

# targeted tests
cargo test -p ccb-providers -- --test-threads=1
cargo test -p ccb-daemon -- --test-threads=1

# final gate
cargo check --workspace
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets
cargo fmt --check
```

---

Good luck. If you hit a contract-level ambiguity, check `prd.md` first; if it is not covered, escalate rather than guess.
