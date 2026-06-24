# PRD — ccb→ccbr 全面重命名（Rust workspace 品牌切割）

## 背景 / 决策

作者决策：Rust 版全面从 `ccb` 重命名为 `ccbr`，**避免与本地已安装的 Python `ccb` 在调试/运行时混淆**。覆盖所有用户可见与内部标识：二进制、配置目录/文件、tmux pane 身份、env 变量、crate 名。这是一次干净的品牌切割（ccbr 不兼容 Python ccb 的运行期标识）。

执行位置：`/home/agnitum/ccb/rust/`（Rust 源所在，测试可跑）。完成后**重建产品仓并重新 force-push** 到 `agnitum2009/ccb-rust:master`。

## 重命名规则表（精确，防碰撞）

| 原始 | 目标 | 类别 | 说明 |
|---|---|---|---|
| `.ccb` | `.ccbr` | 配置目录（路径字符串） | 115 处；含 `.ccb/ccbd.sock`→`.ccbr/...`、`.ccb/agents/...` 等 |
| `ccb.config` | `ccbr.config` | 配置文件名 | |
| `@ccb_` | `@ccbr_` | tmux pane 身份（控制面） | `@ccb_project_id`/`@ccb_role`/`@ccb_managed_by`/`@ccb_session_id`/`@ccb_namespace_epoch`/`@ccb_active`/`@ccb_agent`/`@ccb_*_style` 等 16 文件 |
| `CCB_` | `CCBR_` | env 变量 | `CCB_BACKEND_ENV`/`CCB_CALLER`/`CCB_CCBD_*` 等（仅前缀 `CCB_`→`CCBR_`，注意 `CCB_CCBD_*`→`CCBR_CCBD_*` 或同步 `CCBD`?见下） |
| `ccb-` (crate 名) | `ccbr-` | crate 名 | Cargo.toml `[package] name`、workspace `members`、`use ccb_*`→`use ccbr_*`、crate 间 dep、`ccb-* = { path }` |
| `ccbd` | `ccbrd` | daemon 二进制/标识 | **默认改**（与 ccb 切割一致）；socket `ccbd.sock`→`ccbrd.sock`、handler 名等。**【需作者确认】** 是否连 daemon `ccbd`→`ccbrd` 一并改（默认是） |
| `ccb` (二进制) | `ccbr` | CLI 二进制 | `ccbr` 二进制已存在于 `ccb-cli`；移除/合并任何 `ccb` 二进制目标，`ccbr` 为唯一主二进制 |

## 碰撞与例外（执行前必须甄别）

1. **字面量 `"ccb"` 作 provider/protocol 标识（46 处）**：这些可能是「CCB 托管 provider」的身份名或协议常量（区别于品牌前缀）。**先甄别**：是品牌/路径前缀 → 改 `ccbr`；是 provider 注册名/线协议常量 → **保留 `"ccb"`**（改了会破坏 provider registry/协议）。逐处判定，别一刀切。
2. **`ccbd` 是 `ccb`+`d`**：规则表已列为单元 `ccbd`→`ccbrd`，**不要**先 `ccb`→`ccbr` 留下孤立 `d`。先做 `ccbd`→`ccbrd`，再做 `ccb`→`ccbr`（顺序防残缺）。
3. **`CCB_CCBD_*` env**：前缀改 `CCBR_` 后，`CCBD` 部分按 `ccbd`→`ccbrd` 决定 → `CCBR_CCBRD_*` 或保留 `CCBR_CCBD_*`。与 daemon 改名决定一致。
4. **词边界**：用精确前缀匹配（`ccb-`、`.ccb`、`@ccb_`、`CCB_`、`ccbd`），避免误伤含 `ccb` 子串的其他词（若存在）。
5. **Python 参考保留 `ccb`/`@ccb_*`**——ccbr 是切割，Python 不动（仅 Rust workspace 改）。

## 分阶段（每阶段：cargo check 绿 + 单独提交）

1. **P1 配置路径**：`.ccb`→`.ccbr`、`ccb.config`→`ccbr.config`（115 处）。`cargo check` 绿 → 提交。
2. **P2 crate 名**：`ccb-*`→`ccbr-*`（Cargo.toml name/members/deps + 全部 `use ccb_*`→`use ccbr_*` + 目录名 `crates/ccb-X`→`crates/ccbr-X`）。最大块。`cargo check --workspace` 绿 → 提交。
3. **P3 tmux 身份 + env**：`@ccb_`→`@ccbr_`、`CCB_`→`CCBR_`。绿 → 提交。
4. **P4 二进制 + daemon**：`ccbr` 唯一主二进制（移除 `ccb` 目标）；`ccbd`→`ccbrd`（含 socket/handler）。绿 → 提交。
5. **P5 文档/脚本**：README、scripts、字符串里的品牌 `ccb`→`ccbr`（保留 provider 标识）。绿 → 提交。
6. **P6 产品仓重建 + 重推**：从重命名后的 `rust/` 重建 `/tmp/ccb-rust-build`（同前流程：tar 排除 target/.omc/.agents/.codegraph/gap-reports + README/LICENSE/.gitignore）→ `cargo check` 绿 → force push `agnitum2009/ccb-rust:master`。

## 验收标准

- [ ] `grep -rln '\.ccb["/ ]|ccb\.config' rust/crates rust/tools | wc -l` == 0（配置路径全改）。
- [ ] `grep -rln '@ccb_' rust/crates rust/tools | wc -l` == 0；`grep -rln 'CCB_' rust/... ` == 0（env）。
- [ ] `ls rust/crates | grep -c '^ccb-'` == 0（crate 名全 `ccbr-`）。
- [ ] `cargo check --workspace` 绿；`cargo test --workspace -- --test-threads=1` 全绿（0 回归）；`cargo clippy --workspace --all-targets -- -D warnings` 0；`cargo fmt --check` clean。
- [ ] 产品仓 `agnitum2009/ccb-rust:master` 已用重命名后版本 force-push 更新。

## 护栏 / 风险

- **每阶段 cargo check 必须绿**；断裂回退该阶段最近提交定位。
- **`"ccb"` provider/protocol 标识逐处甄别**，不盲改（破坏 registry/协议的高风险）。
- `ccbd`→`ccbrd` 默认改，但**作者最终确认**（见规则表）。
- 不动 Python 参考（`lib/`、`test/`）；不改 ccb-mailbox 线协议语义（仅改名）。
- 提交分阶段，格式 `refactor(brand): ccb->ccbr <phase>`，尾注 `Co-Authored-By: Claude <noreply@anthropic.com>`。

## 参考

- 重命名前产品仓：`agnitum2009/ccb-rust:master`（commit `be093f6`）。
- ccb 用量分类盘点（本任务讨论）：配置路径 115、tmux `@ccb_` 16、env `CCB_*` 多、crate `ccb-*` 10+、二进制 `ccbr` 已存、`"ccb"` 标识 46。
- 产品仓构建流程：见上轮 `/tmp/ccb-rust-build`（tar 复制 rust/ 核心排除 target/.omc/.agents/.codegraph/gap-reports + README/LICENSE/.gitignore）。
