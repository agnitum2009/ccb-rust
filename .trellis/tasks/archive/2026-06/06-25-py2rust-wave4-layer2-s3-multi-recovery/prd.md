# Wave 4 Layer 2 S3: 多 agent + 恢复 live e2e

## Goal
真实 tmux + 真实 provider CLI 下覆盖 S3：(1) 多 agent 交互与跨 agent 回复路由；(2) 故障恢复（pane 死亡/重生、daemon 重启 job 连续性、stale-pane 复用、heartbeat 重注册）。

## Requirements
- 多 agent：agent1/agent2 (codex) + agent3 (claude)。跨 agent ask→reply：agent1 ask agent3、agent3 ask agent1，回复正确落回 asker inbox（不错路由）。
- 恢复-pane：`tmux kill-pane` 杀 provider pane → remain-on-exit 存活 → daemon respawn → agent 恢复可用。
- 恢复-daemon：`ccbr shutdown` 后重启 → 运行中 job 连续性（resume 或干净终止），heartbeat 重注册 Running jobs。
- stale-pane 复用：`ccbr restart <agent>`，验证 pane 复用/重建正确（commit 138ca1ab 回归）。
- 全程真实环境 /mnt/d/dapro-ass。

## Acceptance Criteria
- [ ] 跨 agent ask/reply 路由正确（reply 落 asker inbox），有 trace + inbox 证据
- [ ] pane 死亡→respawn 恢复验证通过
- [ ] daemon 重启后 job 连续性 + heartbeat 重注册验证通过
- [ ] stale-pane 复用回归通过
- [ ] 无 stuck job、无丢失 reply、无 panic

## Notes
- 轻量任务，PRD-only 可启动。运行方式与代码位置见 HANDOFF-KIMI.md。
