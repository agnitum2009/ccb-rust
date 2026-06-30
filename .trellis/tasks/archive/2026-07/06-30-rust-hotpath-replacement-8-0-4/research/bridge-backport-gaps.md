# Codex Bridge Backport Gaps: Python v8.0.4 vs Rust `ccb-legacy`

## Scope

This document identifies exactly what the Python `provider_backends.codex.bridge` process does in production v8.0.4 and what is missing from the Rust `ccb-providers` Codex adapter so that the Python bridge process can be eliminated.

## What the Python bridge process does

The bridge is spawned by `lib/provider_backends/codex/launcher_runtime/bridge.py::spawn_codex_bridge()` after the Codex CLI is running in its tmux pane. It runs until the pane dies and performs:

1. **Input transport / FIFO listener** (`runtime_io.py`)
2. **Request processing + ack/history/bridge logging** (`runtime_io.py`)
3. **Pane injection** (`session.py`)
4. **Background session binding tracker** (`binding_runtime.py`)
5. **Diagnostic log filtering** (`launcher_runtime/command_runtime/diagnostics.py`)
6. **Signal handling / lifecycle** (`service.py`, `cli.py`)

## v7.5.2 → v8.0.4 bridge changes

Only three bridge files changed:

| File | v7.5.2..v8.0.4 diff | Why it matters |
|---|---|---|
| `bridge_runtime/service.py` | +14 / -6 | Adds `CodexDiagnosticLogFilterInstaller`, ack cleanup, new poll timeout defaults, closes `fifo_reader` on exit. |
| `bridge_runtime/runtime_io.py` | +223 / -5 | Adds `PersistentFifoReader` with keepalive fd, spool-file support, comm logging, ack writes, bridge-log error logging. |
| `bridge_runtime/binding_runtime.py` | +90 / -10 | Adds comm logging, poll interval 5s, deferred switch-scan signature cache to avoid repeated full scans. |

Other bridge files (`runtime_state.py`, `session.py`, `env.py`, `cli.py`, `bridge.py`, `launcher_runtime/bridge.py`) are unchanged between v7.5.2 and v8.0.4.

## New v8 transport / FIFO abstractions

`provider_core/transport.py` and `provider_core/fifo_delivery.py` are new in v8. They introduce:

- `MessageTransport` contract: `send_line(line)`, `read_line(timeout)`, `close()`.
- `FifoTransport` (POSIX): reader holds FIFO open persistently; writer uses non-blocking open with retry.
- `SpoolDirTransport` (Windows fallback): atomically-renamed `.msg` files.
- `write_fifo_line()` with `CommDeliveryError` on no reader.
- `write_ack()` / `wait_for_ack()` / `cleanup_acks()` / `spool_payload()` for reliable delivery confirmation.
- Payloads > 4096 bytes are spooled to a file and the spool path is sent instead of the JSON body.

The bridge runtime state now uses `create_transport(endpoint_for_fifo_path(paths.input_fifo))` instead of a raw FIFO path.

## Rust `ccb-legacy` status per bridge function

