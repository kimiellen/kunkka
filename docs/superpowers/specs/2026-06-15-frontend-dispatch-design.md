# Frontend Dispatch 设计

## 状态

已在 2026-06-15 批准用于规格化。

## 背景

Kunkka 当前已经实现：

- `kunkka-ipc`：frame protocol、opaque `Payload`、postcard codec、Unix Domain Socket transport。
- `kunkka-protocol`：shared core-control protocol。
- `kunkka-core`：XDG runtime、core-control、app manifest registry、worker startup、active worker registry、core-internal worker dispatch 和 idle cleanup。
- `kunkka-worker-sdk`：worker registration/dispatch protocol、payload codec、registration client 和 dispatch receive/respond helpers。
- `kunkka-native-host`：WebExtension Native Messaging JSON 到 core-control IPC 的第一版 bridge，支持 `ping` 和 `status`。

当前 worker dispatch 仍是 core-internal API。Browser Extension 只能通过 `kunkka-native-host` 进入本地系统，不能直接连接 Unix Domain Socket，也不能直接调用 app backend worker。下一阶段需要让 Browser Extension 通过 Native Messaging 发起 app dispatch，同时保持权限决策和 worker lifecycle 都由 `kunkka-core` 管控。

## 目标

- 新增 `frontend-dispatch v1` shared protocol，让 frontend form 可以通过 core 请求 app worker dispatch。
- 第一版优先支持 `kunkka-native-host` 的 Native Messaging JSON `dispatch` 命令。
- Native Messaging 只接收 JSON payload，不暴露 IPC frame、postcard、worker protocol 或 UDS 细节。
- `kunkka-native-host` 将 JSON payload 编码为 `kunkka_ipc::Payload`，`content_type = application/json`。
- `kunkka-core` 按 `Payload.schema = kunkka.frontend-dispatch.v1` 分发请求，并调用现有 `WorkerManager::dispatch_with_start`。
- 第一版在 `kunkka-core` 内部显式允许 frontend dispatch，作为后续权限系统的临时替换点。
- Worker app error 和 core platform error 必须在协议层区分。

## 非目标

- 不修改 `kunkka-ipc`。
- 不修改 worker-facing dispatch protocol。
- 不新增 CLI 或 TUI frontend。
- 不实现完整 permission system。
- 不支持 binary/base64 payload envelope。
- 不支持 stream、cancel、heartbeat 或 worker restart policy。
- 不支持 native-host 自动启动 `kunkka-core`。
- 不暴露通用 IPC frame envelope 给 Browser Extension。
- 不让 `kunkka-native-host` 做最终权限判断或 app business logic。

## 架构边界

`kunkka-protocol` 拥有跨 crate 共享的 typed frontend-dispatch protocol 和 payload codec。它只承载 protocol message 和 postcard 编解码，不拥有 IPC transport、runtime、permission decision、worker lifecycle 或 app business logic。

`kunkka-core` 拥有 frontend dispatch 的本地能力平台行为：

- 校验 frontend-dispatch request。
- 执行 core 内部的临时 allow decision。
- 将 wire `app_id` 转换为 core/worker dispatch 使用的 `AppId`。
- 调用 `WorkerManager::dispatch_with_start`。
- 将 worker success、worker app error、core platform error 转换为 frontend-dispatch response。

`kunkka-native-host` 只做桥接：

```text
WebExtension Native Messaging JSON <-> Kunkka IPC frontend-dispatch/core-control
```

它不能直接连接 app worker、不能访问本地 capability、不能读取或修改 app payload business semantics、不能做权限决策、不能自动启动 core。

`kunkka-worker-sdk` 继续只拥有 worker-facing registration/dispatch helper。本切片不把 frontend-dispatch protocol 放入 worker SDK。

## Frontend Dispatch Protocol

在 `kunkka-protocol` 中新增 `frontend_dispatch` 模块。

Payload metadata：

```text
content_type = application/vnd.kunkka.frontend-dispatch.v1+postcard
schema = kunkka.frontend-dispatch.v1
```

概念类型：

```rust
pub const FRONTEND_DISPATCH_CONTENT_TYPE: &str =
    "application/vnd.kunkka.frontend-dispatch.v1+postcard";
pub const FRONTEND_DISPATCH_SCHEMA: &str = "kunkka.frontend-dispatch.v1";

pub struct FrontendDispatchRequest {
    pub app_id: String,
    pub method: String,
    pub payload: kunkka_ipc::Payload,
}

pub enum FrontendDispatchResponse {
    Ok(kunkka_ipc::Payload),
    AppError { code: String, message: String },
    PlatformError { code: String, message: String },
}

pub enum FrontendDispatchMessage {
    Dispatch(FrontendDispatchRequest),
    DispatchResult(FrontendDispatchResponse),
}
```

`app_id` 在 shared protocol 中使用 `String`，避免 `kunkka-protocol` 依赖 `kunkka-worker-sdk`。`kunkka-core` 负责校验非空后转换为 `kunkka_worker_sdk::AppId`。

Request 规则：

