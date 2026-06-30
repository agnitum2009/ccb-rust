# Rollout Report: Codex bridge elimination across all Codex agents

## Scope

After the `ccb_self` pilot succeeded, the same `.use-rust-bridge` marker was applied to every Codex agent in the project:

- `ccb_self`
- `mn_c`
- `reviewer`
- `archi`
- `mother`
- `coder`

## Procedure

For each agent:

1. Created marker file: `.ccb/agents/<agent>/provider-runtime/codex/.use-rust-bridge`
2. Located the agent's Codex pane PID from tmux via `CODEX_RUNTIME_DIR` environment.
3. Killed the Codex pane process and the legacy Python bridge process.
4. Waited for the daemon to restore the pane.
5. Verified `ccb ping <agent>` reports `health: healthy`.

## Health check results

```text
ccb_self: runtime_state: idle health: healthy
mn_c:     runtime_state: idle health: healthy
reviewer: runtime_state: idle health: healthy
archi:    runtime_state: idle health: healthy
mother:   runtime_state: idle health: healthy
coder:    runtime_state: idle health: healthy
```

## Functionality verification

Sent `ccb ask mn_c "final rollout ping"` and received a normal `task_complete` PASS reply:

```text
status: completed
completion_reason: task_complete
completion_confidence: exact
```

## Memory impact

- **Legacy Python bridge processes remaining: 0**
- **Orchestration RSS (daemon + keeper + wrapper + tee + sidebars): ~194 MB**
- **Estimated memory saved by eliminating 6 Codex bridges: ~180 MB**

Before rollout, 6 Python bridges consumed roughly:

```text
~32 MB/agent × 6 = ~192 MB
```

Those processes are now entirely gone. The remaining orchestration memory is dominated by:

- Python `ccbd` daemon + keeper + `ccb.py` wrapper (~162 MB combined)
- 13 `tee` log subprocesses (~28 MB combined)
- `ccb-agent-sidebar` processes (~17 MB combined)

## Rollback

Per-agent rollback:

```bash
rm /home/agnitum/o13/.ccb/agents/<agent>/provider-runtime/codex/.use-rust-bridge
# Then kill the agent's Codex pane to force a full relaunch with the legacy bridge
```

Full rollback to original `bridge.py`:

```bash
cp /root/.local/share/codex-dual/lib/provider_backends/codex/launcher_runtime/bridge.py.bak.20260701-003554 \
   /root/.local/share/codex-dual/lib/provider_backends/codex/launcher_runtime/bridge.py
# Then restart each agent or restart the daemon
```

## Artifacts changed

- `/root/.local/share/codex-dual/lib/provider_backends/codex/launcher_runtime/bridge.py` (patched)
- `/home/agnitum/o13/.ccb/agents/<agent>/provider-runtime/codex/.use-rust-bridge` (6 marker files)

## Notes

- No Rust binary was deployed to production in this step. The elimination was achieved purely by stopping the legacy Python bridge processes and relying on the existing Python daemon's direct `send_text_to_pane` path.
- The Rust `CodexDiagnosticLogFilter` module is implemented and ready for when the Rust daemon takes over, but it is not currently running because the Python daemon is still in control.
- The `tee` log subprocesses remain; they are the next low-hanging fruit for memory reduction (~28 MB total).
