# Native Host Bridge 设计

## 目标

实现第一版 `kunkka-native-host` 桥接能力，让 Browser Extension 通过 WebExtension Native Messaging 访问本地 `kunkka-core` 的最小 core-control 能力。

第一版只支持：

- `ping`
- `status`

## 背景

当前已实现：

- `kunkka-ipc`：frame protocol、opaque payload、postcard codec、Unix Domain Socket transport。
- `kunkka-core`：XDG runtime socket、single-connection runtime loop、worker registration、core-control `Ping/Pong` 和 `Status/StatusResult`。
- `kunkka-native-host`：仅有 skeleton binary。

Browser Extension 不能直接连接 Unix Domain Socket。它进入本地 Kunkka 系统只能通过 WebExtension Native Messaging 和 `kunkka-native-host`。

## 边界决策

### Shared protocol crate

新增 `crates/kunkka-protocol`，承载跨 crate 共享的 typed protocol 类型和 payload codec。

第一版 `kunkka-protocol` 拥有：

- `core-control v1` message 类型。
- core-control content type 和 schema 常量。
- core-control postcard payload encode/decode。

`kunkka-protocol` 可以依赖 `kunkka-ipc::Payload`，但不能拥有：

- IPC transport。
- UDS listener/connection。
- core runtime。
- permission decision。
- app business logic。
- database、LLM、filesystem、shell 等本地能力业务逻辑。

`kunkka-core` 迁移为使用 `kunkka_protocol::core_control::*`。当前项目没有外部 API 兼容压力，因此 `kunkka-core` 不需要保留 `control` re-export。

### Native host responsibility

`kunkka-native-host` 只做桥接：

```text
WebExtension Native Messaging JSON <-> Kunkka IPC core-control
```

它不能：

- 启动 `kunkka-core`。
- 做 permission decision。
- 做 app business logic。
- 访问数据库业务逻辑。
- 直接实现 LLM、filesystem、shell 等 core capability。
- 暴露通用 IPC frame envelope 给 Browser Extension。

## Native Messaging JSON API

第一版采用高层命令，不暴露 IPC frame 或 opaque payload。

请求示例：

```json
{ "id": "req-1", "command": "ping" }
{ "id": "req-2", "command": "status" }
```

成功响应示例：

```json
{ "id": "req-1", "ok": true, "result": { "type": "pong" } }
{ "id": "req-2", "ok": true, "result": { "type": "status", "worker_count": 1, "socket_path": "/run/user/1000/kunkka/core.sock", "runtime_ready": true } }
```

错误响应示例：

```json
{ "id": "req-3", "ok": false, "error": { "code": "core_unavailable", "message": "failed to connect core socket" } }
{ "id": null, "ok": false, "error": { "code": "invalid_request", "message": "missing request id" } }
```

规则：

- Request 必须包含 `id`。
- 能解析出 request `id` 时，response 必须原样返回同一个 `id`。
- 如果 JSON 非法或缺少 string `id`，错误 response 的 `id` 为 `null`。
- `command` 第一版只接受 `ping` 和 `status`。
- 每条有效 Native Messaging request 都应产生一条 response。
- stdin EOF 时 native-host 退出。

## Native Messaging I/O

stdin/stdout 使用 WebExtension Native Messaging 标准：

```text
4-byte little-endian length prefix + UTF-8 JSON body
```

stdout 只能写 length-prefixed JSON response。

stderr 可用于诊断日志，但第一版不要求结构化日志。

## Core IPC 连接策略

`kunkka-native-host` 是长驻进程。

为了支持 native-host 复用 core IPC connection，`kunkka-core` 的单连接 runtime loop 需要在同一个 accepted connection 上连续处理多个 request frame，直到该 connection EOF 或发生错误。

第一版仍不实现多连接并发；core 同一时间只处理一个 accepted connection。多 frontend/worker 并发连接是后续切片。

连接策略：