- `app_id` 必须是非空字符串。
- `method` 必须是非空字符串。
- `payload` 是 opaque `Payload`，core 不解释 app business semantics。
- 第一版 Native Messaging 入口只生成 JSON payload，但 protocol 本身保留 `Payload` 以便未来 CLI/TUI 或其他 frontend form 复用。

Response 规则：

- `Ok(payload)` 表示 worker 正常返回 success payload。
- `AppError` 表示 worker 正常处理请求，但 app backend 返回业务错误。
- `PlatformError` 表示 core、manifest、worker lifecycle、permission、protocol 或 dispatch transport 失败。

## Native Messaging JSON API

`kunkka-native-host` 在现有 `ping`、`status` 之外新增 `dispatch` 命令。

请求示例：

```json
{
  "id": "req-1",
  "command": "dispatch",
  "app_id": "notes",
  "method": "search",
  "payload": { "query": "hello" }
}
```

Native host 将 `payload` JSON value 序列化为 bytes，并构造：

```text
Payload.content_type = application/json
Payload.schema = null
Payload.metadata = {}
Payload.bytes = serde_json(payload)
```

成功响应示例：

```json
{
  "id": "req-1",
  "ok": true,
  "result": {
    "type": "dispatch",
    "payload": { "items": [] }
  }
}
```

Worker app error 响应示例：

```json
{
  "id": "req-1",
  "ok": true,
  "result": {
    "type": "dispatch_error",
    "code": "not_found",
    "message": "note not found"
  }
}
```

App error 使用 `ok: true`，因为 core、transport 和 worker dispatch 都成功完成，错误属于 app backend business result。

Core platform error 响应示例：

```json
{
  "id": "req-1",
  "ok": false,
  "error": {
    "code": "app_not_found",
    "message": "app not found: notes"
  }
}
```

Native request validation：

- Request 必须包含非空 string `id`。
- `dispatch` 必须包含非空 string `app_id`。
- `dispatch` 必须包含非空 string `method`。
- `dispatch` 必须包含 `payload` 字段，且该字段可以是任意 JSON value，包括 object、array、string、number、boolean 或 null。
- `ping/status` 不要求 `app_id`、`method` 或 `payload`。

Native success payload validation：

- 第一版 native-host 只支持把 worker success payload 解码为 JSON。
- 如果 `DispatchResult::Ok(payload)` 不是 `application/json` 或 bytes 不是合法 JSON，native-host 返回 `unexpected_core_response`。
- native-host 不解释 JSON payload 内部字段。

## Core Runtime 分发

Core runtime 需要支持同一 frontend IPC connection 上连续处理 core-control 和 frontend-dispatch request。原因是 `kunkka-native-host` 会缓存 core IPC connection，并可能在同一连接上先发送 `ping/status`，再发送 `dispatch`。

Runtime 分发规则：

- worker registration connection 仍按 `kunkka.worker.v1` 识别，并在 registration accepted 后交给 `WorkerManager` 持有。
- frontend connection loop 对每个 request frame 按 `Payload.schema` 分发：
  - `kunkka.core-control.v1` 调用现有 core-control handler。
  - `kunkka.frontend-dispatch.v1` 调用新的 frontend-dispatch handler。
  - 未知 schema 返回 `CoreError::InvalidCoreFrame`。
- frontend connection loop 保持 idle reap tick，继续清理空闲 worker。

Frontend-dispatch handler 行为：

1. 要求 frame 是 `Frame::Request`。
2. Decode `FrontendDispatchMessage::Dispatch`。
3. 校验 `app_id` 和 `method` 非空。
4. 调用 core 内部 `allow_frontend_dispatch_v1()`。
5. 构造 `AppId` 并调用 `WorkerManager::dispatch_with_start`。
6. 将 `DispatchResult::Ok` 转换为 `FrontendDispatchResponse::Ok`。
7. 将 `DispatchResult::AppError` 转换为 `FrontendDispatchResponse::AppError`。
8. 将 core dispatch platform error 转换为 `FrontendDispatchResponse::PlatformError`。
9. 用同一个 `request_id` 返回 `Frame::Response`。

Malformed frame、无法 decode 的 payload、非 request frame 和非 dispatch message 属于 protocol failure，可以返回 `CoreError::InvalidCoreFrame` 并关闭当前 connection。业务可恢复的平台错误通过 `PlatformError` 返回。

## 临时权限策略

`worker invocation` 是受控能力，最终 permission decision 必须属于 `kunkka-core`。

第一版不实现完整 permission system，但必须把权限替换点放在 core 内部：

```rust
fn allow_frontend_dispatch_v1(_request: &FrontendDispatchRequest) -> bool {
    true
}
```

规则：

- native-host 不做 allow/deny 决策。
- Browser Extension 不持有本地 capability 权限。
- core handler 必须显式调用该函数。
- 后续 permission system 落地时替换该函数，不改变 frontend-dispatch wire protocol。
- 预留 `permission_denied` platform error code，但第一版不会触发。

## Error Handling

Frontend-dispatch `PlatformError.code` 使用稳定 snake_case 字符串。

