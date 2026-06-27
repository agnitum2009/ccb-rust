# /mnt/d/dapro-ass Codex resend smoke

Date: 2026-06-27
Project root: `/mnt/d/dapro-ass`
Config: 2 Codex agents + 1 Claude agent
Hook rule: no Codex hook was disabled, masked, or removed.

## Change under test

Codex active delivery now records tmux socket data in `runtime_state` and performs one bounded resend when all are true:

- job is still `pending_anchor`;
- prompt was sent once;
- current request anchor has not been observed;
- target pane shows Codex ready prompt `›`;
- `prompt_resent_after_ready` is still false.

This covers the real `/mnt/d/dapro-ass` failure mode where prompt text can be sent while Codex TUI is still starting, leaving text in terminal scrollback without creating a Codex session event.

## Live result

- `ccbr start` materialized `agent1` Codex, `agent2` Codex, and `agent3` Claude.
- `ask agent1 --from agent2 "Reply exactly: CCBR_DAPRO_RESEND_SMOKE_1782556401"` created `job_a82175ce0592`.
- `trace job_a82175ce0592` reached `completed`.
- `inbox agent2 --detail` contained `job_a82175ce0592 -> "CCBR_DAPRO_RESEND_SMOKE_1782556401"`.
- Pane capture showed the request anchor and final token.
- Shutdown and socket cleanup completed; final process check showed no `/mnt/d/dapro-ass` ccbrd residue.

## Boundary

This closes the single Codex delivery smoke for `/mnt/d/dapro-ass`. It does not yet cover queued multi-ask ordering, callback chains, or cancel/resubmit matrix.
