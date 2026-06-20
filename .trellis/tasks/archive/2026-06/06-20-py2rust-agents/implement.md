# Agents 迁移执行计划

## 实施步骤

1. **扩展 `rolepacks.rs` 导入**
   - 添加 `std::io::{BufRead, Write}`。

2. **实现核心函数**
   - `ProjectRoleLockUpdate` 结构体。
   - `find_project_role_lock_updates`。
   - `confirm_project_role_lock_refresh`。
   - 辅助函数 `installed_current_digest`、`format_update_available`、`format_versions`。

3. **扩展 `rolepack_tests.rs`**
   - 导入新增函数。
   - 添加 helper 函数创建多版本已安装 role 与旧 lock。
   - 添加 3 个测试。

4. **验证**
   - `cargo test -p ccb-agents -- --test-threads=1`
   - `cargo clippy -p ccb-agents -- -D warnings`

5. **更新 parity matrix**
   - 在 `agents_roles` 行说明 role lock refresh 已覆盖。

## 停止边界

- 不修改 `ccb-cli/src/services/role_lock_refresh.rs` 调用逻辑。
- 不处理 daemon start 流程中的 lock refresh 集成。
