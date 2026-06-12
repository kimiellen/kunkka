# Worker Dispatch Design

## Status

Approved for specification on 2026-06-12.

## Context

Kunkka 当前已经实现：

- `kunkka-ipc`：frame protocol、opaque `Payload`、postcard codec、UDS transport。
- `kunkka-core`：XDG path management、runtime socket setup、single-connection runtime loop、in-memory worker registry、core-control ping/status。
- `kunkka-worker-sdk`：worker registration protocol、payload codec、registration client。
- `kunkka-native-host`：WebExtension Native Messaging JSON 到 core-control IPC 的第一版 bridge。

当前尚未实现 worker request dispatch、worker process spawning、idle lifecycle、app registry、worker streams、cancellation、permission checks 和 SQLite persistence。

本设计定义下一阶段的 worker dispatch 和 worker lifecycle manager。目标是让 core 在收到内部 dispatch 请求时，能按需拉起 app backend worker、等待 worker 注册、转发请求、返回响应，并在空闲后释放 worker 进程。

## Goals

- `kunkka-core` 提供内部 `dispatch(app_id, method, payload)` API。
- `AppId` 是第一版 dispatch 路由键。
- 第一版每个 `AppId` 只有一个 active worker。
- 同一 `AppId` 再注册时替换旧 active worker。
- 第一版 `WorkerId == AppId`，后续如支持多实例再拆分。
- core 从 XDG config app manifest 读取 worker 启动配置。
- 没有 active worker 时，core 根据 manifest 拉起 worker 进程。
- core 等待 worker 连接并注册；超时或启动失败时返回平台错误。
- core 通过现有 Kunkka IPC connection 向 active worker 串行发送 dispatch request。
- worker response 支持 success payload 或 app error `{ code, message }`。
- worker idle 超时后，core 停止进程、关闭连接、移除 active entry。

## Non-Goals

- 不把 dispatch API 暴露给 native-host、CLI 或 TUI。
- 不实现 Browser Extension 到 app worker 的调用路径。
- 不实现 permission checks。
- 不实现 worker streams、cancellation、heartbeat 或 restart policy。
- 不实现 SQLite persistence。
- 不支持同一 `AppId` 多 active worker、负载均衡或并发多 in-flight dispatch。
- 不让 `kunkka-ipc` 承载 app/worker business semantics。

## Architecture Boundaries

`kunkka-core` owns worker dispatch as a local capability platform concern:

- app manifest discovery
- worker process lifecycle
- active worker registry
- dispatch routing
- idle cleanup

`kunkka-core` must not interpret app payload business semantics. It only routes by `AppId` and carries `method` plus opaque payload.

`kunkka-worker-sdk` owns worker-facing protocol helpers:

- worker registration message types
- worker dispatch request/response message types
- worker request receive/respond helpers

`kunkka-ipc` remains limited to frame, transport, serialization, codec, and opaque `Payload`.

Frontend forms remain out of scope for this slice. Browser Extension still enters only through `kunkka-native-host`, but `kunkka-native-host` does not call app workers in this design.

## App Manifest

Core reads app manifests from:

```text
$XDG_CONFIG_HOME/kunkka/apps/*.json
```

Resolved through existing `KunkkaPaths.config_dir`:

```text
<config_dir>/apps/<app-id>.json
```

First version manifest shape:

```json
{
  "app_id": "notes",
  "worker": {
    "program": "/usr/bin/notes-worker",
    "args": ["--serve"],
    "env": {
      "NOTES_ENV": "local"
    },
    "cwd": "/home/user"
  },
  "idle_timeout_ms": 300000,
  "startup_timeout_ms": 10000
}
```

Required fields:

- `app_id`
- `worker.program`
- `worker.args`

Optional fields:

- `worker.env`
- `worker.cwd`
- `idle_timeout_ms`
- `startup_timeout_ms`

Default values:

- `idle_timeout_ms`: `300000`
- `startup_timeout_ms`: `10000`

The manifest is configuration, not runtime state. Core does not write this file in the first version.

## Worker Startup Context

