# Providers 迁移审查

## 审计对象

- `rust/crates/ccb-provider-core/src/source_home.rs`
- `rust/crates/ccb-provider-core/tests/source_home_tests.rs`（新建）

## 现状

- `current_provider_source_home()` 已实现 `CCB_SOURCE_HOME` 覆盖与 CCB provider home 检测。
- 当 `HOME` 指向托管 provider home 时，Python 会回退到 `pwd.getpwuid` 的真实用户 home；Rust 原实现直接回退到 `USERPROFILE`/HOME，缺少 passwd 回退。

## 已做变更

1. 在 `source_home.rs` 中新增 `passwd_home()`（Unix only，使用 `libc::getpwuid`）。
2. 调整解析顺序：托管 `HOME` 之后先尝试 `passwd_home()`，再 `USERPROFILE`，最后 `HOME`。
3. 新建 `source_home_tests.rs`，覆盖：
   - 非托管 HOME 直接使用。
   - 托管 HOME 回退到 passwd home（Linux）。
   - `CCB_SOURCE_HOME` 显式覆盖。
4. 更新 `plans/rust-python-test-parity-matrix.md`。

## 验证结果

- `cargo test -p ccb-provider-core -- --test-threads=1`：通过（新增 3 个测试）。
- `cargo clippy -p ccb-provider-core -- -D warnings`：通过。

## 剩余缺口

- Provider launcher/session 绑定、provider profiles/hooks 等大量测试仍留在 Python 参考实现中，计划在 `py2rust-parity` 阶段继续处理。
