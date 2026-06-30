# Design: Module-level Rust replacement for CCB Python runtime

## Architecture boundary

```text
┌─────────────────────────────────────────────────────────────┐
│  Production CCB v8.0.4 (Python)                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ ccb.py      │  │ ccbd daemon │  │ provider bridges    │  │
│  │ wrapper     │  │ + keeper    │  │ (codex, claude, ...)│  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         │                │                     │             │
│         └────────────────┴─────────────────────┘             │
│                          │ tmux / FIFO / logs               │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│  Replacement target: Rust `ccb-legacy` binaries              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ ccb         │  │ ccbd        │  │ in-process provider │  │
│  │ (CLI)       │  │ (daemon)    │  │ adapters            │  │
│  └─────────────┘  └──────┬──────┘  └─────────────────────┘  │
│                          │                                   │
│         uses ccbr-terminal / ccbr-mailbox / ccbr-storage    │
└─────────────────────────────────────────────────────────────┘
```

## Replacement phases

### Phase 0 — Baseline and diff

1. Sync `/home/agnitum/ccb-git` to `v8.0.4`.
2. Generate subsystem diff matrix (Python v8.0.4 vs Rust ccb-legacy).
3. Mark each subsystem as:
   - `safe_to_replace_now`
   - `needs_backport`
   - `out_of_scope`
   - `blocked_by_protocol`

### Phase 1 — Low-risk infrastructure modules

Replace subsystems that are stable between 7.5.2 and 8.0.4 and have low protocol blast radius.

| Module | Rust crate | Replacement strategy |
|---|---|---|
| `lib/heartbeat/*` | `ccb-heartbeat` | File-backed state store; replace Python heartbeat engine with Rust crate, keep same JSON schema. |
| `lib/mailbox_kernel/*` | `ccb-mailbox` | JSONL store + leasing; replace store reads/writes. |
| `lib/message_bureau/*` | `ccb-message-bureau` | Thin wrapper over mailbox; replace bureau facade. |
| `lib/terminal_runtime/tmux_logs.py` + `tee` | `ccb-terminal` | Write pane logs directly from Rust tmux backend; eliminate `tee` subprocesses. |

### Phase 2 — Provider bridge elimination (Codex pilot)

The Python `provider_backends.codex.bridge` process does four things:

1. **FIFO input listener** (`PersistentFifoReader`) — read daemon→bridge commands.
2. **Request processing** — parse JSON, write ack, log history, forward to Codex pane.
3. **Pane injection** — send text via tmux.
4. **Background binding tracker** — watch `.codex-session`, rebind to new logs, resolve switched sessions.

In Rust, the adapter will be **in-process inside `ccbd`**:

- FIFO writer stays in daemon (`CodexCommunicator::send_async`).
- Log reader / binding tracker moves to `ccb-providers/src/providers/codex.rs::poll_submission()` and `refresh_runtime_state()`.
- Pane injection uses `ccb-terminal::TmuxBackend`.
- Ack/history/bridge_log files are evaluated for compatibility; if nothing in v8.0.4 consumes them, they are retired.

### Phase 3 — Daemon / CLI / keeper consolidation

- Replace Python `ccbd` + `keeper_main.py` + `ccb.py` with `ccbd` + `ccb` Rust binaries.
- Use `ccbd` Unix-socket RPC for CLI commands.
- Keeper logic (reload handoff, lifecycle) is already inside Rust `ccb-daemon`.

## Compatibility guardrails

- **Do not change mailbox/control-plane protocol** without explicit owner approval (AGENTS.md stop rule).
- **Do not change tmux namespace / pane identity logic** without approval.
- **Do not force-push** product repo (`agnitum/ccb-rust`).
- Run single-threaded tests (`--test-threads=1`) for all env/socket tests.

## Data contracts

The following runtime artifact layout must remain compatible:

