# Handoff — Wave 4 Layer 2 S3 (multi-agent + recovery) → kimi2.7

> Prepared for: **kimi2.7** | From: glm5.2 | Branch: `python-rust/rolepacks-versioning-translation`
> 测试环境: `/mnt/d/dapro-ass`（3 agent: agent1/agent2 codex + agent3 claude，非 git 仓，有 `.ccbr/ccbr.config`）

## 背景
Layer 2 的 A (codex trust) + B (inbox reply) 已 live 验证通过（归档任务 06-25-py2rust-wave4-layer2-live-e2e / commit `1951583a`）。S3 把 live 覆盖扩到多 agent + 恢复。

## 任务范围（全部 live 验证，证据入 research/）

### 1. 多 agent 跨 agent 路由
- 跨 agent ask（agent1→agent3、agent3→agent1），验证 reply 落到正确 **asker** 的 inbox：`ccbr inbox --detail <asker>`
- 反例回归：reply 不能错路由到非 asker（B 修复正是为此；commit `1951583a` P1）

### 2. pane 死亡 → respawn
- `tmux -S <sock> kill-pane -t <session>:<pane>`（杀 agent1 codex pane）
- remain-on-exit on（commit `2846aa69`）让 pane 存活直到 respawn
- 看 `/tmp/ccbrd.log` + `ccbr status` / `trace agent1`，确认 agent1 恢复
- 再 `ccbr ask agent1 "ping"` 确认可用

### 3. daemon 重启 + job 连续性
- 运行中发起一个 ask（agent3 claude，处理中），然后 `ccbr shutdown` → 重启 ccbrd
- 检查 job 状态（resume 或干净终止），不出现孤儿/stuck
- heartbeat（`ccbr-daemon/src/app.rs:heartbeat()`）应重注册 Running jobs

### 4. stale-pane 复用回归
- `ccbr restart agent1`，验证 pane 复用检测（commit `138ca1ab`）不误用死 pane

## 运行方式（glm5.2 本会话实测）
```bash
export CCBR_SOURCE_RUNTIME_OK=1
cd /home/agnitum/ccb/rust
cargo build -p ccbr-cli -p ccbr-daemon -p ccbr-providers
PROJECT=/mnt/d/dapro-ass
# 干净状态（必做：清遗留 daemon/tmux/socket）
timeout 5 ./target/debug/ccbr --project "$PROJECT" shutdown 2>/dev/null; sleep 2
pkill -f 'ccbrd --project /mnt/d/dapro-ass'; sleep 1
rm -f /run/user/0/ccbr-runtime/ccbrd-*.sock /run/user/0/ccbr-runtime/tmux-*.sock
# 启动
RUST_LOG=info nohup ./target/debug/ccbrd --project "$PROJECT" > /tmp/ccbrd.log 2>&1 & disown
sleep 3
./target/debug/ccbr --project "$PROJECT" start
# 观察（socket/pane 名动态取，勿硬编码）
SOCK=$(ls /run/user/0/ccbr-runtime/tmux-*.sock | head -1)
tmux -S "$SOCK" list-panes -a                       # 找 pane target
tmux -S "$SOCK" capture-pane -t <target> -p         # 抓 pane 文本
./target/debug/ccbr --project "$PROJECT" status
./target/debug/ccbr --project "$PROJECT" trace <agent>
./target/debug/ccbr --project "$PROJECT" inbox --detail <agent>
```

## 关键代码位置
| 组件 | 文件 |
|---|---|
| heartbeat + job 重注册 | `ccbr-daemon/src/app.rs:heartbeat()` + `feed_active_pane_text_to_execution()` |
| remain-on-exit + pane 创建 | `ccbr-daemon/src/start_flow/service.rs:execute()` |
| stale-pane 复用检测 | `start_flow/service.rs`（commit `138ca1ab`） |
| provider 重启 | `ccbr-daemon/src/provider_launcher.rs` |
| inbox/ack 渲染 | `ccbr-cli/src/render.rs` |

## 完成判定
4 类场景全部 live 验证 + 证据入 research/ 或 journal；无 panic/stuck。修复如需同步 ccb-legacy / 产品仓，走既有流程（见 ccb-legacy 非 rust/ 任务 + `/home/agnitum/ccb-rust-prod-workflow.md`）。