第一版 core platform error codes：

- `invalid_request`：frontend-dispatch request 缺少 `app_id`、缺少 `method` 或 message 类型不符合预期。
- `permission_denied`：预留给后续权限系统。
- `app_not_found`：没有找到 `AppId` 对应 manifest。
- `worker_start_failed`：worker process 启动失败。
- `worker_start_timeout`：worker 未在 `startup_timeout_ms` 内注册。
- `worker_unavailable`：active worker 缺失、连接关闭或不可用。
- `dispatch_ipc_error`：core 与 worker dispatch IPC 失败。
- `unexpected_worker_response`：worker response frame 或 worker protocol message 不符合预期。
- `core_error`：其他未分类 core platform error。

Native-host error behavior：

- core socket 不存在或连接失败：返回 `core_unavailable`。
- IPC send/recv/decode failure：返回 `core_ipc_error` 并清空 cached connection。
- core 返回非 frontend-dispatch response 或 response request id mismatch：返回 `unexpected_core_response` 并清空 cached connection。
- core 返回 `PlatformError`：native-host 透传 `code/message` 到 Native Messaging error body，不清空连接。
- native request JSON 无法 decode 或缺少 dispatch required fields：返回 `invalid_request`。

Worker app error behavior：

- core 不解释或改写 app error code/message。
- native-host 将 app error 表示为 `ok: true` 的 `dispatch_error` result。
- app error 不导致 core 移除 active worker。

## 数据流

### Native Dispatch Success

```text
Browser Extension
  -> Native Messaging JSON { id, command: "dispatch", app_id, method, payload }
  -> kunkka-native-host validates request and builds JSON Payload
  -> Kunkka IPC Request with FrontendDispatchMessage::Dispatch
  -> kunkka-core frontend-dispatch handler
  -> allow_frontend_dispatch_v1
  -> WorkerManager::dispatch_with_start
  -> app backend worker DispatchWorker
  -> worker DispatchWorkerResult::Ok(Payload)
  -> kunkka-core FrontendDispatchResponse::Ok(Payload)
  -> kunkka-native-host JSON result { type: "dispatch", payload }
```

### Native Dispatch App Error

```text
app backend worker
  -> DispatchWorkerResult::Err { code, message }
  -> kunkka-core FrontendDispatchResponse::AppError { code, message }
  -> kunkka-native-host JSON result { type: "dispatch_error", code, message }
```

### Native Dispatch Platform Error

```text
kunkka-core dispatch platform failure
  -> FrontendDispatchResponse::PlatformError { code, message }
  -> kunkka-native-host JSON error { code, message }
```

## 测试策略

使用 TDD，按 crate 分层。

`kunkka-protocol` tests：

- frontend-dispatch request payload metadata 和 roundtrip。
- success response roundtrip。
- app error response roundtrip。
- platform error response roundtrip。

`kunkka-core` tests：

- frontend-dispatch request over runtime connection 调用 warm active worker。
- frontend-dispatch app error 返回 `FrontendDispatchResponse::AppError`。
- manifest missing 映射为 `PlatformError { code: "app_not_found" }`。
- empty `app_id` 或 empty `method` 映射为 `PlatformError { code: "invalid_request" }`。
- non-request frontend-dispatch frame 返回 invalid core frame。
- 同一 frontend connection 上连续发送 `status` 和 `dispatch` 均可处理。
- 现有 `ping/status` 测试继续通过。

`kunkka-native-host` tests：

- dispatch JSON request decode 和 required field validation。
- dispatch JSON payload 转 `application/json` opaque payload。
- frontend-dispatch success response 转 Native Messaging `dispatch` result。
- frontend-dispatch app error response 转 Native Messaging `dispatch_error` result。
- frontend-dispatch platform error response 转 Native Messaging error。
- success payload 非 JSON 时返回 `unexpected_core_response`。
- core unavailable、IPC failure、request id mismatch 继续遵守现有 connection cache 清理策略。

Integration tests：

- test core runtime + active worker fixture + native-host session，Native Messaging `dispatch` 返回 worker JSON payload。
- 同一 native-host session 先 `status` 再 `dispatch`，验证 cached connection 可复用。

## 实施备注

建议实施顺序：

1. 在 `kunkka-protocol` 添加 frontend-dispatch message、codec 和 roundtrip tests。
2. 在 `kunkka-core` 添加 frontend-dispatch handler 和 platform error mapping tests。
3. 调整 core frontend connection loop，使同一 connection 支持 core-control 和 frontend-dispatch。
4. 在 `kunkka-native-host` 添加 dispatch request validation 和 JSON payload conversion tests。
5. 在 native-host bridge/session 中发送 frontend-dispatch request 并转换 response。
6. 添加 native-host 到 core runtime 的 dispatch integration test。
7. 更新 README、architecture、IPC、browser-extension、permissions 和 development log。
8. 运行 workspace fmt、test、clippy 验证。

该顺序保持 `kunkka-ipc` 不变，并优先验证 protocol 和 core runtime 边界，再接入 Native Messaging JSON。
