# 核心类型与存储迁移执行计划

## 1. 执行步骤

### Step 1：消息键一致性检查

```bash
cd /home/agnitum/ccb
python3 - <<'PY'
import ast, json
with open('lib/ui_text/i18n.py') as f:
    tree = ast.parse(f.read())
for node in ast.walk(tree):
    if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'MESSAGES' for t in node.targets):
        msgs = ast.literal_eval(node.value)
        py_keys = set(msgs['en'].keys())
        break
with open('rust/crates/ccb-ui-text/src/i18n.rs') as f:
    rs_text = f.read()
import re
rs_keys = set(re.findall(r'^\s*\("([a-z_]+)"\s*,', rs_text, re.MULTILINE))
missing = sorted(py_keys - rs_keys)
extra = sorted(rs_keys - py_keys)
print('Python keys:', len(py_keys))
print('Rust keys:', len(rs_keys))
print('Missing in Rust:', missing)
print('Extra in Rust:', extra)
PY
```

- 若有缺失，在 `ccb-ui-text/src/i18n.rs` 中补充对应中英文消息。
- 若 Rust 侧多出键不影响功能，可保留；若引发 clippy 冗余警告，标记为待清理。

### Step 2：parity matrix 审计

1. 打开 `plans/rust-python-test-parity-matrix.md`。
2. 对 `types_i18n`、`storage_paths` 集群，列出每个 Python 测试名称。
3. 在每个目标 crate 中搜索对应行为的关键词，判断是否已有 Rust 测试覆盖。
4. 将审计结果记录到本任务目录的 `audit.md`。

### Step 3：运行时环境审计

```bash
cd /home/agnitum/ccb/rust
cargo test -p ccb-runtime-env -- --test-threads=1
```

- 检查 `src/env.rs`、`src/control_plane.rs`、`src/user_session.rs` 是否与 Python `runtime_env` 语义一致。
- 补充缺失的环境键过滤或解析函数。

### Step 4：存储抽象审计

```bash
cd /home/agnitum/ccb/rust
cargo test -p ccb-storage -- --test-threads=1
```

- 检查 `PathLayout` 等效路径是否完整。
- 检查 `text_artifacts` 的 spill/sweep/validate 行为是否与 Python 一致。
- 检查 `jsonl_store` 的 append/read_all/read_tail/find_last 语义。

### Step 5：分类规则审计

```bash
cd /home/agnitum/ccb/rust
cargo test -p ccb-storage-classification -- --test-threads=1
```

- 检查 `provider_home` 分类规则是否覆盖 Python 中所有 provider 目录。
- 检查 `runtime skills` 分类是否与 Python 一致。

### Step 6：补全与测试

- 对 Step 1-5 发现的每个缺口，按最小改动原则补全。
- 每个补全点必须伴随新增或更新的 Rust 测试。
- 补全过程中若发现某缺口涉及上层逻辑，记录到 parent task 并 defer 到对应 child task。

### Step 7：最终验证

```bash
cd /home/agnitum/ccb/rust
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test -p ccb-runtime-env -p ccb-types -p ccb-storage -p ccb-storage-classification -p ccb-ui-text -- --test-threads=1
cargo test --workspace -- --test-threads=1
```

### Step 8：文档更新

- 更新 `plans/rust-python-test-parity-matrix.md`：
  - `types_i18n` → `complete`（或说明 deferred 项）
  - `storage_paths` → `complete`（或说明 deferred 项）
- 在 child task 目录下创建 `audit.md`，记录审计结果和补全清单。

## 2. 审查门

- **Gate A**：消息键一致性无缺失。
- **Gate B**：目标 crate 测试全部通过。
- **Gate C**：parity matrix 对应集群状态更新。
- **Gate D**：全 workspace 测试无回归。

## 3. 回滚点

- 本任务开始前打 tag：`py2rust-core-baseline`。
- 若审查门失败，回滚到 baseline 重新审计。

## 4. 预计产出

- 少量补全代码 + 新增测试。
- `audit.md`：核心层现状与缺口记录。
- 更新的 `plans/rust-python-test-parity-matrix.md`。
- 通过所有审查门后，child task 可 archive。
