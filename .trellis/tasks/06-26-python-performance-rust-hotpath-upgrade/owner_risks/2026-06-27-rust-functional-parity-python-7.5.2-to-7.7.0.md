# Rust Functional Parity Owner Plan — Python 7.5.2 → 7.7.0 Upgrade

## Boundary correction

Current practical target is no longer "stop at Python `v7.5.2`". The target is:

```text
Python v7.5.2 historical baseline
  -> Python v7.7.0 current local production baseline
  -> ccb-legacy one-by-one Rust-compatible replacement proof
  -> optional ccbr selective intake
```

Reason: this machine has one current production `ccb` environment, now updated to `v7.7.0`; we cannot rely on a separate live `v7.5.2` runtime for acceptance. `v7.5.2` remains the historical diff baseline and prior ccbr parity anchor, not the current runtime acceptance target.

- Python historical evidence: `/home/agnitum/ccb-git` tag `v7.5.2` (`cb97581b`)
- Python current production/source evidence: `/home/agnitum/ccb-git` tag `v7.7.0`, current `HEAD=fdd11024`, `VERSION=7.7.0`
- Rust main evidence: `/home/agnitum/ccb` HEAD `80210c20`
- Owner-method status: owner-risk/work list, not confirmed-owner registry
- Python `7.6.x/7.7.0` additions are now upgrade-intake candidates against the current production baseline, not ignorable future-only items.

## Evidence snapshot: historical 7.5.2 coverage

- Python 7.5.2 daemon handlers: 26.
- Rust daemon handlers: 33.
- Python-only daemon handlers: none.
- Rust-only daemon handlers: `ask`, `cleanup`, `fault_arm`, `fault_clear`, `fault_list`, `logs`, `maintenance_tick`.
- Python 7.5.2 provider backends: 16.
- Rust provider backends: 16.
- Python-only providers: none.
- Python 7.5.2 CLI exposes `wait-any`, `wait-all`, `wait-quorum`; Rust exposes a single `wait` shape.

## Required work to claim 7.5.2 → 7.7.0 upgrade alignment

| Priority | Owner surface | Current status | Work required | Gate to close |
| --- | --- | --- | --- | --- |
| P0 | live wire/payload compatibility | Handler names are covered, but prior handoff says Python sidebar still saw `ccbd unavailable` | Capture actual Python client RPC payload/response against `ccbrd`; fix concrete field-shape mismatch only | Python sidebar connects to `ccbrd` and renders real ProjectView |
| P0 | ProjectView/sidebar projection | Prior owner matrix lists ProjectView dual owner, comms view, namespace/sidebar fields | Keep one ProjectView owner path; verify namespace, agents, comms, sidebar/window fields match Python 7.5.2 expectations | ProjectView schema tests + live sidebar smoke |
| P0 | topology/sidebar materialization | Prior matrix lists start_flow bypassing topology/materialize and missing sidebar pane creation | Ensure `ccbr start` materializes sidebar panes and applies tmux UI like Python 7.5.2 | Live start smoke shows sidebar pane(s), mouse/border metadata, no manual launch |
| P0 | inter-agent communication | Handoff says Codex coordination rules issue remains; Codex hooks must stay enabled | Prove Python-compatible ask flow A→B→A inbox through `ccbrd` without disabling hooks | Live ask smoke: A ask B, B replies, A receives inbox/reply |
| P1 | CLI wait aliases | Python 7.5.2 has `wait-any/all/quorum`; Rust uses `wait` with quorum option | Add aliases or record accepted CLI divergence | Parser/render tests for Python command forms or owner receipt for divergence |
| P1 | provider execution parity | Provider names match 16/16, but owner method requires execution gates, not just names | Verify each provider has manifest + launcher/session/readback gate or explicit unsupported mode | Provider matrix tests remain green for 16 providers |
| P1 | rolepack/current-store parity | Rolepack implementation exists, but latest 7.5.2 behavior needs receipt-level proof | Compare role install/update/current pointer behavior against Python 7.5.2 | Targeted rolepack parity tests |
| P2 | Rust-only extra ops | Rust has extra `ask`, `cleanup`, `fault_*`, `logs`, `maintenance_tick` handlers | Ensure extras do not break Python 7.5.2 clients and are documented as Rust extensions | Negative/compat tests: Python clients ignore/are unaffected |
| P2 | architecture divergence | Rust active-only polling intentionally differs from Python per-agent bridge | Preserve functional events while keeping lower CPU design; do not port Python hot polling | Completion/readback tests + CPU discipline note |

## Current 7.7.0 upgrade-intake blockers / decisions

These were not 7.5.2 parity gaps, but they must now be classified for the 7.7.0 current-production target:

- `zai` provider
- `ccb mobile` / `mobile_gateway`
- `project_sidebar_click` — closed in Rust daemon as a same-name RPC alias using existing ProjectView row resolution + focus planning
- Python 7.7.0 runtime accelerator sidecar and helper family as current Python-production behavior; route through `ccb-legacy` first, not direct `ccbr` import

## Minimal execution order

0. Diff Python `v7.5.2..v7.7.0` by owner surface and freeze which additions are required for current production compatibility.
1. Reproduce/capture Python client RPC against `ccbrd` and fix the exact ProjectView/sidebar payload mismatch.
2. Close sidebar pane materialization live smoke.
3. Close inter-agent ask/inbox live smoke with Codex hooks enabled.
4. Add Python-style `wait-any/all/quorum` only by routing to the existing Phase2 mailbox reply wait service; do not alias them to readiness `wait`.
5. Run provider and rolepack parity gates.

## Non-claims

- This does not mean direct `ccbr` parity with every Python 7.7.0 implementation detail. It means 7.7.0 production behavior must be classified and either proven through `ccb-legacy`, intentionally deferred, or explicitly marked out-of-scope.
- This does not import `ccb-legacy` or Python hot-loop architecture into `ccbr`.
- This does not disable Codex hooks.


## 2026-06-27 CodeGraph upgrade notes

- Python 7.7.0 registers `project_sidebar_click` as a daemon RPC. Rust had CLI-side sidebar click support but no same-name daemon op. Added the daemon op by reusing existing ProjectView + focus planning behavior.
- Python 7.7.0 `wait-any`, `wait-all`, and `wait-quorum` are mailbox reply waits. Rust has an existing Phase2 mailbox wait implementation, but the active CLI `wait` command is readiness-oriented. Therefore these commands must be wired to Phase2 mailbox wait, not treated as aliases for readiness `wait`.
- Rust CodeGraph still has no `zai` or `mobile` symbols; those remain separate 7.7.0 intake surfaces.
