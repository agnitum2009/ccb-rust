> ✅ **已闭环（2026-06-25，glm5.2 续作）** — A/B/C/D 四项全部解决。下方"剩余任务"仅作历史记录。
>
> | 项 | 终态 | 证据 |
> |---|---|---|
> | A · P0 Codex trust | ✅ live 通过 | codex pane 无 trust dialog，直入工作态；隔离 HOME config.toml `trust_level="trusted"` + `--dangerously-bypass-hook-trust` |
> | B · P1 inbox reply | ✅ live 通过 | 中文回复 `收到确认` 无 panic；`inbox --detail` 规范 task_reply 载荷渲染、`--detail` 前置 parser、`ack`（`status: acked`）全正常 |
> | C · ccb-legacy 同步 | ✅ 完成（重定义） | 经澄清"ccbr 与 ccb-legacy 仅命名不同、双生血系永不合并"→ HEAD rust/ 反向重命名 `ccbr→ccb` 同步；提交 `547e91e5`；`cargo check` 干净；与 HEAD 分叉为独立血系 |
> | D · 产品仓 ff-push | ✅ 早已完成 | `agnitum2009/ccb-rust` master @ `6ebd89e`（P0+P1），local == origin/master |
>
> 下方的"剩余任务/关键代码位置/运行方式"保留作历史参考。

---

# Handoff — Wave 4 Layer 2 Live E2E 剩余 → kimi2.7（历史记录）

> Prepared for: **kimi2.7** | From: glm5.2 | Branch: `python-rust/rolepacks-versioning-translation`
> Product repo: `agnitum2009/ccb-rust` (master @ `412512d`)
> 测试环境: `/mnt/d/dapro-ass`（3 agent: codex×2 + claude，非 git 仓，有 `.ccbr/ccbr.config`）

## 已完成（glm5.2 本轮，12 commits）

全链路 **Claude** ask → response → **job COMPLETED** 验证通过。10 个 live e2e 修复：

1. stale-pane reuse 验证（Tmux 检测 → clear + fallthrough）
2. `remain-on-exit on`（panes 存活直到 provider respawn）
3. heartbeat `execution.start` 注册 Running jobs
4. feed `socket_path` 到 execution runtime_state
5. Claude `.claude.json` onboarding 预置（跳过 "Press Enter" 首运行屏幕）
6. feed `pane_id` 到 execution runtime_state
7. feed `prompt_text`（job.request.body）到 runtime_state
8. `poll_pane_text_completion`（Claude adapter：检测 ❯ turn boundary → terminal）
9. codex `auth.json` + `AGENTS.md` 预置 + malformed `config.toml` 修复
10. codex `poll_pane_text_completion_codex` + trust dialog dismissal（raw tmux send-keys）

## 剩余任务（kimi 接手）

### A. Codex trust dialog 未解决 🔴 P0
**问题**：codex 首次启动在隔离 HOME 里显示 "Do you trust this directory? 1. Yes 2. No. Press enter to continue"。
当前代码（codex.rs `poll_pane_text_completion_codex`）尝试用 raw `tmux send-keys Enter` dismiss 但**不生效**。

**诊断方向**：
1. **根因可能是 socket_path/pane_id 未到达 codex adapter 的 runtime_state**——heartbeat feed 用 job_id 匹配，codex 的 job_id 可能与 completion_job.job_id 不一致。
2. **Python 的解决方案**：`prepare_codex_home_overrides` + `materialize_codex_home_config`（`lib/provider_backends/codex/launcher_runtime/command_runtime/home.py`）可能预置了**自动 trust 目录**的配置。查找是否有 `trusted_directories` 相关。
3. **快速 workaround**：在 codex 的 config.toml（隔离 HOME）里写 trust 配置。

### B. Reply delivery 到 inbox 🟠 P1
job completed 但 inbox 显示 pending=0。reply 可能发给 asker 而非 agent3。

### C. ccb-legacy 同步 🟡 P2
所有 10+ live e2e 修复需同步到 `ccb-legacy` 分支（0961a254）。

### D. 产品仓同步 🟢 P3
每次修改后增量 ff-push（`/home/agnitum/ccb-rust-prod-workflow.md`）。

## 关键代码位置

| 组件 | 文件 |
|---|---|
| heartbeat | `ccbr-daemon/src/app.rs:heartbeat()` + `feed_active_pane_text_to_execution()` |
| Claude pane-text completion | `ccbr-providers/src/providers/claude.rs:poll_pane_text_completion()` |
| Codex pane-text completion | `ccbr-providers/src/providers/codex.rs:poll_pane_text_completion_codex()` |
| Provider launcher（HOME 预置） | `ccbr-daemon/src/provider_launcher.rs:build_claude_launch()` + `build_codex_launch()` |
| Start flow（pane 创建 + remain-on-exit） | `ccbr-daemon/src/start_flow/service.rs:execute()` |
| Codex trust（Python 参考） | `lib/provider_backends/codex/launcher_runtime/command_runtime/home.py:prepare_codex_home_overrides()` |

## 运行方式

```bash
export CCBR_SOURCE_RUNTIME_OK=1
cd /home/agnitum/ccb/rust
cargo build -p ccbr-daemon -p ccbr-providers --bin ccbrd
target/debug/ccbrd --project /mnt/d/dapro-ass &
sleep 3
target/debug/ccbr --project /mnt/d/dapro-ass start
target/debug/ccbr --project /mnt/d/apro-ass ask agent1 "hello"
# wait 30s → trace agent1
```
