# ccbrd 完整实现 ccbd 线协议（Python 客户端互操作 parity）

## Goal
让 ccbrd 完整实现 Python `ccbd` 的线协议（socket RPC），使 **Python 客户端**（`ccb-agent-sidebar` 左侧栏、`ask` skill、Python `ccb` CLI）能与 ccbrd 互操作。这是 ccbr 真正可用于多 agent 场景的**核心剩余 py→rust 工程**。

## 背景与诊断（glm5.2 实查，2026-06-25）
- ccbrd 在跑、socket 可达；**Rust CLI（`ccbr`）↔ ccbrd 协议通**（`status/ask/trace/inbox/restart` 等都工作，本会话验证）。
- 但 **Python `ccb-agent-sidebar` 连上 ccbrd socket 后报 "ccbd unavailable"**、Python `ask` skill 不可用 → codex agent 无机制跨 agent 通信（幻觉 agentmemory）。
- 根因：ccbrd 实现了**Rust 侧 RPC**（与 Rust CLI 同族），但 **Python ccbd 客户端依赖的部分 RPC 端点未实现或线协议/响应结构不一致**。矩阵标的 parity 多是 Rust 侧测试/结构，**非与 Python 客户端的真实互操作**。

## 范围（P0，多 session）
1. **RPC 接口面审计（第一步，必做）**：枚举 Python `ccbd` 服务的全部 RPC 方法（`lib/ccbd/handlers/` + 主分发器的方法注册表）+ ccbrd 现有 dispatch（`rust/crates/ccbr-daemon/src/` 的方法匹配），产出**缺口清单**（Python 有、ccbrd 无/响应不一致的端点）。重点核对 sidebar/ask/mailbox/comms/namespace 相关方法。
2. **补齐缺失端点**：在 ccbrd 实现 Python 客户端依赖的 RPC（线协议格式 + 请求/响应结构严格对齐 `lib/ccbd/handlers/`）。
3. **Python 客户端互操作验证**：用真实 `ccb-agent-sidebar`（软链 `ccbr-agent-sidebar`）+ Python `ask` skill 连 ccbrd，验证左侧栏显示真实状态 + agent 间 `ask` 通信可用。
4. 测试：ccbrd 新增 RPC 的单测 + Python 客户端互操作集成测试。

## Python 参考位置
- 守护进程主入口/handlers：`lib/ccbd/`（`lib/ccbd/handlers/` 各 handler + 主 socket 分发器的 method 注册表）。
- 客户端调用面：`bin/ccb-agent-sidebar`（编译产物，源在 Python `ccb` 包）+ `ask` skill。
- ccbrd 现有 dispatch：`rust/crates/ccbr-daemon/src/`（handlers + 主 socket 路由）。

## Acceptance Criteria
- [ ] RPC 接口面审计文档（research/wire-protocol-gap.md：Python 全量方法 vs ccbrd 现有 vs 缺口）
- [ ] ccbrd 补齐缺口端点，线协议/响应结构与 Python ccbd 严格一致
- [ ] `ccb-agent-sidebar` 连 ccbrd 不再报 "ccbd unavailable"，显示真实 agent/mailbox/comms 状态
- [ ] Python `ask` skill 经 ccbrd 完成 agent 间通信（A ask B → B 回复 → A inbox 收到）
- [ ] `cargo test -p ccbr-daemon` 全绿 + Python 客户端互操作集成测试通过

## Notes
- 这是 ccbr 从"Rust-CLI 可用"升级到"Python 客户端可互操作"的核心里程碑，规模大，建议拆子任务按 RPC 簇推进（sidebar-view / mailbox-comms / namespace / ask-chain 等）。
- 前置已就位：`ccbr-agent-sidebar` 软链、`run-ccbr.sh`（mouse on + sidebar bootstrap）、dapro-ass `[ui.sidebar]` 配置——线协议通了之后这些即可用。
