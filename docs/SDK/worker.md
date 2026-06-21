# Kunkka Workers

## 角色

App backend workers 是独立 Rust 进程，提供 app-specific backend logic。

Workers 通过基于 Unix Domain Socket 的 Kunkka IPC 连接 `kunkka-core`。

## Worker Registration

`kunkka-worker-sdk` 提供：

- `WorkerId` / `AppId` — 强类型 ID
- `WorkerCapability` — worker 声明的能力
- `RegisterWorkerRequest` / `RegisterWorkerResponse` — 注册协议
- `WorkerProtocolMessage` — 消息枚举
- `WorkerClient` — 连接 core、注册、接收 dispatch、发送响应

Worker 连接 core 后发送 `kunkka.worker.v1` schema 的注册帧。Core 接受后返回 `RegisterWorkerAccepted`，worker 进入等待 dispatch 状态。

## Core Registry

`kunkka-core` 提供 in-memory `WorkerRegistry`：

- 注册 worker，使用重复 worker ID 时替换已有记录
- 通过 `WorkerId` 查询已注册 worker
- 按 `Payload.schema` 分发：`kunkka.worker.v1` → worker registration

## Worker Dispatch

Dispatch 路由规则：

- `AppId` 是 dispatch 路由键
- 每个 `AppId` 只有一个 active worker
- 同一 `AppId` 再注册时替换旧 active worker

Core 在没有 active worker 时根据 manifest 拉起 worker 进程，并注入：

```text
KUNKKA_CORE_SOCKET
KUNKKA_APP_ID
KUNKKA_WORKER_ID
```

Dispatch request 使用 `method + Payload`。Core 不解释 app payload，worker 返回 success payload 或 app error `{ code, message }`。

Idle worker 由 core 自动清理（100ms 检查间隔）。

## App Manifest

App manifest 路径：

```text
$XDG_CONFIG_HOME/kunkka/apps/<app-id>.json
```

Manifest 结构：

```json
{
  "app_id": "notes",
  "worker": {
    "program": "notes-worker",
    "args": ["--verbose"],
    "env": {}
  },
  "permissions": {
    "frontend_dispatch": {
      "allowed_methods": ["search", "create", "delete"]
    }
  },
  "capabilities": {
    "fs": {
      "paths": ["/home/user/notes"]
    },
    "http": {
      "domains": ["api.example.com"]
    },
    "shell": {
      "allow": ["rg", "wc"],
      "ask": ["curl"]
    }
  }
}
```

## Capability 层

Worker 通过 `call_capability()` 反向调用 core 能力层。当前支持：

| Capability | Schema | 功能 |
|-----------|--------|------|
| `fs` | `kunkka.capability.v1` | `read_file`、`write_file`、`list_dir`，路径白名单校验 |
| `http` | `kunkka.capability.v1` | 外部 HTTP 请求，域名白名单，30s 超时 |
| `sqlite` | `kunkka.capability.v1` | per-app SQLite 数据库，`open`/`query`/`execute`/`close` |
| `shell` | `kunkka.capability.v1` | 受限 shell 执行，三态策略（allow/ask/deny）+ 审批流 |
| `llm` | `kunkka.capability.v1` | Chat（流式/非流式）、Embeddings、Images，角色路由 |

## Worker SDK

`kunkka-worker-sdk` 提供的客户端 API：

```rust
// 注册
let mut client = WorkerClient::connect(&socket_path, worker_id).await?;
client.register(RegisterWorkerRequest { ... }).await?;

// 接收 dispatch
let ctx = client.recv_dispatch().await?;
// ctx.request.method, ctx.request.payload

// 发送响应
client.respond_dispatch(ctx, DispatchWorkerResponse::Ok(payload)).await?;

// 调用 core 能力
let response = call_capability(&socket_path, &app_id, "fs", "read_file", params).await?;
let mut stream = open_capability_stream(&socket_path, &app_id, "llm", "chat", params).await?;

// LLM 便捷封装
let chat = collect_llm_chat(&socket_path, &app_id, LlmChatParams { ... }).await?;
let mut stream = open_llm_chat_stream(&socket_path, &app_id, LlmChatParams { ... }).await?;
let embeddings = call_llm_embeddings(&socket_path, &app_id, LlmEmbeddingsParams { ... }).await?;
let images = call_llm_images(&socket_path, &app_id, LlmImagesParams { ... }).await?;
```

## 尚未实现

- heartbeat loop（worker 崩溃检测）
- worker lifecycle restart policy（重启策略）
- worker cancellation（取消正在处理的请求）
- shell 命令执行超时