| Python bridge function | Rust equivalent | Status | Notes |
|---|---|---|---|
| `DualBridge.run()` main loop | `ExecutionAdapter::poll()` + `poll_submission()` in `ccb-daemon` | ✅ conceptually | No separate process; daemon polls adapter. |
| Signal handling / graceful exit | `ccbd` ctrl-c / daemon lifecycle | ✅ | Done once at daemon level. |
| `PersistentFifoReader` (keepalive read fd + selector) | None | ❌ missing | Rust `CodexCommunicator::send_async` reopens FIFO write-only on every send; there is no persistent reader to prevent writer blocking/loss. |
| `FifoTransport` / `MessageTransport` | None | ❌ missing | `ccb-provider-core` has no transport abstraction. |
| `SpoolDirTransport` (Windows) | None | ❌ missing | Only needed if Windows support is required. |
| `write_fifo_line()` non-blocking retry | `CodexCommunicator::send_async` | ⚠️ partial | No retry/backoff on `ENXIO`; no `CommDeliveryError`. |
| Spool payload for >4096 bytes | None | ❌ missing | Large ask payloads could fail. |
| `write_ack()` / `wait_for_ack()` | None | ❌ missing | No ack file protocol in Rust. If v8 daemon/ask relies on acks, this must be ported or the dependency removed. |
| `cleanup_acks()` | None | ❌ missing | Housekeeping only. |
| `append_history()` → `history.jsonl` | None | ❌ missing | Rust adapter does not write history. Check if any v8 component reads it. |
| `log_bridge()` → `bridge_log` | None | ❌ missing | Debug artifact; can likely be retired or redirected to tracing. |
| `process_request()` forward to pane | `CodexCommunicator::send_async` / `TmuxBackend::send_text` | ✅ present | Rust sends raw text + newline to FIFO or tmux. |
| `CodexBindingTracker` refresh loop | `refresh_runtime_state()` + `anchor_fallback_log()` | ✅ present | Rust has binding refresh and fallback log scanning. |
| Deferred switch-scan signature cache | None | ❌ missing | Rust re-scans the session root on every refresh; for large session dirs this is expensive and can miss v8 optimizations. |
| `CodexDiagnosticLogFilterInstaller` | None | ❌ missing | No diagnostic log filtering in Rust; Codex CLI noise may pollute pane logs. |
| `TerminalCodexSession.send()` | `TmuxBackend::send_text` / `CodexCommunicator::send_async` | ✅ present | Rust already injects prompts. |
| `read_session_data()`, `session_root()`, `session_work_dir()` | `load_project_session`, `codex_session_root_path`, etc. | ✅ present | Rust has equivalent env/session parsing. |
| `provider_core.comm_logging` | `tracing` macros | ⚠️ partial | Rust uses `tracing`; there is no structured comm event log equivalent. |

## Criticality / effort assessment

| Gap | Criticality for pilot | Effort | Owner / accountability |
|---|---|---|---|
| Persistent FIFO reader / `FifoTransport` | **Must-have** — without it, daemon writing to FIFO can block or lose messages when Codex is slow to open reader. | Medium (1–2 days) | provider integration team |
| `write_ack` / `wait_for_ack` | **Must-have if daemon uses acks** — need to verify v8 ask/forward path. If unused, can retire. | Small if retire; Medium if implement | provider integration team |
| `CodexDiagnosticLogFilterInstaller` | **Should-have** — prevents noisy diagnostic logs from breaking reply parsing. | Small–Medium | terminal/provider integration team |
| Deferred switch-scan signature cache | **Should-have** — avoids repeated expensive scans; important for large projects. | Small | provider integration team |
| Spool payload >4096 bytes | **Should-have** — large asks/artifacts may exceed atomic FIFO limit. | Small | provider integration team |
| `history.jsonl` / `bridge_log` writes | **Can defer** — likely debug/auditing; verify no consumer first. | Small | provider integration team |
| `comm_logging` structured events | **Can defer** — nice for debugging; not blocking functionality. | Small | provider integration team |

## Recommended minimal backport for `ccb_self` pilot

To safely eliminate the Python bridge for one Codex agent, implement in this order:

1. **`ccb-provider-core` transport module**
   - Add `MessageTransport`, `FifoTransport`, `SpoolDirTransport`, `endpoint_for_fifo_path`, `create_transport`.
   - Implement `write_fifo_line()` with non-blocking retry and `CommDeliveryError`.
   - Implement `PersistentFifoReader` in `ccb-providers/src/providers/codex.rs` (or a new `codex/bridge_runtime/` module).

2. **Persistent reader integration**
   - When `CodexExecutionAdapter::start()` creates the input FIFO, also spawn/open a `PersistentFifoReader` held by the adapter.
   - This reader lives in the daemon and consumes any stray writes; the actual user prompts go through `CodexCommunicator::send_async`.

