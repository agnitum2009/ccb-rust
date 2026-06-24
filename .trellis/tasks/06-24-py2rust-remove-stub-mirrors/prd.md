# PRD — Wave 3 清理：批量删除空 stub 镜像（干净树）

## 背景 / 决策

glm5.2 的 P0 源码级审计（`.trellis/tasks/06-24-py2rust-providers-daemon-deep/stub-triage.md` §2/§3）已证明：ccb-providers（352）+ ccb-daemon（345）= **697 个 `TODO: align with Python` stub 全是空 1:1 镜像**（仅 doc 注释 + TODO，零 Rust item），**零真实引用**（空模块无法满足 item 引用；`cargo check` 全绿）。

功能 parity 经 glm5.2（comms_recover 12/12）+ kimi2.7（6 缺口）两轮关闭**已基本达成**（Phase C 门绿、24/3 matrix）。作者早先「功能对齐前不动 stub」的前提**现已满足**。

**作者决策（本任务依据）**：选「批量删除（干净树）」。理由：**Python 源码在，编译/运行出错时可对照定位**——stub 镜像作为路线图的价值已被 Python 源码取代，保留只剩死文件噪音（反复制造「这是不是缺口？」的混淆）。

## 范围

删除 ccb-providers + ccb-daemon 下**所有**含 `TODO: align with Python` 标记的空 stub 文件，并同步移除其 `pub mod` 声明，保持工作区构建绿。

- **ccb-providers/src/**：352 个 stub（200 扁平顶层 + 168 深层；kimi 上轮已减 16）。
- **ccb-daemon/src/**：345 个 stub（166 扁平顶层 + 179 深层 services/* 等）。

## 方法（分批 + 每批 cargo check）

1. **识别**：仅删除**同时满足**两条件的文件——(a) 含 `TODO: align with Python` 标记，(b) 文件体为空 stub（无真实 Rust item：仅 doc 注释 + TODO 注释，或 ≤3 行）。**绝不删除含真实 Rust 代码的文件**（如 canonical adapters、dispatcher.rs、mailbox 等）。
2. **声明清理**：每删一个 stub 文件，移除其 `pub mod <name>;` 声明：
   - 扁平顶层 stub：删 `ccb-{providers,daemon}/src/lib.rs` 里对应 `pub mod X;` 行。
   - 深层 stub：删其父 `mod.rs` 里对应 `pub mod child;` 行；若某子树删除后**全部为空**，整子树（含 mod.rs）可一并删除；**若子树仍有真实文件**（如 `claude/launcher.rs`、`opencode/storage.rs`），仅删 stub 子项、保留子树与真实文件。
   - 孤儿 stub（未被任何 mod.rs 声明、本就未编译）：直接删文件即可。
3. **分批 + 验证**：按 crate（ccb-providers 一批、ccb-daemon 一批）或按子目录分批；**每批后 `cargo check -p <crate>` 必须绿**，发现问题立即定位（多半是漏删/误删了某个 `pub mod` 声明）。
4. **fmt**：删除大量 `pub mod` 行后，`cargo fmt -p <crate>` 重排 lib.rs/mod.rs。

## 验收标准

- [ ] `grep -rln 'TODO: align with Python' rust/crates/ccb-providers/src/ | wc -l` == **0**。
- [ ] `grep -rln 'TODO: align with Python' rust/crates/ccb-daemon/src/ | wc -l` == **0**。
- [ ] `cargo check --workspace` 绿；`cargo test --workspace -- --test-threads=1` 全绿（与删除前一致，0 回归）。
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 0；`cargo fmt --check` clean。
- [ ] `plans/rust-python-test-parity-matrix.md` 加一条说明：stub 镜像已清理，Python 源码作为编译/运行期对照参考。

## 护栏（stop-rule）

- **只删 `TODO: align with Python` 空标记文件**；任何含真实 Rust item 的文件**不动**（canonical 实现、ccb-mailbox、测试等）。
- **每批 cargo check 必须绿**；一旦断裂，回退该批最近的删除，定位漏删的 `pub mod` 声明。
- **不碰 luck 的并行任务文件**（cli-ask-install-restart、daemon-startup-foreground-wait、providers-catalog-health-restore、e2e-terminal-edge）。
- 不改任何**功能代码**——本任务纯删除空镜像 + 对应声明，零行为变更。
- 提交格式：`chore(providers/daemon): remove empty 1:1 stub mirrors`；分批提交（每 crate / 每子目录一提交），便于回退。提交信息尾注 `Co-Authored-By: Claude <noreply@anthropic.com>`。

## 风险与回退

- **风险**：漏删某个 stub 的 `pub mod` 声明 → `cargo check` 报「file not found for module」→ 按报错定位补删该声明即可（机械修复）。
- **风险**：误删含真实代码的文件 → 仅当 (a)+(b) 两条件都满足才删，且每批 cargo check 把关，可立即发现。
- **回退**：分批提交，任一批出问题 `git revert <commit>` 即可。

## 参考

- `.trellis/tasks/06-24-py2rust-providers-daemon-deep/stub-triage.md` §2（为何是空镜像）/§3（零引用证明）/§6（决策：此前保留，现功能对齐已达，转删除）。
- `.trellis/tasks/06-24-py2rust-consistency-finishline/research/finishline-audit.md`（功能 parity 已达成的证据）。
- Python 源码（编译/运行期对照）：`lib/ccbd/`、`lib/provider_backends/`、`lib/cli/`、`lib/completion/`。
