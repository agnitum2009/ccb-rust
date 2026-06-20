# CLI 迁移执行计划

## 实施步骤

1. **修改 `ask.rs`**
   - 引入 `ccb_cli::ask_usage::write_ask_usage`。
   - 在 `main()`/`delegate_to_ccb()` 中检测 `--help`/`-h`/`help`，输出 ask 用法并返回 0。
   - 保持 `--version` 走现有 top-level 委托路径。

2. **修改 `autonew.rs`**
   - 检测 `--help`/`-h`/`help`，输出 `autonew` 用法。
   - 保持 `--version` 路径。

3. **修改 `ctx-transfer.rs`**
   - 检测 `--help`/`-h`/`help`，输出 `ctx-transfer` 用法。
   - 保持 `--version` 路径。

4. **扩展测试**
   - 在 `helper_binaries_tests.rs` 中增加 3 个 `--help` 测试。

5. **验证**
   - `cargo test -p ccb-cli -- --test-threads=1`
   - `cargo clippy -p ccb-cli -- -D warnings`

6. **更新 parity matrix**
   - 在 `plans/rust-python-test-parity-matrix.md` 的 `cli_entrypoint` 行补充说明：辅助二进制 `--help` parity 已覆盖。

## 停止边界

- 不进入 `ccb-daemon`、`ccb-terminal` 或 `ccb-provider-*` 的实现细节。
- 不修改主 CLI 命令解析与分发逻辑。
