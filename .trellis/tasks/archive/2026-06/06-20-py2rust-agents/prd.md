# Completion 与 Agents 迁移（py2rust-agents）

## Goal

完成 `ccb-agents` crate 中与 Python `test_role_lock_refresh.py` 对应的能力迁移与 parity 对齐。

## Scope

- `rust/crates/ccb-agents/src/rolepacks.rs`：role lock 发现与刷新核心函数。
- `rust/crates/ccb-agents/tests/rolepack_tests.rs`：对应 Python 测试的 Rust 版。

## Requirements

1. 实现 `find_project_role_lock_updates(project_root)`：扫描项目配置中所有带 `role` 的 agent，对比 `role-lock.json` 与当前已安装 role（`current` 软链）的版本/digest，返回需要更新的条目。
2. 实现 `confirm_project_role_lock_refresh(...)`：
   - 非交互环境下仅输出警告并跳过修改。
   - 交互环境下提示用户，确认后将 lock 更新到当前已安装版本。
3. 行为与输出字符串与 Python 参考实现一致（`role_lock_update_available`、`role_lock_refresh: skipped_noninteractive`、`role_lock_refreshed` 等）。
4. 提供单元测试覆盖三种场景：
   - 检测到 lock 与 current 不一致。
   - 交互式确认后更新 lock。
   - 非交互式跳过且不修改 lock。

## Acceptance Criteria

- [ ] `cargo test -p ccb-agents -- --test-threads=1` 通过。
- [ ] `cargo clippy -p ccb-agents -- -D warnings` 通过。
- [ ] 新增 3 个 role lock refresh 测试通过。
- [ ] parity matrix 已更新。

## Notes

- 本次任务聚焦 `ccb-agents` 核心能力；CLI service `role_lock_refresh.rs` 的调用集成留待 `py2rust-parity` 阶段处理。
