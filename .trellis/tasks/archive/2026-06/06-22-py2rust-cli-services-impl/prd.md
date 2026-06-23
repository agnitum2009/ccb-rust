# W1: CLI services impl + socket client parity

## Goal

Wave 1 architecture unlock. 接通 `dispatch → handle_xxx → render_xxx` 链：实现 `Phase2Services` trait（29 方法），通过 daemon socket client 调用 ccbd，让 CLI 端到端可跑真实命令。

## Context（2026-06-22 调研）

- `Phase2Services` trait（`crates/ccb-cli/src/phase2_runtime/handlers_ops.rs`）29 方法（`write_lines` + 28 service：kill/cleanup/clear/logs/maintenance/ps/doctor×2/fault×3/reload/restart/submit_ask/ping/pend/queue/trace/inbox/ack/watch/resubmit/retry/wait/cancel/validate_config/start_agents），**0 impl**。
- `crates/ccb-cli/src/ccbd.rs`（CLI 侧 daemon client）= **纯空 stub**。
- daemon 侧 socket runtime 部分 stub：`socket_server_runtime/{protocol,lifecycle,loop_,server}`、`client_runtime/{explicit,registry}`、`api_models_runtime/rpc`。

## Phased plan（多 session）

### Phase 1 — 调研 + client 基础（本 session 起）
- 读 Python `lib/cli/services/daemon.py` + `lib/ccbd/socket_client.py` 理解 RPC 协议（请求/响应格式、socket 路径、序列化）。
- 起步 `ccb-cli/ccbd.rs`：socket 连接 + 单个 RPC 往返（如 `ping`）。

### Phase 2 — daemon 侧 socket runtime 补全
- 补 `socket_server_runtime` / `client_runtime` 关键 stub，打通 daemon RPC 链路（若 daemon 侧已可用则跳过）。

### Phase 3 — Phase2Services impl
- 为 service struct impl 29 方法，每个映射到一个 daemon RPC，返回 `serde_json::Value` 供 render 消费。

### Phase 4 — 端到端验证
- `dispatch` 驱动 `ccb ps` / `ccb ping` 端到端，render 输出与 Python 一致。

## Acceptance criteria

- [ ] `ccb-cli/ccbd.rs` 实现可用的 daemon socket client（ping RPC 往返成功）。
- [ ] `Phase2Services` 有一个 impl struct，29 方法全部有实现（非 todo!()）。
- [ ] `dispatch` 端到端跑通至少 `ccb ps` + `ccb ping`，render 输出正确。
- [ ] `cargo build -p ccb-cli` + `cargo test -p ccb-cli --lib -- --test-threads=1` 全绿。
- [ ] `cargo clippy -p ccb-cli --all-targets` 0 error。

## Constraints

- 遵循 migration-roadmap spec 约定：`&serde_json::Value` 风格、禁 chrono/regex/reqwest、cargo 用 `--manifest-path` 或 `cd rust &&`。
- 真实 provider 交互仍 mock；RPC 用 socket（非 HTTP）。
- 每个 Phase 完成走 Trellis check→commit（本任务跨度大，可按 Phase 分 commit）。

## Out of scope

- provider launchers 编排（W2）、completion parity（W2）。
- 真实 provider CLI 实时交互。
