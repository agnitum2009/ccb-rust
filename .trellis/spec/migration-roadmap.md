# Python→Rust Migration Roadmap

> 整体迁移的计划基准。驱动后续 Trellis 任务创建与优先级决策。source-backed：所有数字/路径来自 2026-06-22 扫描。

## Current state (2026-06-24)

- **workspace stub**: **1101**（含 `TODO: align`），较 2026-06-22 基准 1218 减少 117。分布：
  - `ccb-providers` 368, `ccb-daemon` 345（合计 ~65%）
  - `ccb-cli` 158, `ccb-agents` 73, `ccb-memory` 53, `ccb-provider-core` 46
  - `ccb-mailbox` 19, `ccb-completion` 11, 其余 < 10
- **parity matrix**: 多个集群已标 `complete`（runtime_env、stdio_runtime、doctor_runtime、diagnostics_bundle、cleanup_service、kill_runtime_agent_cleanup、kill_service、ask_service、runtime_launch、management_cleanup、storage_paths、agents_roles、heartbeat、memory、mailbox、config_project、types_i18n 等），`partial` 集群集中在 `cli_entrypoint`、`daemon_lifecycle`、`providers`、`completion`、`terminal_runtime`。Wave 4 Layer 1 新增 `e2e-terminal-edge` 集群：`terminal_namespace_identity`、`install_core`、`mcp_delegation`、`sidebar_click_resize`、`active_runtime_polling`、`ask_restart_cli_edge`、`runtime_env_control_plane`、`stability_regressions` 已关闭；`multi_agent_recovery` 为 `partial`（P0-c..h 缺口仍存）。
- **已闭环/已提交 Trellis 任务**:
  - `06-24-py2rust-cli-services-impl`（Wave 1，`Phase2Services` 架构解锁，commit `9be27c74` 包含）。
  - `06-24-py2rust-core-parity`（Wave 2，runtime launch / completion SessionRotate / heartbeat classifier / jobs store filter，commit `9be27c74` 包含）。
- **daemon_runtime 接口完整**（`crates/ccb-cli/src/services/daemon_runtime/` stub=0）。
- **Wave 3 已准备**: `06-24-py2rust-providers-daemon-deep` 状态 `ready`，并附有 `HANDOFF.md` 供 `glm5.2` 接手。

## ⚠️ Architecture gap（最高风险，已解除）

`Phase2Services` trait 在 Wave 1 已实现为 `DaemonPhase2Services`，`dispatch → handle_xxx → render_xxx` 链已打通。剩余风险转移至 **provider execution adapters** 和 **daemon dispatcher runtime** 的 parity 完整性。

## Roadmap（4 waves, dependency-ordered）

### Wave 1 — 架构解锁（最高优先级，杠杆点，已完成）
- **Task**: `06-24-py2rust-cli-services-impl`
- **Scope**: 实现 `Phase2Services`（`DaemonPhase2Services`），调用 ccb-daemon socket client + ccb-providers launchers，接通 dispatch。
- **依赖**: daemon_runtime（✓ stub=0）、socket client（✓）。
- **验收**: `dispatch` 能驱动真实命令（`ccb ps` / `ccb ping` / `ccb start` / `ccb ask` 等）端到端，render 输出与 Python 一致。
- **Commit**: `9be27c74`（包含在 Wave 2 commit 中，任务已归档）。
- **文件**: `crates/ccb-cli/src/phase2_services.rs`, `crates/ccb-cli/tests/phase2_services_tests.rs`。

### Wave 2 — 核心 parity（已完成）
- **Task**: `06-24-py2rust-core-parity`
- **Scope**: `ensure_agent_runtime` 编排（detached fallback / pane size / foreign binding / namespace limits）、completion `SessionRotate` selector reset、heartbeat classifier re-export、jobs store 非 `job_event` 记录过滤、CLI maintenance `status/tick/schedule/runner` 完整编排。
- **Commit**: `9be27c74`。
- **文件**: `crates/ccb-daemon/src/start_runtime/agent_runtime*.rs`, `crates/ccb-completion/tests/integration_tests.rs`, `crates/ccb-heartbeat/src/classifier.rs`, `crates/ccb-jobs/src/store.rs`, `crates/ccb-cli/src/services/maintenance.rs`。

