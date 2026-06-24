# Wave 2: 核心 parity（runtime launch + completion/heartbeat + CLI maintenance）

## Problem

Wave 1 解锁了 CLI `Phase2Services` 的端到端路径，但 Wave 3/4 依赖的三个底层子系统仍未达到 Python v7.5.2 的行为 parity：

1. **Runtime launch 编排缺口**：`ccb-daemon/src/start_runtime/ensure_agent_runtime.rs` 的 `EnsureAgentRuntimeImpl` 只实现了最基本的 pane 创建/respawn 路径。Python `cli.services.runtime_launch_runtime.tmux_panes` 中的关键行为尚未对齐：
   - 当项目 namespace 的 tmux session 无空间再分 pane 时，没有 `detached fallback`（创建独立 tmux session）。
   - 没有 pane 最小尺寸检查，小尺寸 pane 不会被杀掉并回退到 detached。
   - 没有 `prepare_detached_tmux_server` 等价逻辑（clipboard、vi copy-mode、环境变量等初始化）。
   - `foreign binding` 场景（pane 存在但属于其他 project/socket）没有被检测和拒绝复用。
   - `tmux namespace limits` 下 `allow_detached_fallback=false` 的语义未实现。

2. **Completion/heartbeat/job-store 剩余缺口**：
   - `ccb-completion` 的 orchestrator/tracker/registry 已存在，但与 daemon dispatcher 的集成缺少对 `CompletionItemKind::SessionRotate` 后 selector reset 的端到端断言。
   - `ccb-heartbeat/src/classifier.rs` 仍是空 stub（Python `lib/maintenance_heartbeat/classifier.py` 的功能实际已迁移到 `maintenance.rs`），需要清理或补齐符号导出，避免误导。
   - `ccb-jobs` 的 `JobEventStore.read_since_target` 尚未验证“跳过非 `job_event` record_type”的 Python 兼容行为。

3. **CLI maintenance 编排缺口**：`ccb-cli/src/commands.rs` 的 `maintenance` 只实现了 `status` 和 `tick`，缺少 Python `cli.services.maintenance` 中的 `runner`、`schedule` 动作以及完整的 `tick` 编排（project_view 评估、写入 status/schedule/activation、去重、local-ps fallback、lock 保护）。这导致 `test_maintenance_heartbeat.py` 的 CLI 部分无法在 Rust 侧退役。

这些缺口会阻塞 Wave 3（providers deep）的 live launch 测试和 Wave 4（e2e recovery/terminal namespace）的维护自愈路径。

## Goal

补齐上述三个子系统的核心 parity，使 Rust 实现能够覆盖 Python `test_v2_runtime_launch.py` 中的 detached/namespace/foreign 场景、`test_maintenance_heartbeat.py` 中的 CLI 编排场景，以及 `test_v2_job_store.py` 中的事件日志过滤场景。

## Scope

### A. Runtime launch orchestration（ccb-daemon）

- 修改 `rust/crates/ccb-daemon/src/start_runtime/ensure_agent_runtime.rs`：
  - 在 `EnsureAgentRuntimeImpl::call` 中引入 `allow_detached_fallback` 控制。
  - 实现 pane 最小尺寸检查（`pane_meets_minimum_size`）和尺寸不足时杀掉 pane 的回退。
  - 实现 `create_detached_tmux_pane`（独立 session）作为 `create_pane` 失败/空间不足时的 fallback。
  - 实现 `prepare_detached_tmux_server`（初始化 clipboard、vi mode、环境变量等）。
  - 在 `launch_binding_hint` / `binding_alive` 中检测 foreign socket/project，拒绝复用 foreign binding。
- 扩展 `rust/crates/ccb-daemon/src/start_runtime/agent_runtime_models.rs`：
  - 为 `RuntimeBinding` 增加 `provider_identity_state`、`provider_identity_reason`、`ccb_project_id` 等字段，用于 foreign/stale 判断。
- 在 `rust/crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs` 中新增测试：
  - `test_detached_fallback_when_no_space_for_new_pane`
  - `test_namespace_launch_rejects_detached_fallback`
  - `test_foreign_binding_is_not_reused`
  - `test_pane_too_small_triggers_detached_fallback`

### B. Completion/heartbeat/job-store parity

- `ccb-completion`：
  - 在 `rust/crates/ccb-completion/tests/integration_tests.rs` 中补充 `tracker_resets_selector_on_session_rotate`，验证 `ingest(SessionRotate)` 后 selector 被 reset。
- `ccb-heartbeat`：
  - 删除/替换 `rust/crates/ccb-heartbeat/src/classifier.rs` 的空 stub，将 Python `classifier.py` 的公开函数签名迁移到 `maintenance.rs` 或添加 re-export，确保 `grep -n "TODO: align" crates/ccb-heartbeat/src/classifier.rs` 不再命中。
  - 在 `rust/crates/ccb-heartbeat/tests/integration.rs` 中补充 `classifier_no_stub_warning`（编译期检查）。
- `ccb-jobs`：
  - 在 `rust/crates/ccb-jobs/tests/store_integration.rs` 中补充 `event_store_skips_non_job_event_records`，验证 `read_since_target` 遇到 `record_type != "job_event"` 时跳过。

