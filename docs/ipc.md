# Kunkka IPC

## 范围

Kunkka IPC 是本地进程通信协议，用于连接：

- core
- native host
- worker SDK / workers
- CLI
- TUI

Browser Extension 不直接连接 Unix Domain Socket。它们必须通过 Native Messaging 和 `kunkka-native-host` 进入本地系统。

## Transport

- Transport: Unix Domain Socket
- Framing: `tokio-util::codec::LengthDelimitedCodec`
- Serialization: `postcard`

## 当前 crate

`crates/kunkka-ipc` 拥有：

- `Frame`
- `RequestId`
- `StreamId`
- `SessionId`
- `EndpointId`
- `Payload`
- frame encode/decode helpers
- UDS connection/listener wrapper
- IPC error type

## Frame variants

当前 frame variants：

- `Request`
- `Response`
- `Event`
- `Stream`
- `Cancel`
- `Heartbeat`
- `Error`

## Opaque payload

`kunkka-ipc` 使用 opaque payload bytes：

```rust
pub struct Payload {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    pub schema: Option<String>,
    pub metadata: FrameMetadata,
}
```

Typed business payload 属于 IPC 之上的 crate。

Typed protocol ownership:

- IPC frame、transport、opaque payload 仍属于 `kunkka-ipc`。
- 跨 core/frontend 共享的 typed protocol 位于 `kunkka-protocol`。
- `kunkka.core-control.v1` 当前由 `kunkka-protocol::core_control` 定义。

当前 examples：

- Worker registration payload 位于 `kunkka-worker-sdk`。
- Core control payload 位于 `kunkka-protocol`。
- 未来 Native Messaging request envelope 应位于 `kunkka-ipc` 之外。
- 未来 app request schema 应位于 `kunkka-ipc` 之外。

## 当前 typed payload schemas

Worker registration：

```text
content_type = application/vnd.kunkka.worker.v1+postcard
schema = kunkka.worker.v1
```

Core control：

```text
content_type = application/vnd.kunkka.core-control.v1+postcard
schema = kunkka.core-control.v1
```

Core runtime 当前按 `Payload.schema` 分发：

- `kunkka.worker.v1` 进入 worker registration handler。
- `kunkka.core-control.v1` 进入 core control handler。
- 未知 schema 返回 invalid core frame error。

Core control v1 当前支持：

- `Ping` -> `Pong`
- `Status` -> `StatusResult`

## ID 规则

- `RequestId(u128)` 关联 request 和 response。
- `StreamId(u128)` 关联 stream frames。
- `SessionId(u128)` 标识一个 connection、frontend context 或 worker session。

这些 ID 是 strong Rust newtypes，用于避免不同 ID 类型被意外混用。