When core starts a worker process, it preserves manifest `args` and injects runtime context through environment variables:

```text
KUNKKA_CORE_SOCKET=<runtime core.sock path>
KUNKKA_APP_ID=<app-id>
KUNKKA_WORKER_ID=<app-id>
```

First version uses `WorkerId == AppId`. This keeps the single-active-worker model explicit. If Kunkka later supports multiple worker instances for one app, worker instance identity will be split from app identity in a later design.

## Worker Protocol

`kunkka-worker-sdk` extends `WorkerProtocolMessage` with dispatch messages.

Conceptual message shape:

```rust
pub struct DispatchWorkerRequest {
    pub app_id: AppId,
    pub method: String,
    pub payload: kunkka_ipc::Payload,
}

pub enum DispatchWorkerResponse {
    Ok(kunkka_ipc::Payload),
    Err(WorkerAppError),
}

pub struct WorkerAppError {
    pub code: String,
    pub message: String,
}

pub enum WorkerProtocolMessage {
    RegisterWorker(RegisterWorkerRequest),
    RegisterWorkerAccepted(RegisterWorkerResponse),
    DispatchWorker(DispatchWorkerRequest),
    DispatchWorkerResult(DispatchWorkerResponse),
}
```

`method` is app-owned. Core does not validate or interpret it beyond requiring it to be non-empty.

`payload` reuses `kunkka_ipc::Payload` to preserve opaque bytes, `content_type`, `schema`, and payload metadata.

## Core Components

### AppRegistry

`AppRegistry` scans `<config_dir>/apps/*.json` and stores `AppManifest` by `AppId`.

Responsibilities:

- load all JSON manifests
- validate required fields
- expose `get(app_id)`
- report `app_not_found` and `manifest_invalid`

First version loads manifests during core runtime preparation. File watching and runtime reload are out of scope.

### WorkerManager

`WorkerManager` owns active worker lifecycle.

Responsibilities:

- resolve `AppId` to manifest
- start worker when no active worker exists
- wait for matching registration
- replace old worker when same `AppId` registers again
- send dispatch request over active worker IPC connection
- enforce one in-flight dispatch per active worker
- update `last_used_at` after successful dispatch or app error response
- stop idle workers after `idle_timeout_ms`
- remove workers after IPC/protocol failures

### Runtime Connection Ownership

Live dispatch requires core to retain the worker IPC connection after registration. The core runtime accepts a worker connection, decodes `RegisterWorker`, writes `RegisterWorkerAccepted`, and hands the still-open `IpcConnection` to `WorkerManager` as part of `ActiveWorker`.

When dispatch starts a worker process, it waits for this registration handoff. The first version does not expose dispatch to frontend connections, so the implementation plan can keep this coordination internal to core tests and core runtime APIs before adding frontend dispatch entrypoints later.

### ActiveWorker

`ActiveWorker` contains:

- `app_id`
- `worker_id`
- child process handle
- worker IPC connection
- `last_used_at`
- `idle_timeout_ms`
- exclusive mutable access to the worker connection while a dispatch is in flight

The first version stores active workers in a map keyed by `AppId`.

### Dispatch API

Core exposes an internal API shaped like:

```rust
pub async fn dispatch(
    &mut self,
    app_id: AppId,
    method: String,
    payload: kunkka_ipc::Payload,
) -> Result<DispatchResult>;
```

Conceptual result:

```rust
pub enum DispatchResult {
    Ok(kunkka_ipc::Payload),
    AppError { code: String, message: String },
}
```

Platform failures return `CoreError` variants, not `DispatchResult::AppError`.

## Data Flow

### Cold Dispatch

1. A core internal caller invokes `dispatch(app_id, method, payload)`.
2. `WorkerManager` checks active workers by `AppId`.
3. If no active worker exists, `AppRegistry` resolves the manifest.
4. Core starts the worker process from manifest command fields.
5. Core injects `KUNKKA_CORE_SOCKET`, `KUNKKA_APP_ID`, and `KUNKKA_WORKER_ID`.
6. Worker connects to core over UDS and sends `RegisterWorker`.
7. Core waits up to `startup_timeout_ms` for matching `AppId` / `WorkerId` registration.
8. On registration success, core stores `ActiveWorker`.
9. Core sends `DispatchWorker` to the active worker connection.
10. Worker returns `DispatchWorkerResult`.
11. Core returns `DispatchResult` or platform error to the internal caller.
12. Core updates `last_used_at` when the worker returns success payload or app error.

