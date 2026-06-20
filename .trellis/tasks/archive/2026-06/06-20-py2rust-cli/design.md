# CLI 迁移设计

## 变更点

1. `rust/crates/ccb-cli/src/bin/ask.rs`
   - 在 `delegate_to_ccb` 中识别 `--help`/`-h`/`help`。
   - 直接调用 `ccb_cli::ask_usage::write_ask_usage` 输出 ask 用法。
   - 其余情况仍委托给 `ccb ask <args>`；`--version` 仍委托给 `ccb --version`。

2. `rust/crates/ccb-cli/src/bin/autonew.rs`
   - 识别 `--help`/`-h`/`help`。
   - 直接输出 `autonew` 用法字符串（与 Python wrapper 一致）。
   - `--version` 保持委托给 `ccb --version`。

3. `rust/crates/ccb-cli/src/bin/ctx-transfer.rs`
   - 识别 `--help`/`-h`/`help`。
   - 输出简明的 `ctx-transfer` 用法字符串。
   - `--version` 保持委托给 `ccb --version`。

## 测试策略

- 在 `rust/crates/ccb-cli/tests/helper_binaries_tests.rs` 中新增：
  - `ask_help_introspection`
  - `autonew_help_introspection`
  - `ctx_transfer_help_introspection`
- 每个测试运行对应二进制并检查 stdout 包含子命令名称与用法标记。

## 兼容性

- 不改动 `ccb-cli` 主入口、不改动 `--version` 路径。
- 现有 `helper_binaries_refuse_to_run_without_runtime_ok` 测试继续通过。