### Wave 3 — stub 削减（量，~65% 在此）
- **Task**: `06-24-py2rust-providers-daemon-deep`
- **Scope**: `ccb-providers` 368 stub + `ccb-daemon` 345 stub，按 provider/daemon 子主题拆分实施。
- **Handoff**: `.trellis/tasks/06-24-py2rust-providers-daemon-deep/HANDOFF.md`（已为 `glm5.2` 准备）。
- **目标**: providers/daemon stubs 各降至 ≤ 50。

### Wave 4 — 端到端 + 边缘
- `py2rust-e2e-terminal-edge`（Layer 1，mock 边界内 integration parity）：
  - [x] 多 agent 会话持久化/恢复 P0-a/b（reload handoff、keeper/lifecycle）已关闭。
  - [x] terminal namespace / pane identity 跨重启 parity 已关闭。
  - [x] install core Rust 函数 parity（tar 安全、line-ending）已关闭。
  - [x] MCP delegation server parity 已关闭。
  - [x] sidebar click/resize sync parity 已关闭。
  - [x] active runtime polling parity 已关闭。
  - [x] ask/restart CLI edge parity 已关闭。
  - [x] runtime env control plane parity 已关闭。
  - [x] stability regressions（Codex log-reader / delivery guard）parity 已关闭。
  - [ ] P0-c..h 多 agent 恢复缺口：persistent `MountManager`/`OwnershipGuard`、`RuntimeSupervisionLoop`、rich ping payload、dispatcher restore/terminate jobs；记录在 parity matrix 和 roadmap。
  - [ ] 12 个 Python 安装/bash/Windows/WSL/skill/repo-hygiene 测试明确退役并记录理由。
- `py2rust-e2e-recovery`（Layer 2，live provider 集成）保留在 Python 参考中，不在 Rust mock 边界内。

## Conventions（迁移约定，已验证）

- **payload 风格**: `&serde_json::Value` + `.get("k").and_then(|v| v.as_str())`（与 render/handler 一致）。不为 Python 动态 dict 建强类型 struct。
- **依赖约束**: 禁 `chrono`/`regex`/`reqwest`（用 `std::time`、字符串操作、`curl` 子进程）。`libc` 已用于 `kill(2)`（ccb-cli）。
- **camino**: `PathLayout::new` 接受 `impl Into<Utf8PathBuf>`，非 std `PathBuf`。
- **Trellis 流程**: check（质量门）→ Phase 3.4 commit → finish-work（archive+journal）。每任务必走全闭环。
- **GateGuard**: 每次 Edit/Write 呈现 4 点事实（调用文件/无重复/数据读写/用户指令）。
- **测试**: PRD 验收用 `-- --test-threads=1`（规避并行 env 竞争）；env 测试加 `static Mutex` 串行化（见 `source_guard.rs` 模板）。
- **subagent**: 大文件(>200行)委托 `oh-my-claudecode:executor` + `model=sonnet`，**硬性要求构建零 error 才算完成**。

## Done criteria（整体迁移完成）

- [ ] workspace stub → 0（或仅 intentionally-out-of-scope：Python wrapper scripts、Windows/WSL 工具链）。
- [ ] parity matrix 26 集群 → `complete`（或明确 out-of-scope 并记录理由）。
- [ ] `Phase2Services` impl 存在，CLI 端到端可跑核心命令（ps/wait/ask/kill/start/ping）。
- [ ] `cargo test --workspace -- --test-threads=1` 全绿。
- [ ] `cargo clippy --workspace --all-targets` 0 error，`cargo fmt --check` 干净。

## Out of scope（明确排除）

- Python wrapper scripts（`bin/ask`, `bin/autonew`, `bin/ctx-transfer`, `ccb`）→ 由 Rust 原生 binary 替代。
- 真实 provider CLI 实时交互测试 → 保留 Python 参考，Rust 用 mock。
- Windows bootstrap / WSL path utils → 无 Rust 等价（除非后续需求）。
- 下列 Python 测试属于 bash/install/skill/repo-hygiene/Windows/WSL 行为，已从 Rust parity 目标退役，理由记录在 `plans/rust-python-test-parity-matrix.md`：
  `test_ask_skill_templates.py`, `test_ccb_github_skill.py`, `test_install_identity_output.py`, `test_install_major_upgrade_guard.py`, `test_install_root_confirmation.py`, `test_install_script_sidebar.py`, `test_install_source_dev_mode.py`, `test_install_watchdog_optional.py`, `test_install_droid_delegation.py`, `test_repo_hygiene.py`, `test_windows_bootstrap_script.py`, `test_wsl_path_utils.py`。
