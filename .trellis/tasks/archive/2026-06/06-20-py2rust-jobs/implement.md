# Heartbeat 与 Jobs 迁移执行计划

## 1. 执行步骤

### Step 1：运行测试基线

```bash
cd /home/agnitum/ccb/rust
cargo test -p ccb-heartbeat -p ccb-jobs -- --test-threads=1
```

### Step 2：修复 JobEvent JSON 字段名

在 `rust/crates/ccb-jobs/src/models.rs` 中：

```rust
#[serde(rename = "type")]
pub event_type: String,
```

### Step 3：新增/更新测试

- 在 `ccb-jobs/src/models.rs` 或 `tests/store_integration.rs` 中增加 `JobEvent` JSON roundtrip 测试，断言序列化后的 JSON 包含 `"type"` 字段而不是 `"event_type"`。

### Step 4：最终验证

```bash
cd /home/agnitum/ccb/rust
cargo fmt --check
cargo clippy -p ccb-heartbeat -p ccb-jobs -- -D warnings
cargo test -p ccb-heartbeat -p ccb-jobs -- --test-threads=1
cargo test --workspace -- --test-threads=1
```

### Step 5：文档更新

- 更新 `plans/rust-python-test-parity-matrix.md` 中相关集群 Notes。
- 编写 `audit.md`。

## 2. 审查门

- **Gate A**：`JobEvent` 序列化字段名为 `type`。
- **Gate B**：目标 crate 测试全部通过。
- **Gate C**：目标 crate clippy 无警告。
- **Gate D**：全 workspace 测试无新增回归。
- **Gate E**：parity matrix 更新。

## 3. 回滚点

- 本任务开始前打 tag：`py2rust-jobs-baseline`。

## 4. 预计产出

- `rust/crates/ccb-jobs/src/models.rs` 的 `JobEvent` 字段修复。
- 新增 `JobEvent` JSON roundtrip 测试。
- `audit.md` 与更新的 parity matrix。
