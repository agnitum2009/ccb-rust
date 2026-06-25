# S3 Multi-Agent + Recovery Live Verification Summary

Test environment: /mnt/d/dapro-ass (agent1/agent2 codex, agent3 claude)

## ✅ S3.1 多 agent 跨 agent 路由
- `ccbr ask agent3 --from agent1 'hello from agent1'` → reply event `iev_d18aa416c282` landed in agent1 inbox.
- `ccbr ask agent1 --from agent3 'hello from agent3'` → reply event `iev_528fbce71dc1` landed in agent3 inbox.
- Reply routing is correct; no leakage to non-asker.

## ✅ S3.2 pane 死亡 → respawn
- Killed agent1 tmux pane externally (`tmux kill-pane`).
- Daemon heartbeat detects dead pane and auto-respawns:
  - `ccbrd: detected dead pane %7 for agent agent1, will respawn`
  - `ccbrd: respawning agents after pane death: ["agent1"]`
- Manual `ccbr restart agent1` also works (status: ok) and recreates topology.
- After respawn agent1 received a new pane id and `ccbr ask agent1 'ping'` completed.

## ⚠️ S3.3 daemon 重启 + job 连续性
- `ccbr shutdown` + restart ccbrd + `ccbr start` succeeded.
- Running/just-completed job history is not reloaded into daemon memory: `ccbr trace agent3` shows `(no jobs)` immediately after restart.
- Reply events persisted in mailbox inbox; no orphan/stuck job observed.
- Gap: daemon does not persist and re-register running jobs across restart.

## ✅ S3.4 stale-pane 复用回归
- Verified via S3.2: start_flow stale-pane detection (commit 138ca1ab) rejected dead pane id from project-namespace.json and created fresh panes instead of reusing phantom panes.
