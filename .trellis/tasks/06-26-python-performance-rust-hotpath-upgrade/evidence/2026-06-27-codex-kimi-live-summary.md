# 2026-06-27 Codex + Kimi live verification

Scope: `/mnt/d/dapro-ass` live workspace with temporary config containing only `agent1=codex` and `kimi1=kimi`. Claude was not launched or used for traffic.

Findings:
- Kimi 0.20.1 displays a boxed prompt (`│ > │`) and may keep `K2.7 Code thinking` in the status line even while it accepts input.
- Kimi provider session payload was missing `tmux_socket_path`, so Rust prompt/capture targeted the wrong/default tmux server.
- Kimi native wire log did not expose the CCBR request anchor for the live turn, but the pane contained the submitted request and final `●` reply.

Fixes verified:
- Kimi launch session now persists `tmux_socket_path`.
- Kimi provider recognizes the new boxed prompt and stabilizes it before sending.
- Kimi provider has pane fallback extraction using the visible request line from the pending prompt.

Live evidence:
- Config validation: ok, agents=2, defaults=`agent1,kimi1`.
- Started agents: `agent1 codex %1`, `kimi1 kimi %2`.
- Session socket in `.kimi-kimi1-session`: `/run/user/0/ccbr-runtime/tmux-302a3b148cf7.sock`.
- Ask: `ask kimi1 --from agent1 "Reply exactly: CCBR_KIMI_PANE_FALLBACK_1782565201"`.
- Job: `job_ad27c98bf192`.
- Trace terminal: `completed`.
- Kimi pane contained final token: `CCBR_KIMI_PANE_FALLBACK_1782565201`.

Resource cleanup:
- `/mnt/d/dapro-ass/.ccbr/ccbr.config` restored to the original 3-agent config.
- No `ccbrd`, dapro-ass tmux socket, or dapro-ass test processes remained after cleanup.

Verification commands:
- `cargo test --manifest-path rust/Cargo.toml -p ccbr-providers kimi -- --test-threads=1` → 28 passed.
- `cargo build --manifest-path rust/Cargo.toml -p ccbr-cli -p ccbr-daemon --bins` → passed.
