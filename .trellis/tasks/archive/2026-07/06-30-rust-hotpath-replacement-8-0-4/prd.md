# PRD: Module-level Rust replacement for CCB Python runtime memory reduction

## Goal

Reduce the memory footprint of the CCB orchestration layer by replacing the Python daemon/keeper/wrapper and per-agent Python provider bridges with native Rust binaries from the `ccb-legacy` branch, while preserving compatibility with production CCB v8.0.4 behavior.

## Confirmed facts

- Production CCB runs **v8.0.4** (`upstream/SeemSeam/claude_codex_bridge`, tag `v8.0.4`, commit `e110e267`).
- Local `/home/agnitum/ccb-git` is the production-sync / PR repo. It is currently on branch `codex/claude-callback-capture-repair` at **v7.7.0**, **101 commits behind** `v8.0.4`.
- Rust repo `/home/agnitum/ccb` has two relevant branches:
  - `ccb-legacy`: crate/binary names are `ccb*` (`ccb-daemon` → `ccbd`, `ccb-cli` → `ccb`, etc.), version **7.5.2**.
  - `python-rust/rolepacks-versioning-translation` (current): same 7.5.2 functionality renamed to `ccbr*`.
- `ccb-legacy` is **~30 commits behind** the pre-rename tag and **~270 commits behind** production v8.0.4 (1202 files, ~+214k lines).
- Current production instance memory profile:
  - LLM provider binaries (Claude/Codex) dominate: ~2.9 GB.
  - Python orchestration + provider bridges + `tee` logging: ~350 MB.
  - Each Python `provider_backends.codex.bridge` process: 32–49 MB RSS.
  - Python `ccbd` daemon: 66 MB RSS; keeper: 36 MB; `ccb.py` wrapper: 42 MB.
- `provider_backends.codex.bridge` is a **per-agent subprocess** launched by `launcher_runtime/bridge.py`; it is not hot-loaded.

## Owner-method classification

Per `/mnt/g/owner/CCB_N14_OWNER_DISCOVERY_AND_ADOPTION_METHOD_2026-06-24.md` and `/mnt/g/owner/SOFTWARE_OWNER_DISCOVERY_AND_POSITIONING_METHOD_GENERAL_2026-06-25.md`, every replaced surface must have one Accountable owner and explicit non-claims.

| Subsystem | Surface | Proposed accountable owner | Non-claims |
|---|---|---|---|
| CCB daemon control plane (`ccbd`) | lifecycle/runtime gate | CCB runtime/platform team | no business-domain truth, no provider callbacks |
| Provider bridge (`codex.bridge`) | interface/adapter | CCB provider integration team | no LLM model behavior, no source adoption |
| Terminal/tmux logging (`tee` subprocesses) | capability/infrastructure | CCB terminal runtime team | no business logic, no pane identity authority |
| Heartbeat / mailbox / storage | capability/infrastructure | CCB runtime/platform team | no legal/financial effect, no lifecycle truth |
| `ccb` CLI wrapper | interface | CCB CLI/UX team | no runtime authority, no source adoption |

> Delegation to agents (`mn_c`, `kimi2.7`, etc.) is Responsible implementation only; Accountable owner must still be named before receipt.

## Requirements

1. **Sync production baseline**
   - Update `/home/agnitum/ccb-git` to `v8.0.4` so that the replacement effort targets the real production code.

2. **Subsystem diff matrix**
   - Produce a matrix comparing every Python subsystem in `v8.0.4` with its Rust counterpart in `ccb-legacy`:
     - API/protocol compatibility
     - behavioral gaps
     - file/function-level diff size
     - readiness for replacement

3. **Low-risk replacement candidates**
   - Identify subsystems that are stable between 7.5.2 and 8.0.4 and can be replaced first:
     - heartbeat (`lib/heartbeat/*`)
     - mailbox kernel (`lib/mailbox_kernel/*`)
     - message bureau (`lib/message_bureau/*`)
     - terminal pane logging / `tee` subprocesses (`lib/terminal_runtime/tmux_logs.py`)

4. **Provider bridge replacement design**
   - Eliminate the per-agent Python `provider_backends.codex.bridge` process.
   - Move FIFO read/write, binding tracker, session rebind, and pane injection into the Rust daemon/provider adapter.
   - Backport 8.0.4 bridge changes that are absent from Rust 7.5.2:
     - `PersistentFifoReader` keepalive-fd pattern
     - `CodexDiagnosticLogFilterInstaller`
     - deferred switch-scan signature caching

5. **Build and deployment pipeline**
   - Build `ccb-legacy` Rust release artifacts (`ccbd`, `ccb`, `ask`, provider hooks).
   - Define a safe rollout path that can replace one agent / one daemon at a time with rollback capability.

6. **Validation**
   - Measure per-agent RSS before and after bridge replacement.
   - Confirm no regression in Codex agent behavior (ask, reply, resume, session rebind).
   - Confirm daemon replacement does not break 8.0.4 dynamic layout / mobile / reload-drain features.

## Acceptance criteria

- [ ] `prd.md`, `design.md`, and `implement.md` are reviewed and approved.
- [ ] `/home/agnitum/ccb-git` is at `v8.0.4` (or a documented merge base).
- [ ] Subsystem diff matrix exists and is stored in `.trellis/tasks/06-30-rust-hotpath-replacement-8-0-4/research/`.
- [ ] At least one low-risk subsystem is replaced and passes functional tests.
- [ ] Codex provider bridge replacement is implemented and shows measurable per-agent memory reduction.
- [ ] No regression in production 8.0.4 features for the replaced subsystems.

## Out of scope

- Replacing upstream LLM binaries (Claude/Codex Node processes).
- Replacing the tmux server.
- Replacing external Node MCP servers.
- Full 8.0.4 feature backport into Rust (only the subset needed for the replaced modules).
- Windows/WSL bootstrap or install-script-only tests.

## Open questions

1. Should we target the `ccb-legacy` branch (7.5.2 + ccb naming) or first backport 8.0.4 bridge changes onto `python-rust/rolepacks-versioning-translation` and then rebrand? (Recommended: use `ccb-legacy` naming but backport 8.0.4 bridge/daemon changes.)
2. Do production 8.0.4 features rely on bridge artifacts (`acks/`, `history.jsonl`, `bridge_log`) that the Rust adapter must continue writing, or can they be retired?
3. Which agent is safest for the first Codex bridge replacement pilot?

## References

- `/mnt/g/owner/CCB_N14_OWNER_DISCOVERY_AND_ADOPTION_METHOD_2026-06-24.md`
- `/mnt/g/owner/SOFTWARE_OWNER_DISCOVERY_AND_POSITIONING_METHOD_GENERAL_2026-06-25.md`
- `/mnt/g/owner/owner-method-kit/TRELLIS_INTEGRATION.md`
- `/mnt/g/agent/CLAUDE.md`
- `/home/agnitum/ccb-git` production sync repo
- `/home/agnitum/ccb` Rust repo (`ccb-legacy`, `python-rust/rolepacks-versioning-translation`)
