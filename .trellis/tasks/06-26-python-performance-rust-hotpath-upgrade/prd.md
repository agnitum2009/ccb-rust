# Python 最新版性能热点基于 ccb-legacy 的 Rust 模块替代路线

## Goal

在 `/home/agnitum/ccb-git` 这条已更新到 Python upstream `v7.7.0` 的源码线上，识别可被 Rust 局部模块替代的性能热点，并设计一条不会破坏现有 Rust 血系的升级路线：

- Python `ccb` 最新版继续作为最新能力与性能优化需求来源。
- `ccb-legacy` 作为 Python-compatible Rust 血系，保持对 Python `ccb` 的 100% 兼容验证载体。
- `ccbr` 作为 Rust 主线，当前已对齐 Python 7.5.2，需要通过有序 intake 升级到新版能力，而不是被 Python 最新版直接污染。

## Confirmed Facts

- `/home/agnitum/ccb-git` 已 fetch `origin` + `upstream` tags，并 fast-forward 到 upstream tag `v7.7.0`。
- `/home/agnitum/ccb-git` 当前 HEAD 为 `fdd11024`，`VERSION=7.7.0`，`git describe=v7.7.0`；未跟踪 `.codegraph/`、`o13-ccb-ops-unit/` 保留未动。
- `/home/agnitum/ccb` Rust 主线 `python-rust/rolepacks-versioning-translation` 已完成 7.5.2 线协议/sidebar/ask live smoke，并已 push 到 `agnitum/ccb-rust`。
- `/home/agnitum/ccb/ccb-legacy` 的 `ccb-legacy` 已同步等价 provider cache 修复并 push 到 `agnitum/ccb-rust`。
- 本任务实现工作树固定为 `/home/agnitum/ccb/ccb-legacy`；`/home/agnitum/ccb` 的 `ccbr` 只作为后续可选 intake 目标，不作为本轮替代实现线。
- Python 最新版已有 idle pressure 计划文档：`docs/plantree/plans/ccb-idle-resource-pressure/`。
- Python 最新版当前可见热点包括：
  - `storage/json_store.py` / `storage/atomic.py` 每次 save 都 atomic rewrite。
  - `ccbd/services/mount.py::refresh_heartbeat` 每 heartbeat 写 lease。
  - `ccbd/socket_server_runtime/loop.py` maintenance worker 以 daemon poll interval 驱动。
  - `ccbd/keeper_runtime/loop.py` keeper 默认 0.5s loop。
  - `ccbd/socket_server_runtime/loop.py` 仍有 accept worker + maintenance worker 两个 `while True`，默认 0.2s socket/worker poll。
  - `provider_backends/codex/bridge_runtime/service.py` 每 Codex agent 一个 bridge 进程，`CCB_BRIDGE_IDLE_SLEEP` 默认 0.05s。
  - `provider_backends/codex/comm_runtime/polling_runtime/reader_runtime/service_runtime.py` 对 Codex log/session 输出做 `reader._poll_interval` 轮询。
  - `provider_backends/codex/bridge_runtime/binding_runtime.py` binding tracker 线程默认 0.5s poll。
- Codex bridge 已使用 persistent FIFO reader + selectors，但仍保留 per-agent process、bridge idle timeout、binding/log polling；不能只把 readback parser 当作第一替换目标。

## Requirements

1. 建立先后逻辑：先用 Python latest 定合同，再在 `ccb-legacy` 做 Rust hotpath 替代，最后才考虑可选回流到 `ccbr`。
2. 明确三条线的边界：Python latest、`ccb-legacy`、`ccbr`。
3. Rust 模块替代必须以 `ccb-legacy` 为实现/验证主线：先用 Python latest golden tests 定合同，再在 `ccb-legacy` 落 Rust 模块，最后才考虑 `ccbr` 选择性吸收。
4. `ccbr` 不是现行 Python 性能替代基线；它只能在 `ccb-legacy` 证明 Python-compatible 后，按 owner surface / capability slice 做可选 intake。
5. 不禁用 Codex hooks，不混用 `.ccb` 与 `.ccbr` runtime state。
6. 第一批 Rust 替代必须瞄准 Python CPU 最大来源：`ccbd` control-plane loop 与 per-agent Codex bridge/polling loop。
7. 功能对齐，不对齐 Python 的 per-agent process + 0.05~0.2s 紧轮询实现。
8. Rust 替代后的目标架构应保持：单进程或少进程、事件驱动/多路等待、active-only polling、idle agent 近零开销。
9. provider transcript/readback parser 可以作为该 runtime loop 替代的一部分，但不能作为第一阶段唯一交付。

