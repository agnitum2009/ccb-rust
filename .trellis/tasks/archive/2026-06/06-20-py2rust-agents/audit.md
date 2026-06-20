# Agents 迁移审查

## 审计对象

- `rust/crates/ccb-agents/src/rolepacks.rs`
- `rust/crates/ccb-agents/tests/rolepack_tests.rs`

## 现状

- `ccb-agents` crate 已有完整的 role pack 加载、安装、锁定、解析能力，测试全部通过。
- Python `test_role_lock_refresh.py` 对应的 `find_project_role_lock_updates` 与 `confirm_project_role_lock_refresh` 在 Rust 中缺失。

## 已做变更

1. 在 `rolepacks.rs` 中新增：
   - `ProjectRoleLockUpdate` 结构体。
   - `find_project_role_lock_updates`：扫描项目配置，对比 lock 与当前安装 role。
   - `confirm_project_role_lock_refresh`：交互/非交互两种模式，输出与 Python 一致。
   - 辅助函数 `installed_current_digest`、`format_update_available`、`format_versions`。
2. 在 `rolepack_tests.rs` 中新增 3 个测试，覆盖检测、交互刷新、非交互跳过。
3. 更新 `plans/rust-python-test-parity-matrix.md`。

## 验证结果

- `cargo test -p ccb-agents -- --test-threads=1`：全部通过（新增 3 个测试）。
- `cargo clippy -p ccb-agents -- -D warnings`：通过。

## 剩余缺口

- `ccb-cli/src/services/role_lock_refresh.rs` 仍为 stub，未调用新 API。
- `commands::start` 未在项目启动前执行 role lock refresh。
- 上述集成工作计划在 `py2rust-parity` 阶段完成。
