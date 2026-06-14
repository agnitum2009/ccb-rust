# Gap Report: ccb-agents

**Goal:** A1 — verify 1:1 parity of Python `agents/` + `rolepacks/` vs Rust `ccb-agents`.
**Date:** 2026-06-14
**Methodology:** Recursive extraction of Python public `def`/`class`/`CONST` across
`lib/agents/` (53 files) + `lib/rolepacks/` (8 files); name-presence check against
Rust `(pub )?fn <name>` / `pub struct` / `pub const` definitions in
`rust/crates/ccb-agents/src/`.

## Result: ✅ NO NAME-LEVEL GAPS (0 of 133 functions)

| Metric | Python | Rust | Note |
|--------|--------|------|------|
| Public functions (`def`) | 133 | — | all 133 have a same-named Rust `fn` |
| Public classes | 13 | — | mapped to `pub struct`/`enum` |
| Constants | ~11 | — | all present (SUPPORTED_ROLE_SCHEMA, ARCHITEC_*, etc.) |
| `pub` items in Rust | — | 198 | exceeds Python surface |
| Source LOC | 5687 | 8934 | Rust > Python (consolidation + tests) |

## Module mapping (confirmed present)

| Python module | pub fns | Rust landing |
|---|---:|---|
| `rolepacks/sources.py` | 18 | `rolepacks.rs` |
| `rolepacks/service.py` | 11 | `rolepacks.rs` (+ `agent_roles_manager.rs`) |
| `rolepacks/runtime_lookup.py` | 11 | `rolepacks.rs` |
| `rolepacks/manifest.py` | 6 | `rolepacks.rs` |
| `rolepacks/agent_roles_manager.py` | 4 | `agent_roles_manager.rs` |
| `agents/policy.py` | 6 | `policy.rs` |
| `agents/runtime_binding.py` | 4 | `runtime_binding.rs` |
| `agents/store.py` | 3 | `store.rs` |
| `agents/models_runtime/*` | ~50 | `models.rs` (consolidated) |
| `agents/config_loader_runtime/*` | ~20 | `config.rs` + `config_identity.rs` |

## Architectural splits (not gaps — by design, see handoff §5/§6)

- `resolve_agent_binding` / `default_binding_adapter` — live in **`ccb-provider-core`**
  (returns `None` default; provider-specific loaders in `ccb-providers`). This is the
  documented dual-crate decision, not a ccb-agents gap.
- `materialize_provider_memory_file` — in `ccb-provider-core` (self-contained due to
  cyclic-dep avoidance).

## Caveats (honest limitations of this audit)

1. **Name-presence ≠ semantic parity.** Same-named Rust `fn` exists, but behavior
   fidelity was NOT diffed line-by-line. A deeper audit would compare docstrings/
   signatures/return shapes.
2. **Internal helpers:** some Python `def` are private helpers; their Rust equivalent
   may be inlined or renamed. These register as "present" by name coincidence in a
   few cases — a full review should distinguish public API from helpers.
3. **`__all__` exports** (12 modules) were not individually diffed against Rust re-exports.

## Verdict

ccb-agents is **name-level 1:1 complete**. Recommended next-level check (optional):
signature/behavior fidelity diff on the 11 highest-traffic functions
(`normalize_role_id`, `load_role_manifest`, `install_role`, `role_status`, etc.).
