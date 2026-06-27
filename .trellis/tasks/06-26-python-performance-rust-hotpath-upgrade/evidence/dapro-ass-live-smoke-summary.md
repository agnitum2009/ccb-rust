# /mnt/d/dapro-ass live smoke

Date: 2026-06-27
Project root: `/mnt/d/dapro-ass`
Config: 2 Codex agents + 1 Claude agent from `.ccbr/ccbr.config`
Hook rule: no Codex hook was disabled, masked, or removed.

## First run: failure reproduced

Transient raw logs were captured during the run and reduced to this summary before commit.

- `config validate` passed for `/mnt/d/dapro-ass/.ccbr/ccbr.config`.
- `start` materialized three panes: `agent1` Codex, `agent2` Codex, `agent3` Claude.
- `ask agent1 --from agent2` created `job_f3a6558b6646`.
- Trace incorrectly reached `completed` while the agent pane still contained Codex startup/TUI warning text.
- `inbox agent2 --detail` showed the reply body was the Codex TUI warning text, not the requested token.

Root cause: Codex pane fallback completed a job when it saw the ready prompt `›`, without requiring the current job `request_anchor` to appear in the pane text.

## Fix

File: `rust/crates/ccbr-providers/src/providers/codex.rs`

- `poll_pane_text_completion_codex` now returns `None` when `request_anchor` is set but absent from the pane buffer.
- Added `test_pane_text_completion_waits_for_request_anchor` to lock the startup-warning case.

## After-fix run

Transient raw logs were captured during the run and reduced to this summary before commit.

- `config validate` passed.
- `start` materialized the same three panes.
- `ask agent1 --from agent2` created `job_2826ab34d849`.
- Trace stayed `running` for the observation window instead of falsely completing from TUI startup text.
- Pane capture showed the request anchor `<<BEGIN:req-f7afa450>>`, confirming the prompt reached the pane.
- The job was cancelled after the test; shutdown was requested; final residue check showed no `/mnt/d/dapro-ass` ccbrd process and no ccbr runtime socket entries.

Boundary: this fixes false completion. It does not claim full provider reply delivery is solved; the after-fix job did not produce the requested final token within the bounded observation window.
