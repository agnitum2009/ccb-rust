# Providers 迁移设计

## 变更点

1. `rust/crates/ccb-provider-core/src/source_home.rs`
   - 新增 `passwd_home()` 辅助函数（Unix only）：调用 `libc::getpwuid(libc::getuid())` 读取用户 home。
   - 调整 `current_provider_source_home()` 解析顺序：
     1. `CCB_SOURCE_HOME`
     2. `HOME`（若非 CCB provider home）
     3. `passwd_home()`（Unix 回退）
     4. `USERPROFILE`
     5. `HOME`（最后手段）

2. `rust/crates/ccb-provider-core/tests/source_home_tests.rs`
   - 测试非托管 HOME 直接使用。
   - 测试托管 HOME 回退到 passwd home（Linux）。
   - 测试 `CCB_SOURCE_HOME` 覆盖。

## 兼容性

- 非 Unix 平台 `passwd_home()` 返回 `None`，行为不变。
- 现有 `source_home.rs` 单元测试继续保留。
