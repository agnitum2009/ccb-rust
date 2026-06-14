# Gap Report: ccb-memory

**Goal:** A2 — verify 1:1 parity of Python `memory/` + `project_memory/` vs Rust `ccb-memory`.
**Date:** 2026-06-14
**Methodology:** Recursive public `def` extraction from `lib/memory/` + `lib/project_memory/`;
name-presence check against Rust `(pub )?fn <name>` first in `ccb-memory/src/`, then
workspace-wide (`crates/` + `tools/`) to distinguish real gaps from architectural splits.

## Result: ⚠️ 13 REAL GAPS (of 70 functions); 16 architectural splits

| Metric | Count |
|--------|-------|
| Python public fns | 70 |
| Present in ccb-memory | 41 |
| Split to other Rust crates (by design) | 16 |
| **Real gaps (missing everywhere)** | **13** |
| Rust ccb-memory `pub` items | 58 |

## Real gaps (missing across the whole workspace)

### Cluster 1: context-transfer (8 fns) — **largest gap**
Likely belongs in `ccb-cli` (the `ccb ctx-transfer` command) or a new transfer module.
Mirrors Python `memory/transfer.py` / `lib/cli/ctx_transfer*`.
- `build_context`
- `build_transfer_context`
- `run_transfer`
- `start_transfer_thread`
- `transfer_timestamp`
- `send_to_agent`
- `submit_agent_target`
- `watch_job`

### Cluster 2: small helpers (5 fns)
- `ensure_supported_provider`
- `exit_code_for_status`
- `expand_optional_path`
- `fetch_count`
- `resolved_session_id`

## Architectural splits (present in other crates — NOT gaps)

`agent_private_memory_path`, `ensure_project_memory`, `filter_memory_source`,
`filters_for_source`, `load_memory_sources`, `materialize_runtime_memory_bundle`,
`memory_policy_for_provider`, `project_memory_path`, `provider_native_memory_path`,
`read_memory_source`, `read_seed_metadata`, `render_memory_bundle`,
`runtime_memory_bundle_path`, `seed_metadata_path`, `sha256_text`,
`should_include_source` — these live in `ccb-provider-core` / `ccb-cli` / shared utils.

## Verdict & recommended action

ccb-memory core is largely complete, but the **context-transfer pipeline (8 functions)
has no Rust implementation** — `ccb ctx-transfer` likely runs on a stub or is missing.
Highest-value follow-up: translate `lib/memory/transfer.py` + the ctx-transfer CLI
into a transfer module (suggested landing: `ccb-cli` or a new `ccb-ctx-transfer`).

Helpers (cluster 2) are low-priority small utilities.