3. **Ack protocol decision**
   - Search v8.0.4 Python daemon/ask code for `wait_for_ack` usage.
   - If acks are required: add `write_ack`/`wait_for_ack` to Rust and wire into the adapter.
   - If unused: document the retirement in the pilot notes.

4. **Diagnostic log filter**
   - Port `CodexDiagnosticLogFilterInstaller` to Rust; run after Codex CLI starts, before first poll.

5. **Deferred switch-scan cache**
   - Port `_switch_scan_signature` and `_should_skip_deferred_switch_scan` into `refresh_runtime_state()`.

6. **Spool payload support**
   - In the ask/daemon send path, detect payload size >4096 bytes and use `spool_payload` + spool-ref message.

## Open verification items

- Does the v8.0.4 daemon/ask path call `wait_for_ack` anywhere?
- Does any v8.0.4 tool read `history.jsonl` or `bridge_log`?
- Does `ccb_self` currently use large payloads (>4096 bytes) that would require spooling?
- What is the exact `CCB_CODEX_BIND_POLL_INTERVAL` in production? (v8 changed default from 0.5s to 5s.)

## References

- Python files at `v8.0.4`:
  - `lib/provider_backends/codex/bridge_runtime/service.py`
  - `lib/provider_backends/codex/bridge_runtime/runtime_io.py`
  - `lib/provider_backends/codex/bridge_runtime/binding_runtime.py`
  - `lib/provider_core/transport.py`
  - `lib/provider_core/fifo_delivery.py`
  - `lib/provider_backends/codex/launcher_runtime/command_runtime/diagnostics.py`
- Rust files in `ccb-legacy`:
  - `rust/crates/ccb-providers/src/providers/codex.rs`
  - `rust/crates/ccb-provider-core/src/`

## Verification of artifact consumers in v8.0.4

Performed after initial gap analysis.

### `wait_for_ack`

`wait_for_ack` is actively used by the Python sender side:

- `lib/provider_backends/codex/comm_runtime/communicator_io_runtime/asking.py:65`
- `lib/provider_backends/codex/comm_runtime/communicator_io_runtime/waiting.py:23`

These are called from `CodexCommunicator._send_message()` (`communicator_facade.py`). The facade is exposed by `provider_backends/codex/comm.py` and used by direct-provider consumers. The Rust `ask`/`ccbr` CLI routes through the daemon socket, but any external Python tool or legacy script that imports `provider_backends.codex.comm` still expects the FIFO/ack contract.

**Implication:** For the bridge elimination, we have two options:

1. **Daemon-only path**: guarantee that all senders go through `ccbd`; then acks are unnecessary because the adapter is in-process.
2. **Keep FIFO/ack contract**: implement `write_ack`/`wait_for_ack` in Rust so that legacy direct-provider callers continue to work.

For a safe pilot on `ccb_self`, option 2 is recommended unless we can prove no caller bypasses the daemon.

### `history.jsonl`

Consumed by:

- `lib/provider_profiles/materializer.py:287,301` — includes `history.jsonl` in provider profile materialization.
- `lib/storage_classification/provider_home.py:39` — classifies `history.jsonl` as a provider-home artifact.

**Implication:** `history.jsonl` is not just a debug file; the provider profile materializer reads it. The Rust adapter must continue writing `history.jsonl` with the same schema, or the materializer must be updated.

### `bridge_log`

No consumers other than the bridge itself and launcher health checks:

- `CODEX_TMUX_LOG` env is set to `artifacts.bridge_log` so tmux `pipe-pane` writes Codex pane output there.
- Launcher checks that `bridge_log` exists before declaring the bridge healthy.

**Implication:** The file path must still exist and tmux must still pipe-pane to it, but it does not need to be a separate `bridge.log`; it can be the same pane log that `ccb-terminal` already writes. The launcher health check may need adjustment if the Python launcher is removed.

### `spool_payload`

Used by `lib/provider_backends/codex/comm_runtime/communicator_io_runtime/asking.py:37` when a payload exceeds `PIPE_ATOMIC_LIMIT` (4096 bytes).

