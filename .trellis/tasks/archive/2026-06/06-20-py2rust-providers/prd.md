# Providers 核心与后端迁移（py2rust-providers）

## Goal

补齐 `ccb-provider-core` 中 `source_home` 与 Python `test_provider_source_home.py` 的 parity。

## Scope

- `rust/crates/ccb-provider-core/src/source_home.rs`
- `rust/crates/ccb-provider-core/tests/source_home_tests.rs`（新建）

## Requirements

1. `current_provider_source_home()` 在 `HOME` 指向 CCB 托管 provider home 时，应使用 `libc::getpwuid` 返回的真实用户 home 目录（与 Python `pwd.getpwuid` 行为一致）。
2. 保持 `CCB_SOURCE_HOME` 显式覆盖优先。
3. 保持 `USERPROFILE` Windows 回退。
4. 新建测试文件覆盖三种场景：
   - 非托管 HOME 直接使用。
   - 托管 HOME 回退到 passwd home（Linux）。
   - `CCB_SOURCE_HOME` 显式覆盖。

## Acceptance Criteria

- [ ] `cargo test -p ccb-provider-core -- --test-threads=1` 通过。
- [ ] `cargo clippy -p ccb-provider-core -- -D warnings` 通过。
- [ ] parity matrix 已更新。

## Notes

- 本次任务为 providers 迁移阶段的最小 parity 缺口；更复杂的 provider launchers/session 绑定留待后续 `py2rust-parity`。
