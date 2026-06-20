# CLI 迁移审查

## 审计对象

- `rust/crates/ccb-cli` 主 crate
- `rust/crates/ccb-cli/src/bin/ask.rs`
- `rust/crates/ccb-cli/src/bin/autonew.rs`
- `rust/crates/ccb-cli/src/bin/ctx-transfer.rs`

## 现状

- `ccb-cli` crate 编译、测试、`cargo clippy -p ccb-cli -- -D warnings` 均通过。
- 主 CLI 命令面覆盖 `start`、`ask`、`kill`、`status`、`doctor` 等核心命令。
- 三个辅助二进制（`ask`、`autonew`、`ctx-transfer`）原本对 `--help` 全部委托给顶层 `ccb --help`，与 Python 参考实现行为不一致：
  - Python `ask_cli.main` 对 `--help` 输出 `ask` 专用用法。
  - Python `bin/autonew` 对 `--help` 输出 `autonew` 专用用法。
  - Python `bin/ctx-transfer` fallback 对 `--help` 输出 `ctx-transfer` 专用用法。

## 已做变更

1. `ask.rs`：检测到 `-h`/`--help`/`help` 时直接调用 `ccb_cli::ask_usage::write_ask_usage` 输出 ask 用法。
2. `autonew.rs`：检测到帮助标志时输出 `autonew` 专用用法。
3. `ctx-transfer.rs`：检测到帮助标志时输出 `ctx-transfer` 专用用法。
4. 三个二进制 `--version` 保持委托给 `ccb --version`，兼容既有测试。
5. 在 `helper_binaries_tests.rs` 新增 3 个 `--help` 测试。
6. 更新 `plans/rust-python-test-parity-matrix.md` `cli_entrypoint` 行。

## 验证结果

- `cargo test -p ccb-cli -- --test-threads=1`：全部通过（新增 3 个测试）。
- `cargo clippy -p ccb-cli -- -D warnings`：通过。

## 剩余缺口

- `ccb-cli` 仍存在大量 Python 1:1 对齐占位文件（`//! 1:1 file alignment stub`），多数未在活跃路径中使用，不影响当前 runtime。
- 完整的 Python CLI 命令面 parity（尤其是 `roles`、`tools`、管理类子命令的深层行为）留待后续 `py2rust-parity` 阶段统一处理。
