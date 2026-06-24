# Wave 1: CLI Phase2Services 架构解锁

## Problem

`crates/ccb-cli/src/phase2_runtime/handlers_ops.rs` 定义了 `Phase2Services` trait（17+ service 方法 + 24 handler + 28 命令的 dispatch），但整个 workspace 中 **0 个 `impl Phase2Services`**。

结果：CLI `dispatch → handle_xxx → render_xxx` 链路在 service 层断开。render（29 函数）+ handlers + launchers 零件齐全，但 CLI 无法端到端运行任何命令。

## Goal

实现一个 concrete `Phase2Services`，让 `dispatch` 能驱动真实命令，覆盖核心命令：

- `ps`
- `ping`
- `wait`
- `kill`
- `start`
- `ask`
- `restart`
- `logs`
- `maintenance`
- `reload`

## Scope

- 复用/扩展 `crates/ccb-cli/src/phase2_services.rs` 中的 `DaemonPhase2Services`。
- 补全返回 `"not yet implemented"` / `"error"` 的核心方法。
- 确认 `src/entry.rs` 在 v2 命令路径上调用 `phase2_runtime::dispatch::dispatch`。
- 为每个核心命令添加 Rust 集成测试，断言输出与 Python `cli.phase2` 行为一致。

## Out of scope

- 新增 provider CLI 实时交互测试（保持 mock）。
- 重写 legacy `commands.rs` 中的命令实现；只需保证 phase2 dispatch 路径可用。

## Acceptance criteria

- [ ] `Phase2Services` 存在具体实现并被 CLI 入口使用。
- [ ] 以下命令通过 Rust 集成测试：ps、ping、wait、kill、start、ask、restart、logs、maintenance、reload。
- [ ] `cargo test -p ccb-cli -- --test-threads=1` 全绿。
- [ ] `cargo check --workspace` 通过。
- [ ] 更新 `plans/rust-python-test-parity-matrix.md` 中 `cli_entrypoint` 的测试映射与状态。

## References

- `rust/crates/ccb-cli/src/phase2_runtime/handlers_ops.rs`
- `rust/crates/ccb-cli/src/phase2_services.rs`
- `rust/crates/ccb-cli/src/entry.rs`
- `plans/rust-python-test-parity-matrix.md` (cli_entrypoint 行)