### C. CLI maintenance orchestration（ccb-cli）

- 扩展 `rust/crates/ccb-cli/src/services/maintenance.rs`：
  - 实现 `maintenance_status(context, cmd)`：读取 project config、 MaintenanceHeartbeatStore 的 schedule/status/runner，返回与 Python 兼容的 payload。
  - 实现 `maintenance_tick(context, cmd, client)`：评估 project_view（或 local-ps fallback）、写入 status/schedule、必要时提交 self-activation、处理 lock/dedup。
  - 实现 `maintenance_schedule(context, cmd)`：解析 `--after`/`--reason`，写入 schedule。
  - 实现 `maintenance_runner(context, cmd, client)`：按 schedule 轮询，执行 tick，支持 `--max-iterations`、`--sleep-cap`、`--no-dispatch`。
- 修改 `rust/crates/ccb-cli/src/commands.rs` 的 `maintenance` 函数：
  - 将 `status`/`tick`/`schedule`/`runner` 路由到 `services/maintenance.rs`。
  - `tick`/`runner` 需要 `DaemonClient`，`status`/`schedule` 仅需 `CliContext`。
- 扩展 `rust/crates/ccb-cli/src/render_runtime/ops_views_basic.rs` 的 `render_maintenance`：
  - 支持 `tick_status`、`runner_status`、`schedule_state`、`heartbeat_enabled` 等字段。
- 在 `rust/crates/ccb-cli/tests/cli_maintenance_tests.rs` 中新增测试：
  - `test_cli_maintenance_status`
  - `test_cli_maintenance_tick_healthy_writes_status`
  - `test_cli_maintenance_tick_concern_submits_activation`
  - `test_cli_maintenance_schedule`
  - `test_cli_maintenance_runner_due_tick`

## Out of scope

- 新增真实 provider CLI 实时交互测试（保持 mock）。
- 重写 `ccb-daemon` dispatcher/supervision/reload 的深层 stub（属于 Wave 3）。
- Windows/WSL path 工具链（无 Rust 等价）。
- 重写 `ccb-completion` 的 per-provider execution adapter（属于 Wave 3）。

## Acceptance criteria

- [ ] `cargo check --workspace` 通过。
- [ ] `cargo test -p ccb-daemon -- --test-threads=1` 全绿（含新增 runtime launch 测试）。
- [ ] `cargo test -p ccb-completion -- --test-threads=1` 全绿（含新增 selector reset 测试）。
- [ ] `cargo test -p ccb-jobs -- --test-threads=1` 全绿（含新增事件过滤测试）。
- [ ] `cargo test -p ccb-heartbeat -- --test-threads=1` 全绿（含 classifier stub 清理检查）。
- [ ] `cargo test -p ccb-cli -- --test-threads=1` 全绿（含新增 maintenance 编排测试）。
- [ ] `cargo clippy --workspace --all-targets` 0 error。
- [ ] `cargo fmt --check` 干净。
- [ ] `plans/rust-python-test-parity-matrix.md` 更新：
  - `runtime_launch` 行补充 detached/foreign/namespace 测试映射。
  - `heartbeat` 行确认 CLI maintenance 编排已覆盖。
  - `completion`/`jobs` 行补充新增测试映射。

## References

- `.trellis/tasks/06-24-py2rust-remaining-parity/design.md`（Wave 2 关键决策）
- `.trellis/spec/migration-roadmap.md`（整体 4-wave 顺序）
- `plans/rust-python-test-parity-matrix.md`（当前缺口追踪）
- Python 参考实现：
  - `lib/cli/services/runtime_launch.py`
  - `lib/cli/services/runtime_launch_runtime/tmux_panes.py`
  - `lib/cli/services/runtime_launch_runtime/tmux_runtime.py`
  - `lib/cli/services/maintenance.py`
  - `lib/maintenance_heartbeat/classifier.py`
  - `test/test_v2_runtime_launch.py`
  - `test/test_maintenance_heartbeat.py`
  - `test/test_v2_job_store.py`
- Rust 目标文件：
  - `rust/crates/ccb-daemon/src/start_runtime/ensure_agent_runtime.rs`
  - `rust/crates/ccb-daemon/src/start_runtime/agent_runtime_binding.rs`
  - `rust/crates/ccb-daemon/src/start_runtime/agent_runtime_models.rs`
  - `rust/crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs`
  - `rust/crates/ccb-completion/tests/integration_tests.rs`
  - `rust/crates/ccb-heartbeat/src/classifier.rs`
  - `rust/crates/ccb-heartbeat/src/maintenance.rs`
  - `rust/crates/ccb-heartbeat/tests/integration.rs`
  - `rust/crates/ccb-jobs/tests/store_integration.rs`
  - `rust/crates/ccb-cli/src/services/maintenance.rs`
  - `rust/crates/ccb-cli/src/commands.rs`
  - `rust/crates/ccb-cli/src/render_runtime/ops_views_basic.rs`
  - `rust/crates/ccb-cli/tests/cli_maintenance_tests.rs`
