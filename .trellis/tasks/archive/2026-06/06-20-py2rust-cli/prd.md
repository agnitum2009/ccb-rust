# CLI 迁移（py2rust-cli）

## Goal

完成 CCB CLI 层（`ccb-cli` crate 及 `ask`/`autonew`/`ctx-transfer` 辅助二进制）从 Python 参考实现到 Rust 的对齐与审查。

## Scope

- `rust/crates/ccb-cli` 主 crate（已高度实现，测试/clippy 通过）。
- `ask`、`autonew`、`ctx-transfer` 三个辅助二进制（`src/bin/`）。
- 与 Python `bin/ask`、`bin/autonew`、`bin/ctx-transfer` 的可见行为保持 parity。

## Requirements

1. `ask --help`/`-h` 必须输出 `ask` 子命令的用法（与 Python `ask_cli.main` 行为一致），而不是顶层 `ccb --help`。
2. `autonew --help`/`-h` 必须输出 `autonew` 的用法（与 Python wrapper 行为一致）。
3. `ctx-transfer --help`/`-h` 输出自身的用法说明。
4. `--version` 保持现有行为：委托给顶层 `ccb --version` 以输出 `ccbr 7.5.2`。
5. 为上述辅助二进制添加覆盖 `--help` 输出的单元测试。
6. 更新 `plans/rust-python-test-parity-matrix.md`，记录 CLI parity 进展。

## Acceptance Criteria

- [ ] `cargo test -p ccb-cli -- --test-threads=1` 通过。
- [ ] `cargo clippy -p ccb-cli -- -D warnings` 通过。
- [ ] `ask --help` 输出包含 `Usage: ask` 与 alias note。
- [ ] `autonew --help` 输出包含 `Usage: autonew` 与 provider 列表。
- [ ] `ctx-transfer --help` 输出包含 `Usage: ctx-transfer`。
- [ ] parity matrix 已更新。

## Notes

- `ccb-cli` 主体代码已经完整实现；本次任务聚焦辅助二进制与 CLI 可见行为的最后 parity 缺口。
- 不修改 CLI 主入口的解析逻辑，避免影响现有命令面。
