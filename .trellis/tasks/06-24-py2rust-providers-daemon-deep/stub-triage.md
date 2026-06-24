# Wave 3 Stub Triage — `ccb-providers` + `ccb-daemon`

> Produced: 2026-06-24 (P0) · Author: glm5.2 · Baseline commit: `fb93ec6c`
> Method: structural + reference + build/test evidence (see §3). **Analysis only — no production code changed.**

## 1. Baseline (measured 2026-06-24)

| Crate | Stub files (`TODO: align with Python`) | All empty? | Build | Tests |
|-------|----------------------------------------|-----------|-------|-------|
| `ccb-providers` | **368** | 367/368 are 3-line; 1 is 121-line (`codex/launcher.rs`) | `cargo check` **green** | **29 passed / 0 failed** (4 binaries) |
| `ccb-daemon`    | **345** | 344/345 are 3-line; 1 is 2-line (`services/job_heartbeat_runtime/models.rs`) | `cargo check` **green** | **26 passed / 0 failed** (5 binaries) |

Every stub is a **1:1 file-alignment placeholder** of the form:

```rust
//! Mirrors Python `lib/<deep/path>/<file>.py`.
//! 1:1 file alignment stub.
// TODO: align with Python
```

— **zero Rust items**. They are not partial implementations; they are structural mirrors of the Python tree.

## 2. Headline finding (read before acting on `implement.md`)

`implement.md` was authored on the premise that these stubs are **half-implemented modules to be completed via TDD** (it references filling specific functions such as `delivery_acceptance_guard`, `poll_exact_hook`, etc.). The evidence contradicts that premise:

1. **Canonical parity already lives in real, large files**, and the existing parity tests **already pass**:
   - providers adapters: `providers/codex.rs` (2260), `claude.rs` (1982), `agy/mod.rs` (1307), `gemini/mod.rs` (1167), `opencode.rs` (546), `droid.rs` (445), plus real `providers/gemini/log_reader.rs` (543), `providers/agy/native_log.rs` (490).
   - daemon cores: `services/project_namespace_runtime/materialize_topology.rs` (1351), `ensure.rs` (519), `reload_apply_service.rs` (421).
2. **No empty stub is load-bearing.** `cargo check` is green, and an empty module cannot satisfy any `name::item` reference without a compile error — therefore any reference to an empty module's path is provably a **cross-crate name collision**, not a real usage (proven in §3.2).
3. **Most deep stubs are not even compiled**: their parent `mod.rs` does not declare them (orphans on disk). E.g. `claude/mod.rs` declares 14 submodules but the dir has 118 child files → **104 orphan files**.

**Conclusion:** the path to the stub-count goal (≤50 each) is **~90% DELETE of redundant empty mirrors**, plus a **small, targeted IMPLEMENT set** for genuinely-unwritten cores (chiefly the daemon dispatcher runtime and supervision loop). This is the opposite emphasis from `implement.md`'s per-stub TDD sweep. The behavioral parity axis (tests) is **largely already satisfied** for providers; the real remaining implementation work is the **daemon dispatcher (D1–D3) and supervision (D6)** cores.

> ⚠ This is a plan-premise deviation. `prd.md` does not cover the "canonical logic already exists, stubs are empty mirrors" case. Per the project stop-rule, **escalate before bulk-deleting** (see §6).

## 3. Method

### 3.1 Shape
- providers: **200 flat top-level** stubs (deep Python files *flattened* to `src/*.rs`, e.g. `active_jobs.rs` ← `codex/session_switch/active_jobs.py`) + **168 in deep subtrees** (claude 76, opencode 22, droid 9, common_runtime 8, paths_runtime/fake_runtime 7 each, service_runtime 6, active_runtime 5, replies_runtime/qwen/pane_log_support/copilot/codebuddy 4 each, agy 3, kimi/deepseek 2, codex 1).
- daemon: **166 flat top-level** + **179 in deep subtrees** (`services/` 101 incl. dispatcher_runtime 63 + project_namespace_runtime 14; `models_runtime` 11; `start_runtime` 9; `supervision` 7; `start_flow_runtime`/`keeper_runtime`/`handlers`/`app_runtime` 6 each; etc.).

### 3.2 Reference analysis (proves "unreferenced")
For every top-level stub module `<name>`, counted `name::` occurrences across `ccb-providers`/`ccb-daemon`/`ccb-cli`:

| Crate | top-level stubs | **0 refs** (confident delete) | 1–2 refs | 3–10 refs | >10 refs |
|-------|-----------------|------------------------------|---------|-----------|---------|
| providers | 200 | **157** | 12 | 18 | 13 |
| daemon    | 166 | **108** | 25 | 18 | 15 |

**Path-qualified proof that >0-ref counts are collision noise:** for the 13 highest-refcount provider names (`env`, `context`, `terminal`, `common`, `files`, `manifest`, `service`, `store`, `registry`, `home`, `start`, `binding`, `events`), `crate::<name>::` / `ccb_providers::<name>::` references **inside the crate = 0 for all 13**. The known-real `codex` module has 6 such references. ⇒ the bare `name::` hits are external (`ccb_terminal::env`, `std::env`, …), and **every empty stub is unreferenced as an item-providing module**.

### 3.3 Module-tree wiring (proves "orphan" for deep stubs)
`<dir>/mod.rs` mod-declarations vs child file count:

| Subtree | mod-decls | child files | orphan (uncompiled) |
|---------|-----------|-------------|---------------------|
| `claude/`        | 14 | 118 | **104** |
| `opencode/`      | 13 | 47  | **34** |
| `droid/`         | 12 | 49  | **37** |
| `codex/`         | 2  | 4   | 2 |
| `common_runtime/`| 7  | 8   | 1 |

