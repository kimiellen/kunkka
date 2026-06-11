# Core Control Protocol 设计

## 目标

添加一个最小 core 级控制协议，用于运行时健康检查和状态查询。

## 背景

当前实现已经支持：

- `kunkka-ipc` 中的 IPC frame 传输
- `kunkka-core` 中的 core runtime loop
- `kunkka-worker-sdk` 中的 worker registration typed payload
- `CoreRuntime::run_once` 中的单连接 worker registration 处理

目前还没有通用的 core control request。未来 CLI 和 native-host entrypoint 需要一种共同方式查询 core 是否可达，以及 core 的基本运行状态。

## 边界决策

第一版 core control protocol 放在 `kunkka-core` 内。

不能放入 `kunkka-ipc`，因为 IPC 只拥有 frame、transport、serialization 和 opaque payload。

不能放入 `kunkka-worker-sdk`，因为 worker SDK 拥有 worker registration 和 worker-facing helper，不拥有 core management 语义。

未来如果多个 frontend crate 需要直接共享这些类型，可以再引入 shared protocol crate。

## 协议

新增 `crates/kunkka-core/src/control.rs`。

Control payload metadata：

```text
content_type = application/vnd.kunkka.core-control.v1+postcard
schema = kunkka.core-control.v1
```

消息类型：

```rust
pub enum CoreControlMessage {
    Ping(CorePingRequest),
    Pong(CorePingResponse),
    Status(CoreStatusRequest),
    StatusResult(CoreStatusResponse),
}
```

`Ping` 返回 `Pong`。

`Status` 返回：

- `worker_count`
- `socket_path`
- `runtime_ready`

## Runtime 分发

`CoreRuntime::run_once()` 读取一个 request frame 后调用 `CoreRuntime::handle_frame(frame)`。

分发依据为 `Payload.schema`：

- `kunkka.worker.v1` 使用 `handle_worker_registration_frame`
- `kunkka.core-control.v1` 使用 core control handler
- 未知 schema 返回 `CoreError::InvalidCoreFrame`

## 非目标

- 不实现 CLI。
- 不实现 native-host bridge。
- 不实现 permission check。
- 不实现 request dispatch to app workers。
- 不新增 shared protocol crate。
- 不实现多连接 runtime concurrency。
