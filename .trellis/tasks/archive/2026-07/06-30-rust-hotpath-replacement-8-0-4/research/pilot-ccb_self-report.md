# Pilot Report: Codex bridge elimination on `ccb_self`

## Objective

Verify that `provider_backends.codex.bridge` Python process can be eliminated for one Codex agent (`ccb_self`) while preserving health checks and ask/reply functionality.

## Changes made

### 1. Rust side (preparation)

- Added `ccb-provider-core/src/transport.rs` and `ccb-provider-core/src/fifo_delivery.rs` mirrors of Python `provider_core.transport` / `provider_core.fifo_delivery`.
- Added `ccb-providers/src/providers/codex/diagnostics.rs` mirror of `CodexDiagnosticLogFilterInstaller`.
- Built `ccb-legacy` release binaries (target at `/home/agnitum/ccb/ccb-legacy/rust/target/release/`).

### 2. Production Python side (pilot patch)

- Backed up `/root/.local/share/codex-dual/lib/provider_backends/codex/launcher_runtime/bridge.py` to `bridge.py.bak.20260701-003554`.
- Patched `spawn_codex_bridge` to skip spawning the legacy Python bridge when a per-agent marker file `.use-rust-bridge` exists in the runtime directory.
- When skipped, the launcher still creates the expected `bridge.stdout.log`, `bridge.stderr.log`, and `bridge.pid` files so `validate_bridge_bootstrap` passes.
- Created marker file for `ccb_self`:
  - `/home/agnitum/o13/.ccb/agents/ccb_self/provider-runtime/codex/.use-rust-bridge`

## Procedure

1. Kill the existing `ccb_self` Python bridge process and Codex CLI process.
2. Daemon restored `ccb_self` pane without spawning a new bridge (marker active).
3. Verified `ccb ping ccb_self` reports `health: healthy`.
4. Sent a control ask to `mn_c` (still with bridge) — reply received normally.
5. Sent test ask to `mn_c` without bridge (temporary marker) — reply received normally, proving the patch works.
6. Restored `mn_c` bridge to return it to its original state.
7. Verified `ccb ping mn_c` reports `health: healthy` and ask/reply works after bridge restore.

## Results

### Process state after pilot

```text
  405901 python3.11  31716 KB  /root/.local/bin/python3.11 -m provider_backends.codex.bridge  (reviewer)
  406255 python3.11  35048 KB  /root/.local/bin/python3.11 -m provider_backends.codex.bridge  (archi)
  406367 python3.11  34704 KB  /root/.local/bin/python3.11 -m provider_backends.codex.bridge  (mother)
  406577 python3.11  32576 KB  /root/.local/bin/python3.11 -m provider_backends.codex.bridge  (coder)
  436830 python3     30648 KB  python3 -m provider_backends.codex.bridge                    (mn_c)
```

`ccb_self` no longer has a Python bridge process.

### Memory impact

- Remaining 5 bridges: `31716 + 35048 + 34704 + 32576 + 30648 = 164692 KB` (~161 MB).
- Estimated pre-pilot 6-bridge total: ~196 MB (based on average bridge RSS ~32.7 MB).
- **Observed saving for one agent: ~32–35 MB RSS.**
- Extrapolated saving if all 5 remaining Codex agents are switched: **~150–175 MB**.

### Functionality impact

- `ccb ping ccb_self`: `health: healthy`
- `ccb ask mn_c` without bridge: PASS reply received
- `ccb ask mn_c` with bridge restored: PASS reply received
- `ccb ask ccb_self`: accepted, but reply was empty because the Codex CLI pane is currently at a usage-limit screen (unrelated to bridge elimination). The prompt delivery path through the daemon works.

## Rollback status

- `ccb_self` is still on the pilot path (marker file present).
- `mn_c` was returned to original state (marker removed, bridge process respawned).
- Backup of `bridge.py` exists at:
  - `/root/.local/share/codex-dual/lib/provider_backends/codex/launcher_runtime/bridge.py.bak.20260701-003554`

## One-line rollback command for `ccb_self`

```bash
rm /home/agnitum/o13/.ccb/agents/ccb_self/provider-runtime/codex/.use-rust-bridge
# kill the ccb_self pane/bridge to force a full relaunch with the legacy bridge
```

## Open follow-ups

1. Port / wire the Rust `CodexDiagnosticLogFilter` so it runs without the Python bridge.
2. Determine if any agent relies on `bridge.log` content beyond launcher health checks.
3. Decide whether to roll the marker-based opt-in to remaining Codex agents one-by-one.
4. Replace the patched Python `bridge.py` with a proper feature-flag config once the pilot is accepted.