The declared-but-empty ones are unreferenced too (§3.2). ⇒ deep stubs are delete-safe regardless of wiring (delete the file; if it was declared, also drop the `mod` line — build stays green).

## 4. Classification

### A — DELETE (redundant empty mirrors; logic exists in canonical files or is out of scope)
- **providers (~355 of 368):** all 200 flat top-level stubs + ~155 deep stubs (claude/opencode/droid/codex/kimi/deepseek/qwen/copilot/codebuddy/fake_runtime/paths_runtime/active_runtime/service_runtime/replies_runtime mirrors). Adapters are real and tests pass; these mirrors duplicate or are unused.
- **daemon (~250 of 345):** all 166 flat top-level (incl. `reload_apply.rs` — keep only if D5 needs a shim; `reload_apply_service.rs` is the real impl) + ~84 deep stubs in `models_runtime`, `start_runtime`, `start_flow_runtime`, `keeper_runtime`, `handlers`, `app_runtime`, `api_models_runtime`, `client_runtime`, `runtime_runtime`, `socket_server_runtime`, `stop_flow_runtime`, `supervisor_runtime`, `project_view`, and the redundant tail of `dispatcher_runtime`/`project_namespace_runtime`.

### B — IMPLEMENT (genuine unimplemented parity; empty core that is the designated home for required logic)
- **daemon dispatcher runtime (D1–D3):** `lifecycle.rs`, `polling_service.rs`, `polling.rs`, `routing.rs`, `callbacks.rs`, `submission*.rs`, `comms_recover.rs`, `finalization*.rs`, `reply_delivery*.rs`, `visible_reply.rs`, `execution_cleanup.rs`, `state*.rs`, `records.rs`, `completion.rs` — **all empty**; this is the real D1–D3 work.
- **daemon supervision (D6):** `supervision/loop_.rs`, `loop_*.rs`, `mount*.rs`, `recovery*.rs`, `backoff.rs`, `cmd_slot.rs` — empty; real D6 work.
- **daemon namespace/reload tail (D4/D5):** `project_namespace_runtime/additive_patch*.rs`, `topology_plan.rs`, `backend.rs`, `slot_replacement.rs`; `reload_apply.rs`/`reload_transaction*.rs`/`reload_patch_*.rs` — **only the subset D4/D5 actually need**; the rest collapse into A.
- **providers supporting (only if canonical adapter must delegate):** `common_runtime/serialization.rs`, `pane_log_support/lifecycle_recovery.rs`, `claude/comm_runtime/polling.rs`. **Verify need first** — canonical files may already inline this logic, in which case → A.

### C — DEFER (Windows/WSL, live-CLI, out-of-scope per `prd.md`)
- Any stub mirroring Windows bootstrap / WSL path utils / live provider-CLI integration / Python wrapper-script internals. Identify during the per-subtree delete pass; tag in this doc and leave the stub.

## 5. Revised execution recommendation (vs `implement.md`)

| Step | `implement.md` assumed | Recommended (evidence-based) |
|------|------------------------|------------------------------|
| P0 | triage | **done (this doc)** |
| P1 | registry + delete dup `src/agy/` + `src/mod.rs` | **confirm & do** (small, correct) |
| P2–P7 | fill provider adapter stubs | **verify existing tests cover prd behaviors; delete redundant mirrors; implement only gaps** (adapters already real) |
| P8 | shared infra | **implement only B-set items that canonical code needs; delete rest** |
| D1–D3 | dispatcher TDD | **genuine IMPLEMENT** (cores are empty) — primary remaining work |
| D4 | namespace materialize | **mostly done** (materialize_topology/ensure real); implement additive-patch tail |
| D5 | reload | **mostly done** (reload_apply_service real); implement transaction/patch tail |
| D6 | supervision | **genuine IMPLEMENT** (loop_ empty) |
| D7 | top-level triage | **= bulk DELETE (set A)** — the main stub-count lever |
| Z | matrix update | **confirm & do** |

Expected outcome: providers 368 → ≤50 via ~355 deletes + ≤13 B-set; daemon 345 → ≤50 via ~250 deletes + ~40 B-set (D1–D3/D6) + ~50 defer.

## 6. Escalation (decision needed before bulk delete)

`prd.md` does not contemplate this state. Before deleting ~600 files (hard to reverse), confirm:

1. **Strategy A (recommended):** bulk-DELETE redundant mirrors (set A) to hit stub-count ≤50, and IMPLEMENT only the genuine empty cores (daemon D1–D3 dispatcher, D6 supervision, plus D4/D5 tails and any provider B-set proven necessary). Treat providers adapters as done (tests green).
2. **Strategy B:** keep all stubs as future scaffolding; implement per `implement.md` literally (fill every stub). Slower; stub-count goal not met without later deletes.
3. **Scope of daemon dispatcher parity:** is D1–D3 full dispatcher parity in-scope for Wave 3, or is stub-reduction the priority and dispatcher parity deferred to a Wave 4? (Sets the size of B.)

## 7. Verification commands used

```bash
grep -rln 'TODO: align with Python' rust/crates/ccb-providers/src/ | wc -l   # 368
grep -rln 'TODO: align with Python' rust/crates/ccb-daemon/src/    | wc -l   # 345
cargo check -p ccb-providers -p ccb-daemon                                  # green
cargo test -p ccb-providers -- --test-threads=1                            # 29 passed / 0 failed
cargo test -p ccb-daemon    -- --test-threads=1                            # 26 passed / 0 failed
```
