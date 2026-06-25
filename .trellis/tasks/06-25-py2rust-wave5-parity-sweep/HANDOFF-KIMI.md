# Handoff — Wave 5 parity sweep → kimi2.7

> Prepared for: **kimi2.7** | From: glm5.2 | Branch: `python-rust/rolepacks-versioning-translation`
> 性质：**高体积、机械重复**的 parity grind。一次 sweep，逐项闭环。

## 🔁 统一每项闭环模板（每项都走这 7 步，留证据）
1. **读 Python 参考**：定位 `lib/<module>` 的行为 + 对应 `test_*.py`，记录关键行为点。
2. **Rust 现状**：定位 `rust/crates/ccbr-*` 对应代码，判定 `done/partial/missing/drift`。
3. **对齐**：补/修 Rust 实现，使行为与 Python 等价（同名语义、同错误、同状态机）。
4. **测试 parity**：补/对齐 Rust 测试，覆盖 Python 测试的场景；测试名对齐便于追溯。
5. **验证**：`cargo test -p <crate>` 绿；（行为级项）在 `/mnt/d/dapro-ass` 做 live 或集成验证。
6. **闭环矩阵**：更新 `plans/rust-python-test-parity-matrix.md` 该项：状态(done) + Rust 符号/文件 + 测试名 + 一句行为等价说明。
7. **同步**：改了 rust/ → ccb-legacy 反向重命名（`s/ccbr/ccb/g; s/CCBR/CCB/g`，内容+路径，方法见归档的 ccb-legacy-nonrust-sync HANDOFF）+ 产品仓 ff-push（`/home/agnitum/ccb-rust-prod-workflow.md`）。

## 本 sweep 范围（6 P1/P2 gap + 矩阵扫雷）
| 优先级 | gap | Python 参考 | Rust 位置 |
|---|---|---|---|
| P1 | midrun-cancel | `lib/ccbd/services/dispatcher_runtime/cancel_runtime.py` | `ccbr-daemon/src/services/dispatcher.rs`（handle_cancel 已发 Ctrl-C，补真正 mid-run 截停 + 测试）|
| P1 | provider-timeout | `lib/ccbd/services/job_heartbeat_runtime/` | `ccbr-daemon/src/services/dispatcher.rs` + provider polling |
| P1 | auth-error-surface | `lib/provider_backends/codex/auth_runtime.py` | `ccbr-provider-profiles/src/codex_home_config.rs`（handle_ask 已 fail-fast，补 CLI 显式错误）|
| P1 | rich-ping | `lib/ccbd/handlers/ping_runtime/` | `ccbr-daemon/src/handlers/ping.rs` |
| P2 | codex-delivery-guard | `test_stability_regressions.py::test_codex_delivery_guard_fails_on_shutdown_text_without_anchor` | `ccbr-providers/src/providers/codex.rs` |
| P2 | start-foreground-service | `lib/cli/start_foreground_runtime/` + `lib/ccbd/start_flow_runtime/service.py` | `ccbr-cli/src/start_foreground.rs` + `ccbr-daemon/src/start_flow/service.rs` |

**矩阵扫雷**：`plans/rust-python-test-parity-matrix.md`（121 行）的 6 partial + 6 gap + 2 missing —— 与上表去重后，逐项套模板闭环。

## 纪律（必守）
- **回收**：每次 live 验证后 `bash scripts/ccbr-test-cleanup.sh`（只清 ccbr 测试残留，绝不碰 ccb 生产 n14/o13/e-contract + /tmp/ccb-*）。
- **ccb-legacy**：独立双生血系（跟 ccbr 仅命名不同、永不合并）；只有改了 rust/ 才反向重命名同步。
- **CLI 环境**：所有 ccbr/ccbrd 命令带 `CCBR_SOURCE_RUNTIME_OK=1`；socket/pane 名动态取。

## 完成判定
6 P1/P2 gap + 矩阵扫雷全部闭环（实现+测试+矩阵证据）；cargo test --workspace 全绿；ccb-legacy/产品仓 同步；无 panic/stuck。进度逐项入 research/。
