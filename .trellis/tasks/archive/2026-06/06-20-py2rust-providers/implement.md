# Providers 迁移执行计划

## 实施步骤

1. **修改 `source_home.rs`**
   - 添加 `#[cfg(unix)] fn passwd_home() -> Option<PathBuf>`。
   - 在 `current_provider_source_home` 的 `HOME` 托管分支后插入 `passwd_home()` 回退。

2. **新建测试文件**
   - 创建 `rust/crates/ccb-provider-core/tests/source_home_tests.rs`。
   - 使用序列化方式设置/恢复环境变量，避免测试并行冲突（或接受 `cargo test -- --test-threads=1`）。

3. **验证**
   - `cargo test -p ccb-provider-core -- --test-threads=1`
   - `cargo clippy -p ccb-provider-core -- -D warnings`

4. **更新 parity matrix**
   - 在 `providers` 行补充 `test_provider_source_home.py` parity。

## 停止边界

- 不进入 provider launcher/session 绑定逻辑。
- 不修改 provider profiles 或 hooks。
