# Mailbox 与 Message Bureau 迁移执行计划

## 1. 执行步骤

### Step 1：运行测试基线

```bash
cd /home/agnitum/ccb/rust
cargo test -p ccb-mailbox -p ccb-message-bureau -- --test-threads=1
```

### Step 2：公共 API 对齐检查

- 确认 `ccb-mailbox/src/lib.rs` 中的 `mailbox_kernel_public_items_re_exported` 测试覆盖 Python `__all__`。
- 确认 `ccb-message-bureau/src/lib.rs` 中的 `all_message_bureau_public_items_are_re_exported` 测试覆盖 Python `__all__`。

### Step 3：集成测试抽样审查

- 阅读 `ccb-mailbox/tests/integration.rs`，确认覆盖 submit → queue → claim → complete → inbox empty 的完整生命周期。
- 若发现未覆盖场景，补充测试。

### Step 4：最终验证

```bash
cd /home/agnitum/ccb/rust
cargo fmt --check
cargo clippy -p ccb-mailbox -p ccb-message-bureau -- -D warnings
cargo test -p ccb-mailbox -p ccb-message-bureau -- --test-threads=1
cargo test --workspace -- --test-threads=1
```

### Step 5：文档更新

- 更新 `plans/rust-python-test-parity-matrix.md` 中 `mailbox` 集群 Notes。
- 编写 `audit.md`。

## 2. 审查门

- **Gate A**：目标 crate 测试全部通过。
- **Gate B**：目标 crate clippy 无警告。
- **Gate C**：公共 API 编译期对齐测试通过。
- **Gate D**：全 workspace 测试无新增回归。
- **Gate E**：parity matrix 更新。

## 3. 回滚点

- 本任务开始前打 tag：`py2rust-mailbox-baseline`。

## 4. 预计产出

- `audit.md`：现状与任何 deferred 缺口。
- 少量补全/测试（如有）。
- 更新的 `plans/rust-python-test-parity-matrix.md`。
