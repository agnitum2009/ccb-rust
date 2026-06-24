# Implement: Wave 1 CLI Phase2Services 架构解锁

## Step 1: 现状审计

- [ ] 列出 `DaemonPhase2Services` 所有返回 `"not yet implemented"` / `"error"` 的方法。
- [ ] 对每个核心命令，确认 Python `cli.phase2` 期望的 payload 字段与 render 输出。
- [ ] 确认 daemon socket client 已支持对应 RPC endpoint（`project_view`、`ping`、`watch`、`stop-all`、`start`、`submit`、`project_restart_agent`、`logs`、`maintenance_tick`、`project_reload_config` 等）。

## Step 2: 实现 Phase2Services

- [ ] 在 `phase2_services.rs` 中补全 `DaemonPhase2Services`：
  - `ps_summary` → 已本地实现，确认可工作。
  - `ping_target` → RPC `ping`。
  - `pend_target` / `queue_target` / `trace_target` / `watch_target` / `inbox_target` / `ack_reply` / `cancel_job` → 对应 RPC。
  - `start_agents` → RPC `start`。
  - `agent_logs` → RPC `logs`。
  - `maintenance_status` → RPC `maintenance_tick`。
  - `reload_config` → RPC `project_reload_config`。
  - `kill_project` → RPC `stop-all`。
  - `submit_ask` / `restart_agent` → 本会话已验证，确认集成。
- [ ] 处理 RPC 错误：返回 `{"error": "...", "status": "error"}` 时，render 层应能展示错误。

## Step 3: 入口路由确认

- [ ] 检查 `src/entry.rs`：v2 命令是否走 `dispatch`。
- [ ] 若非，调整入口，使 phase2 命令优先使用 `DaemonPhase2Services`。
- [ ] 保留 legacy `commands.rs` 作为 fallback，避免破坏现有集成测试。

## Step 4: 测试

- [ ] 在 `crates/ccb-cli/tests/` 新增/扩展：
  - `phase2_services_tests.rs`：用 fake `Phase2Services` 验证 dispatch 调用链。
  - 每个核心命令的 daemon-backed 集成测试（可复用 `cli_integration_tests.rs` 模式）。
- [ ] 断言 render 输出与 Python `cli.phase2` 一致（关键字段、顺序、格式）。

## Step 5: 验证

```bash
cd /home/agnitum/ccb/rust
cargo check --workspace
cargo test -p ccb-cli -- --test-threads=1
```

## Step 6: 文档更新

- [ ] 更新 `plans/rust-python-test-parity-matrix.md` 的 `cli_entrypoint` 行。
- [ ] 若发现新的 CLI/daemon payload 约定，更新 `.trellis/spec/` 或相关代码注释。
