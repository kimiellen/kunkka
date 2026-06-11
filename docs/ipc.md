# Kunkka IPC

## Scope

Kunkka IPC is the local process communication protocol used between:

- core
- native host
- worker SDK / workers
- CLI
- TUI

Browser extensions do not connect directly to Unix Domain Socket. They enter through Native Messaging and `kunkka-native-host`.

## Transport

- Transport: Unix Domain Socket
- Framing: `tokio-util::codec::LengthDelimitedCodec`
- Serialization: `postcard`

## Current Crate

`crates/kunkka-ipc` owns:

- `Frame`
- `RequestId`
- `StreamId`
- `SessionId`
- `EndpointId`
- `Payload`
- frame encode/decode helpers
- UDS connection/listener wrapper
- IPC error type

## Frame Variants

Current frame variants:

- `Request`
- `Response`
- `Event`
- `Stream`
- `Cancel`
- `Heartbeat`
- `Error`

## Opaque Payload

`kunkka-ipc` uses opaque payload bytes:

```rust
pub struct Payload {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    pub schema: Option<String>,
    pub metadata: FrameMetadata,
}
```

Typed business payloads belong above IPC.

Examples:

- Worker registration payloads live in `kunkka-worker-sdk`.
- Future Native Messaging request envelopes should live outside `kunkka-ipc`.
- Future app request schemas should live outside `kunkka-ipc`.

## ID Rules

- `RequestId(u128)` correlates request and response.
- `StreamId(u128)` correlates stream frames.
- `SessionId(u128)` identifies one connection, frontend context, or worker session.

These are strong Rust newtypes to avoid accidental ID mixing.
