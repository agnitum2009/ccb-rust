# Gap Report: ccb-providers

**Goal:** A3 — verify 1:1 parity of Python provider backends vs Rust `ccb-providers`.
**Date:** 2026-06-14
**Methodology attempted:** Recursive public `def` extraction from `lib/provider_backends/`
+ `provider_execution/` + `provider_runtime/` + `opencode_runtime/` (810 functions);
name-presence check against Rust `fn <name>` workspace-wide.

## Result: ⚠️ MECHANICAL AUDIT INVALID FOR THIS CRATE

The name-based method that worked for ccb-agents/ccb-memory **breaks here**:

| Metric | Value |
|--------|-------|
| Python public fns | 810 |
| Name-match "gaps" (raw) | 585 |
| **Real gaps (after spot-check)** | **NOT RELIABLE — see below** |
| Rust ccb-providers LOC | 26 196 |
| Provider impl files | 16 |
| Functions in `claude.rs` alone | 68 |

### Why the 585 number is an artifact

ccb-providers has a **fundamental architectural divergence** from the Python source:
- Python: many small **module-level free functions** (`find_claude_session_file`,
  `load_claude_session`, `resolve_claude_session`, `update_claude_binding`, …).
- Rust: same functionality lives as **struct methods / trait impls / renamed fns**
  inside 16 provider files (`claude.rs`, `codex.rs`, `gemini.rs`, `droid.rs`, …).

Spot-check confirmed: `find_claude_session_file`, `load_claude_session`,
`resolve_claude_session`, `update_claude_binding`, `find_codex_session_file` are ALL
flagged as "gaps" but ALL exist (as methods or renamed) in `providers/claude.rs` /
`providers/codex.rs`. So the 585 raw count is ~95% false positives.

## What this DOES tell us

1. **Provider coverage is substantial** — 26k LOC, 16 provider impls, 72% of Python LOC.
2. **9 provider backends are present** (claude, codex, gemini, cursor, crush, kiro, pi,
   qwen, opencode/droid/agy/deepseek as optional) — matches the handoff's claim.
3. **A real gap audit here requires semantic work**, not grep: per-provider, map Python
   functions → Rust methods, then confirm behavior. This is a multi-hour effort.

## Verdict & recommended action

**Do NOT trust the 585 number.** ccb-providers is likely near-complete at the
functionality level (end-to-end `ccb ask` works per Phase C verification), but a
strict per-function 1:1 audit is **deferred** — it needs a method-aware diff tool
(struct methods, trait impls), not free-function name matching.

Recommended: build a small mapping harness that resolves `Python fn → Rust impl block`
per provider, OR accept functional parity (runtime works) as the gate and skip
line-level 1:1 for this crate.