1. 启动后进入 Native Messaging read loop。
2. 首次需要转发 `ping` 或 `status` 时解析 core socket path，并连接 `kunkka-core`。
3. 成功连接后缓存 `IpcConnection`。
4. 后续请求复用同一条 core IPC connection。
5. 如果 send/recv/decode 失败，当前请求返回 JSON error，并清空缓存连接。
6. 下一条请求重新尝试连接 core。

`kunkka-native-host` 不自动启动 core。

如果 core socket 不存在或连接失败，当前请求返回 `core_unavailable`。

## Socket Path Resolution

Socket path resolution 放在 `kunkka-native-host` 内。

原因：socket path resolution 是进程入口配置问题，不是 protocol 语义，不应放入 `kunkka-protocol`。

第一版只解析 core socket path，不复制完整 `KunkkaPaths`。

规则必须与 core 当前 runtime socket 保持一致：

- 如果 `$XDG_RUNTIME_DIR` 是绝对路径，使用 `$XDG_RUNTIME_DIR/kunkka/core.sock`。
- 否则使用 `/tmp/kunkka-runtime-<uid>/core.sock`。

Native host 只解析路径，不创建目录，不修改权限，不存储持久数据。

未来如果 CLI/TUI 也需要共享 local runtime path helper，可再抽出独立 crate。该 helper 不属于 `kunkka-protocol`。

## Error Codes

第一版错误码保持小集合：

- `invalid_request`：JSON 格式错误、缺少 `id`、未知 `command`、Native Messaging length/body 非法。
- `core_unavailable`：无法连接 core socket。
- `core_ipc_error`：已连接但发送、接收或 IPC 解码失败。
- `unexpected_core_response`：core 返回的 core-control message 类型不符合当前 command 预期。

## Data Flow

`ping`：

```text
Browser Extension
  -> Native Messaging JSON { id, command: "ping" }
  -> kunkka-native-host
  -> Kunkka IPC Request with CoreControlMessage::Ping
  -> kunkka-core
  -> Kunkka IPC Response with CoreControlMessage::Pong
  -> kunkka-native-host
  -> Native Messaging JSON { id, ok: true, result: { type: "pong" } }
```

`status`：

```text
Browser Extension
  -> Native Messaging JSON { id, command: "status" }
  -> kunkka-native-host
  -> Kunkka IPC Request with CoreControlMessage::Status
  -> kunkka-core
  -> Kunkka IPC Response with CoreControlMessage::StatusResult
  -> kunkka-native-host
  -> Native Messaging JSON { id, ok: true, result: { type: "status", worker_count, socket_path, runtime_ready } }
```

## 测试策略

### `kunkka-protocol`

新增测试覆盖：

- core-control `Ping` payload roundtrip。
- core-control `StatusResult` payload roundtrip。
- payload `content_type` 和 `schema` metadata。

### `kunkka-core`

迁移到 `kunkka-protocol` 后，保留现有 runtime control tests，验证：

- `Ping -> Pong`。
- `Status -> StatusResult`。
- unknown schema 返回 invalid core frame。
- non-request core-control frame 返回 invalid core frame。
- response message sent as request 返回 invalid core frame。

### `kunkka-native-host`

新增 unit tests 覆盖：

- Native Messaging length-prefixed read/write。
- JSON request/response serde。
- `ping/status` JSON 与 core-control message 映射。
- socket path resolution。

新增 integration tests 覆盖：

- test core runtime + native-host session `ping` 返回 `pong`。
- 注册一个 worker 后，native-host session `status` 返回 `worker_count`、`socket_path`、`runtime_ready`。
- core unavailable 返回 `core_unavailable`。
- IPC failure 清空 cached connection，下一条 request 会重新连接。

## 非目标

- 不实现 Browser Extension manifest。
- 不安装 Native Messaging host manifest。
- 不自动启动 `kunkka-core`。
- 不实现 permission decision。
- 不支持通用 IPC envelope。
- 不支持 worker/app 业务请求。
- 不实现 CLI/TUI frontend。