- `.ccb/ccbd/tmux.sock`
- `.ccb/ccbd/ccbd.sock`
- `.ccb/agents/<agent>/provider-runtime/codex/`
- `.ccb/runtime-accelerator/accelerator.sock`
- `@ccb_*` tmux options (use `ccb-legacy` branch naming).

## Rollback

Each phase must be independently reversible:

- Keep Python `bin/ccb`, `bin/ccbd`, `bin/ccb-provider-*-hook` wrappers as fallbacks.
- Tag release before rollout.
- Per-agent replacement: stop agent, revert launcher to Python bridge, restart.

## Memory targets

| Component | Current Python RSS | Target Rust RSS | Saving |
|---|---|---|---|
| daemon + keeper + wrapper | ~144 MB | ~25 MB | ~120 MB |
| 5× Codex bridge | ~180 MB | ~15 MB incremental | ~165 MB |
| 13× `tee` log processes | ~27 MB | 0 | ~27 MB |
| **Total orchestration** | **~350 MB** | **~40–60 MB** | **~290 MB** |

(Provider LLM binaries ~2.9 GB remain unchanged.)

## Owner receipt gate

Before any phase moves to production:

- Accountable owner for each subsystem signs off on the diff matrix.
- Non-claims are recorded (runtime authority, source adoption, lifecycle truth).
- MX-7C or owning gate review is attached to the task receipt.

## Rollback strategy

Every replacement must be reversible without data loss. The strategy is **feature-flag + binary backup + per-agent opt-in**.

### 1. Binary backup

Before installing Rust release binaries, make timestamped backups of the Python equivalents:

```bash
# On the production host
BIN_DIR="/root/.local/share/codex-dual/bin"
BACKUP_DIR="/root/.local/share/codex-dual/bin/.python-backup-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$BACKUP_DIR"
cp "$BIN_DIR/ccb" "$BIN_DIR/ccbd" "$BIN_DIR/ccb-provider-activity-hook" \
   "$BIN_DIR/ccb-provider-finish-hook" "$BIN_DIR/ask" "$BACKUP_DIR/"
```

### 2. Feature flag for provider bridge

Add an environment variable / agent option that controls whether the Codex adapter runs in-process (Rust) or spawns the legacy Python bridge:

- `CCB_RUST_BRIDGE=1` → Rust in-process adapter.
- unset or `CCB_RUST_BRIDGE=0` → legacy Python bridge.

For the pilot, only set `CCB_RUST_BRIDGE=1` for `ccb_self`. Other agents continue to use the Python bridge. This means the Python bridge code must remain in `lib/provider_backends/codex/bridge_runtime/` and `launcher_runtime/bridge.py`.

### 3. Daemon swap rollback

The Rust daemon `ccbd` uses the same socket/pid/tmux namespaces as the Python daemon. Rollback:

```bash
# Stop Rust daemon
ccbrd stop --project /home/agnitum/o13   # or kill via pidfile

# Restore Python binaries
BACKUP_DIR="..."
cp "$BACKUP_DIR/ccb" "$BACKUP_DIR/ccbd" "$BIN_DIR/"

# Start Python daemon
"$BIN_DIR/ccbd" --project /home/agnitum/o13
```

Because state is stored in `.ccb/` JSON/JSONL files and tmux sessions, rollback preserves running agent panes as long as the state schemas remain compatible.

### 4. Per-agent bridge rollback

If `ccb_self` behaves incorrectly:

```bash
# Stop only ccb_self
ccbrd agent stop ccb_self --project /home/agnitum/o13

# Unset the flag for that agent (or remove from config)
# Restart with legacy bridge
ccbrd agent start ccb_self --project /home/agnitum/o13
```

### 5. Schema compatibility guardrails

- Do not change `.ccb/agents/<agent>/provider-runtime/codex/history.jsonl` schema.
- Keep `acks/` directory layout identical.
- Keep `input_fifo` path and `bridge.log` path identical.
- Do not remove Python bridge source files during the pilot.

### 6. Rollback time target

- Single-agent bridge rollback: < 30 seconds.
- Full daemon rollback: < 2 minutes.