## Out of Scope For Planning Phase

- 立即修改 Python 或 Rust 代码。
- 立即将 `ccbr` 变更到 Python latest 全量语义。
- 引入 PyO3/maturin 依赖前未做 golden test 和热点评估。
- 合并 `ccb-legacy` 与 `ccbr` 血系。

## Acceptance Criteria

- [ ] 产出设计文档，说明 Python latest → ccb-legacy Rust module → optional ccbr intake 的升级顺序。
- [ ] 列出第一批 Rust 替代候选及明确优先级，其中第一目标必须覆盖 Python 高 CPU runtime loop。
- [ ] 第一 milestone 拆为：sidecar 协议壳 + fallback + baseline，Slice A Codex hot loop，Slice B ccbd maintenance hot loop。
- [ ] baseline 包含 2 Codex smoke 与 4+ Codex n14-like 压力场景，CPU 验收以 4+ 场景为准。
- [ ] 列出哪些性能问题先用 Python 小修，不急于 Rust 替代。
- [ ] 定义 golden test / compatibility gate，确保 Rust 模块替代首先服务 `ccb-legacy` 的 Python-compatible 目标。
- [ ] 定义 `ccbr` 的可选 intake 策略：仅在 `ccb-legacy` 证明后吸收共享 crate/设计，并保留 ccbr 已验证的单进程/active-only 架构优势。

## Decision Log

- 2026-06-26: 第一替代形态采用 Python `.ccb` 可调用的 Rust runtime sidecar/accelerator，不直接替换整个 `ccbd` daemon。Python 继续 owns socket/job/mailbox/public CLI；Rust sidecar 先接管高 CPU wait/poll/readback hot loop，并保留 Python fallback。
- 2026-06-26: 第一替换 milestone 必须覆盖两个 Python 高 CPU 持续轮询源：`ccbd` worker/maintenance loop 与 per-agent Codex bridge/comm/binding polling。Codex active-job observation 可以是第一代码切片，但 milestone 不算完成，直到 `ccbd` maintenance scheduling 也完成降频/事件化。
- 2026-06-26: 现行 Python 版本的 Rust 模块替换基线是 `ccb-legacy`，不是 `ccbr`。`ccbr` 不参与 Python `.ccb` runtime 替换验证；它只在 legacy 证明后选择性吸收共享 crate/设计。
- 2026-06-26: Rust sidecar 与 Python `ccbd` 的通信接口采用 Unix domain socket + JSON-RPC/JSONL frame。该 socket 属于 `ccb-legacy` / Python `.ccb` accelerator，不是 `ccbrd`。
- 2026-06-26: 第一 milestone 先在 `ccb-legacy` 完成 sidecar 协议壳、Python fallback、CPU baseline，再进入 Codex bridge / ccbd maintenance hot loop 替换。
- 2026-06-26: baseline/事实源固定为 `/home/agnitum/ccb-git` 当前 merge 后 Python latest；Rust 替代实现工作树固定为 `/home/agnitum/ccb/ccb-legacy` 的 `ccb-legacy`。
- 2026-06-26: sidecar 采用独立 runtime crate/binary：`rust/crates/ccb-runtime-accelerator`，二进制名 `ccb-runtime-accelerator`。不放入 `ccb-daemon`、`ccb-provider-*` 或 `rust/tools/`。
- 2026-06-26: baseline 使用两层：2 个 Codex agent 做可重复 smoke，4+ 个 Codex agent 做 n14-like 高 CPU 复现场景；milestone CPU 验收以 4+ 场景为准。

## Open Question

- 规划是否批准进入 Phase 2 实现 Slice 0？
