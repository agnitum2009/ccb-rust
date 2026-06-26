# Design: Python latest Rust hotpath replacement through ccb-legacy

## Core Order

Do not use Rust modules as a shortcut to modify `ccbr` first.
Use `ccb-legacy` as the implementation and compatibility line for current Python replacement:

```text
Python latest (/home/agnitum/ccb-git)
  -> golden contract fixtures
  -> ccb-legacy Rust hotpath module
  -> Python-compatible proof
  -> optional ccbr selective intake
```

## Source / Implementation Worktrees

- Python latest source of truth: `/home/agnitum/ccb-git` at the current merged branch.
- Rust replacement implementation line: `/home/agnitum/ccb/ccb-legacy` on `ccb-legacy`.
- Optional later intake target: `/home/agnitum/ccb` `ccbr` branch, only after `ccb-legacy` proof.

## Bloodline Roles

### Python latest

- Source for newest behavior and performance pain.
- Owns current `.ccb` runtime semantics.
- Produces golden fixtures and expected outputs.

### ccb-legacy

- Primary implementation line for current Python-version Rust module replacement.
- Rust compatibility mirror for Python `ccb`.
- Receives and proves the Rust accelerator against Python latest golden contracts.
- Acts as the only bridge gate before optional `ccbr` intake.

### ccbr

- Not the baseline for replacing current Python `.ccb` runtime modules.
- Rust product line with `.ccbr` state and Rust-side architecture.
- May optionally consume proven shared crates after `ccb-legacy` compatibility proof.
- Must not receive `.ccb` runtime assumptions or legacy-only compatibility glue.

## Upgrade Strategy

### Phase 0 — Baseline and contract freeze

- Measure Python latest idle/runtime pain on `/home/agnitum/ccb-git`.
- Freeze golden fixtures per candidate module.
- Record owner surface: input files, output payloads, error behavior, side effects.

### Phase 1 — CPU loop contract and baseline first

Do not start with transcript parser alone. The first Rust replacement must
target the measured CPU sources:

- Python `ccbd` control-plane maintenance loop.
- Per-agent Codex bridge / comm log polling / binding polling.

Freeze the behavior contract before replacing implementation:

- socket submit/cancel/get/watch stays hot
- ask callback/reply delivery order is preserved
- Codex completion/readback events are detected
- idle agents do not consume active polling budget
- Codex hooks remain enabled

### Phase 2 — First Rust replacement: runtime loop accelerator

Target shape:

- one Rust process per project, not one bridge process per Codex agent
- event-driven socket/readiness/log observation where possible
- active-only pane/session polling for running jobs
- optional ~200ms active-job completion checks, never idle-agent tight polling
- Python retains the public CLI/config surface while delegating hot loop work

This is a replacement of the loop owner, not a direct port of Python's polling
implementation.

### Phase 2a — Keep Python-only cheap fixes as supporting work

These are still useful but are not the first Rust replacement:

- no-op JSON write skip
- heartbeat debounce
- keeper no-change save suppression

They reduce disk churn and simplify measurement, but they do not replace the
highest CPU source by themselves.

### Phase 2b — Readback parser belongs inside the runtime accelerator

Provider transcript / JSONL parsing remains a Rust candidate, but its role is
supporting the runtime accelerator:

- input: transcript/session files + anchor/request metadata
- output: structured reply/readback decision
- used by active jobs only
- owned first by `ccb-legacy`, then optionally shared with `ccbr`

### Phase 3 — ccb-legacy implementation gate

Implement the module in `ccb-legacy` after Python latest golden tests define the contract.

Required proof:

- Python fixture output equals Rust fixture output.
- Python-compatible edge cases still match.
- No ccbr-only naming, `.ccbr` pathing, or daemon lifecycle assumptions enter legacy.

### Phase 4 — optional ccbr selective intake

Only after `ccb-legacy` proof, `ccbr` may import the proven module behind its own adapters:

- `.ccbr` path resolver, not `.ccb`.
- Rust daemon state owner, not Python daemon state owner.
- Same behavior contract where public semantics match.
- Allowed divergence only with explicit owner note.

### Phase 5 — next candidates

After the runtime loop accelerator proves lower CPU without ask regressions,
consider:

1. runtime cleanup / cache pruning sidecar
2. tmux/project snapshot batch reader
3. JSON stable-write helper only if Python-only fixes are insufficient
4. provider suspend/resume only after ask stability gates are proven

## Non-Goals

- Do not rewrite the Python daemon in Rust.
- Do not make Python latest call into `ccbrd`.
- Do not merge `ccb-legacy` into `ccbr`.
- Do not disable Codex hooks for performance.
- Do not reproduce Python's per-agent bridge + 0.05s polling model in Rust.
- Do not make idle agent polling part of the compatibility requirement.


## Baseline Scenarios

- Smoke baseline: 2 Codex agents, enough to verify repeatable start/ask/fallback behavior.
- Stress baseline: 4+ Codex agents, matching the user-observed n14-style CPU multiplication.
- Acceptance CPU comparisons use the stress baseline, not only the smoke baseline.
- Measure ccbd, each Codex bridge, provider CLI processes, and sidecar separately.

## Crate / Binary Placement

- Crate path: `rust/crates/ccb-runtime-accelerator`.
- Binary name: `ccb-runtime-accelerator`.
- Workspace: add as a normal runtime crate member, not under `rust/tools/`.
- Reason: this is a long-lived runtime sidecar, not release tooling, and it spans daemon maintenance plus provider hot loops rather than belonging to one provider crate.
- Keep public protocol structs local to the crate first; extract shared crates only after Slice A/B prove stable.

## Sidecar Protocol Decision

- Transport: Unix domain socket.
- Framing: JSON-RPC/JSONL frame.
- Owner: `ccb-legacy` / Python `.ccb` runtime accelerator.
- Non-owner: not `ccbrd`, not `.ccbr` runtime state.
- Lifecycle: Python `ccbd` starts/monitors the sidecar and can fall back to Python polling if the sidecar is unavailable.
- Reason: avoids file polling/write churn, supports long-lived low-latency requests, and matches the local socket style without merging with ccbr.

## Key Compatibility Rule

Every Rust replacement module must have a narrow public contract. The module is portable; each bloodline owns its adapter.

```text
ccb-legacy Rust module: event wait / active poll / parse / decide for Python-compatible .ccb runtime
Python latest fixtures: expected .ccb behavior and CPU baseline
optional ccbr adapter: .ccbr paths and Rust daemon contracts
```

## First Replacement Boundary

The first replacement is a Python `.ccb` runtime sidecar/accelerator, called
something like `ccb-runtime-accelerator` or `ccb-hotloopd`, not `ccbrd`.

Reason:

- It serves Python latest `.ccb` runtime through `ccb-legacy` first.
- It can later share crates with optional `ccbr` adapters.
- It avoids pretending Python latest and ccbr have the same daemon/state owner.

Accepted first-shape decision:

```text
Python ccbd owns socket protocol, job store, mailbox, lifecycle, and public CLI.
Rust accelerator owns hot wait/poll/readback loops for active work.
Python fallback remains available behind an env/config switch.
```

First milestone slices:

```text
Slice 0: ccb-legacy adds sidecar protocol shell, Python fallback, and CPU baseline measurement.
No hot-loop semantics are replaced yet. This proves lifecycle, transport, fallback, and measurement.

Slice A: Python ccbd delegates Codex active-job observation to Rust accelerator.
Rust accelerator watches only submitted/running jobs.
Python still owns socket protocol, job store, mailbox, and public CLI.

Slice B: Python ccbd delegates maintenance wake scheduling / hot wait multiplexing to Rust accelerator.
Fixed 0.2s/1s no-op loops become dirty-event + active-job cadence.

Milestone is not complete until Slice 0 is stable and both Slice A and Slice B reduce their measured CPU source.
```
