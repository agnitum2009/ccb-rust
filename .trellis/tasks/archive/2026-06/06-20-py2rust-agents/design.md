# Agents 迁移设计

## 变更点

1. `rust/crates/ccb-agents/src/rolepacks.rs`
   - 新增 `ProjectRoleLockUpdate` 结构体。
   - 新增 `find_project_role_lock_updates`：
     - 加载项目配置，遍历 agents 的 `role`。
     - 读取 `role-lock.json` 对应条目。
     - 通过 `load_installed_role` 获取当前安装版本（`current` 软链）。
     - 比较 version/digest，不一致则产生更新记录。
   - 新增 `confirm_project_role_lock_refresh`：
     - 无更新时直接返回。
     - 非交互模式输出 `role_lock_update_available` 与 `role_lock_refresh: skipped_noninteractive`。
     - 交互模式列出更新并提示 `[y/N]`，确认后调用 `write_project_role_lock` 写入当前版本并输出 `role_lock_refreshed`。
   - 辅助函数：`installed_current_digest`、`format_update_available`、`format_versions`。

2. `rust/crates/ccb-agents/tests/rolepack_tests.rs`
   - 新增测试 helper：`install_role_version`、`set_current_symlink`、`write_locked_project`。
   - 新增 3 个测试对应 Python `test_role_lock_refresh.py`。

## 兼容性

- 不修改既有 role lock 读写、解析逻辑。
- 新增 API 与 Python 函数名/输出保持一致，便于后续 CLI service 集成。
