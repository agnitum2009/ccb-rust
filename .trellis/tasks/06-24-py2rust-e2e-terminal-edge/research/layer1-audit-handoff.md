# Wave 4 Layer 1 — e2e-terminal-edge 审计 + 交接清单（→ kimi2.7）

> Task: `06-24-py2rust-e2e-terminal-edge`（in_progress）· Layer 1（mock 边界内的全链路 integration parity）· 2026-06-24
> 承接者：**kimi2.7**（glm5.2 审计 + 起草交接；你执行机械主体）

## ⚠ 重要：crate 已 ccbr-*

本任务 prd 写的是旧 `ccb-*` 路径——**实际代码已重命名为 `ccbr-*`**（ccb→ccbr rebrand 完成）。执行时一律用 `ccbr-*`：`ccbr-daemon`、`ccbr-cli`、`ccbr-terminal`、`ccbr-providers`、`ccbr-completion`、`ccbr-runtime-env`、`ccbr-mcp-server`(tools) 等。Python 参考（`lib/`、`test/`）仍是 `ccb`，**别改 Python**。

## 方法（glm5.2 已验证，务必沿用）

对每个 in-scope 领域：**枚举 Python 测试行为 → 映射 Rust[ccbr-*] 实现+测试 → 判定 covered/gap → 缺口走 TDD（移植/重表达 Python 测试先失败 → 实现 → 绿）→ 每领域单独提交 → 更新 parity matrix**。先审后写；mock 边界（FakeTmuxBackend/stub StartFlow/mocked PromptTarget），不做 live provider。

## 逐领域清单（10 个 in-scope）

### 1. 多 agent 会话持久化/恢复（最重，优先）
- Python：`test_v2_ccbd_{keeper,socket,start_flow,supervision_loop,ping_runtime,dispatcher,mount_ownership}.py`
- Rust 目标：`ccbr-daemon/tests/`（新建集成测试）+ `ccbr-daemon/src/reload*.rs` + `services/project_namespace_runtime/` + `ccbr-agents` restore stores
- 现有覆盖：有基建（dispatcher/keeper/reload/supervision/mount 关键词命中 59，但多为单元级；**全链路组装测试缺**）
- 缺口：**跨 daemon 重启的会话恢复全链路**（keeper state 持久化→恢复、reload handoff、socket 全往返含 dispatcher/completion/mailbox、mount ownership 跨重启、supervision loop 端到端）。这是 Layer 1 主体。
- 优先：**P0**（最大、最高价值）

### 2. terminal namespace / pane identity（跨重启）
- Python：`test_ccbd_tmux_{namespace,state}.py`
- Rust：`ccbr-daemon/tests/tmux_runtime_{namespace,state}_tests.rs` + `ccbr-terminal/src/identity.rs`
- 现有：有（单元级 namespace/state）
- 缺口：**daemon 启动/恢复时写 pane identity options + namespace state 跨重启存活**的集成测试
- 优先：P1

### 3. install 运行时（仅 core Rust 函数）
- Python：`test_install_*.py`（9 个，但多数 out-of-scope bash——见 prd out-of-scope 表）
- Rust：`ccbr-cli/src/management_runtime/install.rs` + `ccbr-cli/tests/management_install_tests.rs`
- In-scope 子集：identity/line-endings/tar-safety/source-dev/watchdog/droid-delegation 的 **core Rust 函数**部分（`resolve_installer_paths`/`build_unix_installer_env`/`run_installer`）
- 缺口：core Rust install 函数的 parity 测试（bash 函数部分 out-of-scope，记录到 matrix）
- 优先：P2

### 4. MCP delegation
- Python：`test_mcp_delegation_{server,server_runtime_tools}.py`
- Rust：`rust/tools/ccbr-mcp-server/src/lib.rs` + `tests/`
- 现有：有基建（3 命中）
- 缺口：delegation server + runtime tools 的 parity 测试
- 优先：P1