### Warm Dispatch

1. `AppId` already has active worker.
2. Core sends `DispatchWorker` over the existing connection.
3. Core waits for the matching response frame.
4. Core returns success payload or app error.

The first version allows only one in-flight dispatch per active worker.

### Replacement Registration

If a worker registers with an `AppId` that already has an active worker:

1. Core treats the new worker as replacement.
2. Core terminates the old child process when it owns a child handle.
3. Core drops the old worker IPC connection.
4. Core stores the new worker as active entry for that `AppId`.

### Idle Cleanup

Core periodically checks active workers:

1. If `now - last_used_at >= idle_timeout_ms`, core stops the worker process.
2. Core drops the worker IPC connection.
3. Core removes the active worker entry.
4. A later dispatch for the same `AppId` goes through cold dispatch again.

## Error Handling

Core platform errors:

- `app_not_found`: no manifest for `AppId`.
- `manifest_invalid`: manifest JSON or required fields are invalid.
- `worker_start_failed`: `Command` failed to start the worker process.
- `worker_start_timeout`: worker process did not register within `startup_timeout_ms`.
- `worker_unavailable`: worker connection is absent, closed, or unusable.
- `dispatch_ipc_error`: send/receive over worker IPC failed.
- `unexpected_worker_response`: wrong frame type, request_id mismatch, or wrong worker protocol message.

Worker app errors:

- Worker returns `DispatchWorkerResponse::Err { code, message }`.
- Core does not interpret or rewrite `code` and `message`.
- App error does not remove the active worker.

State cleanup rules:

- Startup failure: no active worker is stored.
- Startup timeout: terminate the spawned process if it is still running, and store no active worker.
- Dispatch IPC/protocol failure: remove active worker, terminate the child process when present, and drop the worker IPC connection.
- Worker app error: keep active worker and update `last_used_at`.

## Testing Strategy

Use TDD and keep tests layered.

Manifest tests:

- load valid JSON manifest from temporary XDG config directory
- reject missing required fields
- reject invalid JSON
- verify `worker.env` and `worker.cwd` are optional
- verify `idle_timeout_ms` and `startup_timeout_ms` use defaults when omitted

Registry tests:

- registry stores one active worker per `AppId`
- same `AppId` registration replaces old worker
- lookup by `AppId` returns the active worker

Protocol tests:

- dispatch request payload codec roundtrips
- dispatch success response roundtrips
- dispatch app error response roundtrips

Warm dispatch tests:

- pre-register active worker and dispatch over existing IPC connection
- preserve request_id/session/source/target behavior
- reject non-response frame or mismatched request_id

Cold dispatch tests:

- manifest points to test worker fixture
- core starts process when no active worker exists
- worker registers through SDK
- dispatch returns payload response

Failure tests:

- manifest missing returns `app_not_found`
- invalid manifest returns `manifest_invalid`
- invalid executable returns `worker_start_failed`
- worker that never registers returns `worker_start_timeout`
- worker app error returns `DispatchResult::AppError`
- worker disconnect removes active entry

Idle tests:

- short `idle_timeout_ms` stops worker and removes active entry
- later dispatch restarts worker from manifest

## Implementation Notes

The implementation plan should split this design into smaller tasks. A likely order:

1. Manifest types and JSON loader.
2. Worker protocol dispatch messages and codec tests.
3. Active worker registry keyed by `AppId` with replacement behavior.
4. Warm-path dispatch over an already registered worker connection.
5. Worker process startup and registration wait.
6. Cold dispatch from manifest to worker response.
7. Idle cleanup.
8. Documentation and full verification.

This ordering keeps each task testable without exposing the dispatch API to frontends yet.
