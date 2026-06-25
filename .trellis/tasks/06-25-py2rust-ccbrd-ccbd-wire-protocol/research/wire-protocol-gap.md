# ccbrd ↔ ccbd 线协议审计（glm5.2，2026-06-25）

## 已排除（线协议封皮级都 Python 兼容）
- **帧**：双方 newline 分隔 JSON，单请求单响应（Python `handle_connection` 也是单 shot，无 loop）。✓
- **请求解析**：ccbrd `RpcRequest` 双兼容 `op`/`request`（Python）+ `method`/`params`（CLI），`from_json` 归一化。✓
- **响应信封**：ccbrd `RpcResponse.payload` 用 `#[serde(flatten)]` → payload 摊平顶层（对齐 Python `to_record` 的 `record.update(payload)`）；`error`/`result` 用 `skip_serializing_if=Option::is_none`；含 `api_version`。✓
- **方法覆盖**：ccbrd 注册了 start/ask/submit/cancel/ping/project_view/queue/trace/watch/inbox/ack/resubmit/retry/stop-all/get/restore/attach/mailbox_head/shutdown + logs/cleanup/fault_*（Python 26 handler 基本都有）。✓

## 未排除（疑点，需抓流量定位）
1. **具体 RPC 的 payload 内容差**：sidebar 读某 RPC（如 project_view/ping/inbox/comms）的特定字段，ccbrd 没返回或形状不同 → sidebar 判定 unavailable。**最可能**。
2. **`RpcRequest` 的 `#[serde(deny_unknown_fields)]`**：Python 客户端若发额外字段，ccbrd 拒绝（Python 容忍）。可能性较低（标准客户端只发 api_version/op/request）。
3. sidebar 连接/握手特殊逻辑（如先探测某端点）。

## 下一步（抓流量定根因）
给 ccbrd `handle_one_connection` 加 `tracing::info!("CCBRD_RPC_RECV: {}", line.trim())`，重建，down/up（bootstrap 拉 sidebar），读 /tmp/ccbrd-dapro-ass.log 看 sidebar 实际发的 op + ccbrd 返回的 ok/error + payload，定位是哪个 RPC 哪个字段差。
