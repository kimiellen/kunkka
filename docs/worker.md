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

## 尚未实现

- worker process spawning
- heartbeat loop
- worker lifecycle restart policy
- request dispatch to worker
- worker streams
- worker cancellation
- SQLite persistence
- permission checks