**Implication:** Any sender that may produce large messages must support spooling. The Rust daemon's ask routing should either keep payloads under 4096 bytes or implement spooling.

### Updated recommendation

Because `wait_for_ack` and `history.jsonl` have real consumers, the minimal pilot scope expands slightly:

- Must implement: `FifoTransport`/`PersistentFifoReader`, `write_ack`/`wait_for_ack`, `history.jsonl` writes.
- Should implement: spool payload support, `CodexDiagnosticLogFilterInstaller`, deferred switch-scan cache.
- Can defer: structured `comm_logging` events, `bridge_log` content (path must exist).

## Updated insight: FIFO/ack may not be needed for Rust in-process adapter

After checking how v8.0.4 actually uses the bridge:

- The Python **daemon** routes asks via `provider_execution/common_runtime/terminal.py::send_text_to_pane` directly to the tmux pane. It does **not** write to `input_fifo`.
- `input_fifo` is consumed only by `provider_backends.codex.bridge_runtime` and by `communicator_io_runtime/asking.py` (the direct-provider communicator facade).
- No `lib/cli` or `bin` code in v8.0.4 references `input_fifo` directly.

This means:

- In the Rust architecture, where `ask` already routes through the daemon socket and the daemon sends text directly to the pane, the **entire FIFO/ack/spool bridge IPC can be retired**.
- The `CodexCommunicator` and `input_fifo` paths in `rust/crates/ccb-providers/src/providers/codex.rs` are currently unused/dead code.
- Therefore, the v8.0.4 bridge backports (`PersistentFifoReader`, `FifoTransport`, `SpoolDirTransport`, `write_ack`, `spool_payload`) are **not required for bridge elimination**. They are only needed if we choose to keep a Python-compatible FIFO surface for external direct-provider callers.

### What is still needed for bridge elimination

1. **Do not spawn the Python bridge process** — either by replacing the Python daemon with Rust daemon (which never spawns it), or by patching Python `launcher_runtime/bridge.py` to skip spawn for pilot agents.
2. **Port `CodexDiagnosticLogFilterInstaller`** — this is the only bridge-side function that has runtime side effects beyond prompt forwarding. It prevents Codex diagnostic rows from accumulating in `logs_2.sqlite`.
3. **Ensure bridge artifacts exist for launcher health checks**:
   - `bridge.log` must exist and tmux must pipe-pane to it (or the launcher health check must be relaxed).
   - `bridge.pid` is checked by the Python launcher; if no bridge process, the health check logic must be updated.

### Recommended revised minimal backport

For the `ccb_self` pilot:

- **Rust changes**:
  - Add `CodexDiagnosticLogFilter` to `ccb-providers` (uses `rusqlite`, already in workspace).
  - Optionally remove or deprecate unused `CodexCommunicator` / `input_fifo` code.
- **Python changes** (only for pilot, easily reverted):
  - Patch `lib/provider_backends/codex/launcher_runtime/bridge.py::spawn_codex_bridge` to return early when `CCB_RUST_BRIDGE=1` and agent name is in a configured allow-list.
  - Adjust launcher health check to tolerate missing `bridge.pid` when `CCB_RUST_BRIDGE=1`.

### Rollback for bridge elimination

Because no Rust protocol layer is strictly required, rollback is trivial:

```bash
# Undo the Python patch
mv /path/to/bridge.py.bak /path/to/bridge.py
# Restart the agent
ccbd agent restart ccb_self --project /home/agnitum/o13
```

The Python bridge process will respawn, restoring the pre-pilot behavior.

### Note on transport/fifo_delivery modules

The `ccb-provider-core` transport and `fifo_delivery` modules added during this task provide a faithful Rust mirror of the v8.0.4 Python abstractions. They can be kept for future use (e.g., a Rust-based direct-provider communicator or Windows bridge) but are **not on the critical path** for eliminating the bridge under the Rust daemon.
