# Kunkka Workers

## 角色

App backend workers 是独立 Rust 进程，提供 app-specific backend logic。

Workers 通过基于 Unix Domain Socket 的 Kunkka IPC 连接 `kunkka-core`。

## 当前 worker registration

`kunkka-worker-sdk` 当前定义：

- `WorkerId`
- `AppId`
- `WorkerCapability`
- `RegisterWorkerRequest`
- `RegisterWorkerResponse`
- `WorkerProtocolMessage`
- `WorkerClient`

Worker registration payload 在 `kunkka-worker-sdk` 中建模，并序列化为 opaque IPC payload。

## 当前 core registry

`kunkka-core` 当前提供 in-memory `WorkerRegistry`。

当前行为：

- 注册 worker。
- 使用重复 worker ID 时替换已有记录。
- 通过 `WorkerId` 查询已注册 worker。
- 处理一个 worker registration request frame。
- core runtime loop 每次接受一个 IPC connection，并按 `Payload.schema` 分发。
- `kunkka.worker.v1` 返回 `RegisterWorkerAccepted`。
- `kunkka.core-control.v1` 当前由 core control protocol 处理，不属于 worker registration。

## Worker Dispatch

第一版 worker dispatch 是 core-internal API，不直接暴露给 native-host、CLI 或 TUI。

Dispatch 路由规则：

- `AppId` 是 dispatch 路由键。
- 第一版每个 `AppId` 只有一个 active worker。
- 同一 `AppId` 再注册时替换旧 active worker。
- 第一版 `WorkerId == AppId`。

App manifest 路径：

```text
$XDG_CONFIG_HOME/kunkka/apps/<app-id>.json
```

Core 在没有 active worker 时根据 manifest 拉起 worker 进程，并注入：

```text
KUNKKA_CORE_SOCKET
KUNKKA_APP_ID
KUNKKA_WORKER_ID
```

Dispatch request 使用 `method + Payload`。Core 不解释 app payload，worker 返回 success payload 或 app error `{ code, message }`。

## 尚未实现

- heartbeat loop
- worker lifecycle restart policy
- worker streams
- worker cancellation
- SQLite persistence
- permission checks
