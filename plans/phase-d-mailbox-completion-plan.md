# Phase D: Mailbox / Completion 计划（已实施）

## 目标
让 `ccb ask <agent> <message>` 能把消息真正投递到 agent 所在的 tmux pane，而不是只入队。

## 已实施方案

### 核心改动
1. **新增 `rust/crates/ccb-daemon/src/handlers/ask.rs`**
   - 注册 `ask` RPC。
   - 从 `AgentRegistry` 查找目标 agent 的 `pane_id`。
   - 通过 `TmuxBackend::send_text` 把消息 body 发到 pane，再补一个 `Enter`。
   - 同时把消息记录进 `JobDispatcher`，返回带 `job_id` 的 receipt。

2. **`rust/crates/ccb-daemon/src/handlers/mod.rs`**
   - 注册 `ask` handler。

3. **`rust/crates/ccb-cli/src/commands.rs`**
   - `ask` 命令从调用 `submit` 改为调用 `ask`。

4. **修复 provider 启动方式（回退到 Phase C 的关键修正）**
   - 原来用 `send-keys` 把启动命令发送到 placeholder 进程，placeholder 是 `sh -lc 'while :; do sleep 3600; done'`，无法接受新命令。
   - 改为 `tmux respawn-pane -k -t <pane> -c <cwd> <command>`，直接替换 pane 进程为 provider CLI。
   - 这样 pane 变成真正的 provider 进程（如 `sh`、`claude`、`opencode`），后续 `ask` 的 `send_text` 才能被正确读取。

5. **测试**
   - `rust/crates/ccb-daemon/src/handlers/ask.rs`：单元测试验证无 pane 时返回错误。
   - `rust/crates/ccb-cli/tests/cli_integration_tests.rs`：集成测试使用真实 tmux backend + `CLAUDE_START_CMD=sh` 验证 `ask` 成功。

## 验证

- `cargo test --workspace --all-targets -- --test-threads=1`：全部通过
- `cargo clippy --workspace --all-targets -- -D warnings`：clean
- `cargo fmt -- --check`：clean
- 手动 E2E：
  - `CODEX_START_CMD=sh ccb start agent1`
  - `ccb ask agent1 --from user "echo ASKED > /tmp/ask-test.txt"`
  - `/tmp/ask-test.txt` 成功写入 `ASKED`

## 已知限制

- 只实现了“投递到 pane”，没有实现同步等待回复；回复仍需要后续阶段通过 session 文件轮询或 provider 事件流获取。
- 多行消息通过 `send_text` 发送，shell 可能只执行最后一行；自然语言单句场景无碍。