### 5. sidebar click/resize sync
- Python：`test_sidebar_{click,resize_sync}.py`
- Rust：`ccbr-cli/src/sidebar_{click,resize_sync}.rs` + `ccbr-cli/tests/sidebar_*_tests.rs`
- 现有：有（2 命中）
- 缺口：click/resize sync parity 测试
- 优先：P2

### 6. active runtime polling
- Python：`test_active_runtime_polling.py`
- Rust：`ccbr-completion/` 和/或 `ccbr-providers/` adapter 层（`active_runtime`，kimi 已建）
- 缺口：active runtime polling parity 测试（kimi 的 active_runtime 是否已覆盖？核验）
- 优先：P2

### 7. ask/restart CLI 边界
- Python：`test_{ask_cli,ask_internal_paths,ccb_restart}.py`
- Rust：`ccbr-cli/src/{entry,commands}.rs` + `phase2_runtime/` + `ccbr-cli/tests/`
- 现有：有（ask_service/phase2 14 命中）
- 缺口：ask CLI 边界（--help、内部路径、restart 流）的 parity 测试
- 优先：P1

### 8. runtime env control plane
- Python：`test_runtime_env_control_plane.py`
- Rust：`ccbr-runtime-env/src/control_plane.rs`
- 现有：有（2 命中）
- 缺口：control plane parity 测试
- 优先：P2

### 9. stability regressions
- Python：`test_stability_regressions.py`
- Rust：`ccbr-providers/tests/`（provider log-reader / execution adapter）
- 现有：有（log-reader 9 命中）
- 缺口：稳定性回归用例的 parity（provider 日志读取边界）
- 优先：P2

### 10. matrix/roadmap 更新
- 每领域完成后更新 `plans/rust-python-test-parity-matrix.md` + `.trellis/spec/migration-roadmap.md`；out-of-scope 项记录决策。

## 执行顺序建议

P0（多 agent 恢复）→ P1（terminal 跨重启 / MCP / ask-restart）→ P2（install-core / sidebar / active-runtime / env-control / stability）。每领域：审计→TDD→绿门→提交→matrix。

## 验收

- 每个 in-scope 领域：Python 测试行为有 Rust parity 测试覆盖（移植或重表达），全绿。
- `cargo test --workspace -- --test-threads=1` + `clippy -D warnings` + `fmt --check` 全绿。
- parity matrix + roadmap 按领域更新；out-of-scope 决策记录。
- 产品仓（`agnitum2009/ccb-rust`）用增量 ff-push 同步（见 `/home/agnitum/ccb-rust-prod-workflow.md`，**勿 force**）。

## 护栏（stop-rule）

- mock 边界（不做 live provider CLI——那是 Layer 2）。
- 不改 Python（`lib/`、`test/`）；crate 用 ccbr-*。
- 不碰 ccb-mailbox 线协议语义、provider hook/settings 注入、tmux namespace 核心（仅加测试/补集成）、Phase2Services/ExecutionService trait 契约。
- out-of-scope 项（install.sh bash、windows、wsl、skill templates 等，见 prd 表）**不实现**，仅记录。
- 每领域一提交：`test(e2e): <area> parity`，尾注 `Co-Authored-By: Claude <noreply@anthropic.com>`。

## 参考
- 任务 prd：`.trellis/tasks/06-24-py2rust-e2e-terminal-edge/prd.md`（含完整 in/out-of-scope 表）。
- 方法范本：`.trellis/tasks/06-24-py2rust-providers-daemon-deep/research/consistency-audit-*.md` + `comms-recover-impl-plan.md`。
- 产品仓增量推送流程：`/home/agnitum/ccb-rust-prod-workflow.md`。
- Python 参考：`lib/ccbd/`（含 `test_v2_ccbd_*` 对应的 `lib/ccbd/services/`）、`lib/cli/`。
