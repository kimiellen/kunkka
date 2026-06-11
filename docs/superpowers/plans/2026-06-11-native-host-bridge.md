# Native Host Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a shared core-control protocol crate and implement the first `kunkka-native-host` bridge for Native Messaging `ping` and `status` commands.

**Architecture:** `kunkka-protocol` owns shared typed protocol and postcard payload codec. `kunkka-core` consumes that protocol and extends its single accepted connection loop to handle multiple frames. `kunkka-native-host` owns Native Messaging JSON, length-prefixed stdin/stdout I/O, socket path resolution, and a long-lived bridge session with a cached core IPC connection.

**Tech Stack:** Rust 2021, Tokio, serde, serde_json, postcard, Kunkka IPC over Unix Domain Socket, WebExtension Native Messaging length-prefixed JSON.

---

## File Structure

- Create: `crates/kunkka-protocol/Cargo.toml`
- Create: `crates/kunkka-protocol/src/lib.rs`
- Create: `crates/kunkka-protocol/src/error.rs`
- Create: `crates/kunkka-protocol/src/core_control.rs`
- Create: `crates/kunkka-protocol/tests/core_control.rs`
- Modify: `Cargo.toml`
- Modify: `crates/kunkka-core/Cargo.toml`
- Delete: `crates/kunkka-core/src/control.rs`
- Modify: `crates/kunkka-core/src/error.rs`
- Modify: `crates/kunkka-core/src/lib.rs`
- Modify: `crates/kunkka-core/src/runtime.rs`
- Delete: `crates/kunkka-core/tests/core_control_protocol.rs`
- Modify: `crates/kunkka-core/tests/core_runtime_control.rs`
- Modify: `crates/kunkka-native-host/Cargo.toml`
- Create: `crates/kunkka-native-host/src/lib.rs`
- Create: `crates/kunkka-native-host/src/error.rs`
- Create: `crates/kunkka-native-host/src/native_protocol.rs`
- Create: `crates/kunkka-native-host/src/native_messaging.rs`
- Create: `crates/kunkka-native-host/src/path.rs`
- Create: `crates/kunkka-native-host/src/bridge.rs`
- Create: `crates/kunkka-native-host/src/host.rs`
- Modify: `crates/kunkka-native-host/src/main.rs`
- Create: `crates/kunkka-native-host/tests/native_protocol.rs`
- Create: `crates/kunkka-native-host/tests/native_messaging.rs`
- Create: `crates/kunkka-native-host/tests/path.rs`
- Create: `crates/kunkka-native-host/tests/bridge_mapping.rs`
- Create: `crates/kunkka-native-host/tests/bridge_session.rs`
- Create: `crates/kunkka-native-host/tests/host_loop.rs`
- Modify: `README.md`
- Modify: `docs/architecture.md`
- Modify: `docs/ipc.md`
- Modify: `docs/browser-extension.md`
- Modify: `docs/development-log.md`

## Task 1: Add Shared Core-Control Protocol Crate

**Files:**

- Modify: `Cargo.toml`
- Create: `crates/kunkka-protocol/Cargo.toml`
- Create: `crates/kunkka-protocol/src/lib.rs`
- Create: `crates/kunkka-protocol/src/error.rs`
- Create: `crates/kunkka-protocol/src/core_control.rs`
- Create: `crates/kunkka-protocol/tests/core_control.rs`

- [ ] **Step 1: Create crate skeleton and failing protocol tests**

Modify workspace members in `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
  "crates/kunkka-ipc",
  "crates/kunkka-protocol",
  "crates/kunkka-core",
  "crates/kunkka-worker-sdk",
  "crates/kunkka-native-host",
]
```

Create `crates/kunkka-protocol/Cargo.toml`:

```toml
[package]
name = "kunkka-protocol"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[dependencies]
kunkka-ipc = { path = "../kunkka-ipc" }
postcard.workspace = true
serde.workspace = true
thiserror.workspace = true
```

Create empty `crates/kunkka-protocol/src/lib.rs`:

```rust
pub mod error;

pub use error::{ProtocolError, Result};
```

Create `crates/kunkka-protocol/src/error.rs`:

```rust
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ProtocolError>;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("codec error: {0}")]
    Codec(#[from] postcard::Error),
}
```

Create `crates/kunkka-protocol/tests/core_control.rs`:

```rust
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingRequest,
    CoreStatusResponse, CORE_CONTROL_CONTENT_TYPE, CORE_CONTROL_SCHEMA,
};

#[test]
fn ping_payload_roundtrips_with_control_metadata() {
    let message = CoreControlMessage::Ping(CorePingRequest);

    let payload = encode_control_message(&message).unwrap();

    assert_eq!(
        payload.content_type.as_deref(),
        Some(CORE_CONTROL_CONTENT_TYPE)
    );
    assert_eq!(payload.schema.as_deref(), Some(CORE_CONTROL_SCHEMA));

    let decoded = decode_control_message(&payload).unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn status_result_payload_roundtrips_with_runtime_state() {
    let message = CoreControlMessage::StatusResult(CoreStatusResponse {
        worker_count: 2,
        socket_path: "/run/user/1000/kunkka/core.sock".to_string(),
        runtime_ready: true,
    });

    let payload = encode_control_message(&message).unwrap();
    let decoded = decode_control_message(&payload).unwrap();

    assert_eq!(decoded, message);
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p kunkka-protocol --test core_control`

Expected: FAIL with unresolved import `kunkka_protocol::core_control`.

- [ ] **Step 3: Implement core-control protocol module**

Update `crates/kunkka-protocol/src/lib.rs`:

```rust
pub mod core_control;
pub mod error;

pub use error::{ProtocolError, Result};
```

Create `crates/kunkka-protocol/src/core_control.rs`:

```rust
use crate::Result;
use kunkka_ipc::{FrameMetadata, Payload};
use serde::{Deserialize, Serialize};

pub const CORE_CONTROL_CONTENT_TYPE: &str = "application/vnd.kunkka.core-control.v1+postcard";
pub const CORE_CONTROL_SCHEMA: &str = "kunkka.core-control.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorePingRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorePingResponse;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreStatusRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreStatusResponse {
    pub worker_count: u64,
    pub socket_path: String,
    pub runtime_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreControlMessage {
    Ping(CorePingRequest),
    Pong(CorePingResponse),
    Status(CoreStatusRequest),
    StatusResult(CoreStatusResponse),
}

pub fn encode_control_message(message: &CoreControlMessage) -> Result<Payload> {
    let bytes = postcard::to_stdvec(message)?;

    Ok(Payload {
        bytes,
        content_type: Some(CORE_CONTROL_CONTENT_TYPE.to_string()),
        schema: Some(CORE_CONTROL_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_control_message(payload: &Payload) -> Result<CoreControlMessage> {
    Ok(postcard::from_bytes(&payload.bytes)?)
}
```

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p kunkka-protocol --test core_control`

Expected: PASS with 2 tests.

- [ ] **Step 5: Commit**

Run:

```bash
git add Cargo.toml crates/kunkka-protocol
git commit -m "feat: add shared protocol crate"
```

## Task 2: Migrate Core to Shared Core-Control Protocol

**Files:**

- Modify: `crates/kunkka-core/Cargo.toml`
- Delete: `crates/kunkka-core/src/control.rs`
- Modify: `crates/kunkka-core/src/error.rs`
- Modify: `crates/kunkka-core/src/lib.rs`
- Modify: `crates/kunkka-core/src/runtime.rs`
- Delete: `crates/kunkka-core/tests/core_control_protocol.rs`
- Modify: `crates/kunkka-core/tests/core_runtime_control.rs`

- [ ] **Step 1: Write failing migration test change**

In `crates/kunkka-core/tests/core_runtime_control.rs`, replace the first import block with:

```rust
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_core::CoreError;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingRequest,
    CorePingResponse, CoreStatusRequest,
};
use kunkka_worker_sdk::{AppId, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId};
use std::path::PathBuf;
use tempfile::{tempdir, TempDir};
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p kunkka-core --test core_runtime_control`

Expected: FAIL with unresolved crate or module `kunkka_protocol` because `kunkka-core` does not depend on it yet.

- [ ] **Step 3: Implement core migration**

Update `crates/kunkka-core/Cargo.toml` dependencies:

```toml
[dependencies]
kunkka-ipc = { path = "../kunkka-ipc" }
kunkka-protocol = { path = "../kunkka-protocol" }
kunkka-worker-sdk = { path = "../kunkka-worker-sdk" }
libc.workspace = true
thiserror.workspace = true
tokio.workspace = true
```

Update `crates/kunkka-core/src/error.rs`:

```rust
use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("HOME is missing or is not an absolute path")]
    MissingHome,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ipc error: {0}")]
    Ipc(#[from] kunkka_ipc::IpcError),

    #[error("protocol error: {0}")]
    Protocol(#[from] kunkka_protocol::ProtocolError),

    #[error("worker sdk error: {0}")]
    WorkerSdk(#[from] kunkka_worker_sdk::WorkerSdkError),

    #[error("invalid worker frame: {0}")]
    InvalidWorkerFrame(String),

    #[error("invalid core frame: {0}")]
    InvalidCoreFrame(String),
}
```

Update `crates/kunkka-core/src/lib.rs` to remove `pub mod control;`:

```rust
pub mod error;
pub mod ipc_server;
pub mod runtime;
pub mod worker_registry;
pub mod xdg;

pub use error::{CoreError, Result};
pub use kunkka_ipc as ipc;

use ipc_server::CoreIpcServer;
use runtime::CoreRuntime;
use xdg::KunkkaPaths;

pub async fn prepare_core_server(paths: &KunkkaPaths) -> Result<CoreIpcServer> {
    paths.ensure_dirs()?;
    CoreIpcServer::bind(paths).await
}

pub async fn prepare_core_runtime(paths: &KunkkaPaths) -> Result<CoreRuntime> {
    CoreRuntime::prepare(paths).await
}
```

Update the import section in `crates/kunkka-core/src/runtime.rs`:

```rust
use crate::ipc_server::CoreIpcServer;
use crate::worker_registry::{handle_worker_registration_frame, WorkerRegistry};
use crate::xdg::KunkkaPaths;
use crate::{CoreError, Result};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingResponse,
    CoreStatusResponse, CORE_CONTROL_SCHEMA,
};
use kunkka_worker_sdk::WORKER_PROTOCOL_SCHEMA;
```

Delete `crates/kunkka-core/src/control.rs`.

Delete `crates/kunkka-core/tests/core_control_protocol.rs` because codec ownership moved to `kunkka-protocol`.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p kunkka-core --test core_runtime_control`

Expected: PASS with existing runtime control tests.

Run: `cargo test -p kunkka-protocol --test core_control`

Expected: PASS with 2 tests.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/kunkka-core Cargo.toml Cargo.lock crates/kunkka-protocol
git add -u crates/kunkka-core
git commit -m "refactor: use shared core control protocol"
```

## Task 3: Support Multiple Frames on One Core Connection

**Files:**

- Modify: `crates/kunkka-core/src/runtime.rs`
- Modify: `crates/kunkka-core/tests/core_runtime_control.rs`

- [ ] **Step 1: Write failing reuse test**

Append this test to `crates/kunkka-core/tests/core_runtime_control.rs`:

```rust
#[tokio::test]
async fn one_connection_can_handle_multiple_control_requests() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();

            let ping_payload =
                encode_control_message(&CoreControlMessage::Ping(CorePingRequest)).unwrap();
            let ping_frame = Frame::Request {
                request_id: RequestId(101),
                session_id: SessionId(202),
                source: EndpointId::new("native-host"),
                target: EndpointId::new("core"),
                payload: ping_payload,
                metadata: FrameMetadata::new(),
            };
            connection.send_frame(&ping_frame).await.unwrap();
            let ping_response = connection.recv_frame().await.unwrap().unwrap();

            let status_payload =
                encode_control_message(&CoreControlMessage::Status(CoreStatusRequest)).unwrap();
            let status_frame = Frame::Request {
                request_id: RequestId(102),
                session_id: SessionId(202),
                source: EndpointId::new("native-host"),
                target: EndpointId::new("core"),
                payload: status_payload,
                metadata: FrameMetadata::new(),
            };
            connection.send_frame(&status_frame).await.unwrap();
            let status_response = connection.recv_frame().await.unwrap().unwrap();

            (ping_response, status_response)
        }
    });

    runtime.run_once().await.unwrap();

    let (ping_response, status_response) = client_task.await.unwrap();

    let Frame::Response {
        request_id: ping_request_id,
        payload: ping_payload,
        ..
    } = ping_response
    else {
        panic!("expected ping response frame");
    };
    assert_eq!(ping_request_id, RequestId(101));
    assert_eq!(
        decode_control_message(&ping_payload).unwrap(),
        CoreControlMessage::Pong(CorePingResponse)
    );

    let Frame::Response {
        request_id: status_request_id,
        payload: status_payload,
        ..
    } = status_response
    else {
        panic!("expected status response frame");
    };
    assert_eq!(status_request_id, RequestId(102));
    assert!(matches!(
        decode_control_message(&status_payload).unwrap(),
        CoreControlMessage::StatusResult(_)
    ));
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p kunkka-core --test core_runtime_control one_connection_can_handle_multiple_control_requests`

Expected: FAIL because `CoreRuntime::run_once()` closes the server side after the first frame.

- [ ] **Step 3: Implement connection frame loop**

Update `CoreRuntime::run_once()` in `crates/kunkka-core/src/runtime.rs`:

```rust
pub async fn run_once(&mut self) -> Result<()> {
    let mut connection = self.server.accept_one().await?;

    while let Some(frame) = connection.recv_frame().await? {
        let response = self.handle_frame(frame)?;
        connection.send_frame(&response).await?;
    }

    Ok(())
}
```

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p kunkka-core --test core_runtime_control one_connection_can_handle_multiple_control_requests`

Expected: PASS.

Run: `cargo test -p kunkka-core --test core_runtime_loop`

Expected: PASS, confirming worker registration still works when clients close after one request.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/kunkka-core/src/runtime.rs crates/kunkka-core/tests/core_runtime_control.rs
git commit -m "feat: support multiple frames per core connection"
```

## Task 4: Add Native Messaging JSON and Length Codec

**Files:**

- Modify: `Cargo.toml`
- Modify: `crates/kunkka-native-host/Cargo.toml`
- Create: `crates/kunkka-native-host/src/lib.rs`
- Create: `crates/kunkka-native-host/src/error.rs`
- Create: `crates/kunkka-native-host/src/native_protocol.rs`
- Create: `crates/kunkka-native-host/src/native_messaging.rs`
- Create: `crates/kunkka-native-host/tests/native_protocol.rs`
- Create: `crates/kunkka-native-host/tests/native_messaging.rs`

- [ ] **Step 1: Write failing JSON and Native Messaging tests**

Add `serde_json` to workspace dependencies in `Cargo.toml`:

```toml
serde_json = "1"
```

Create `crates/kunkka-native-host/tests/native_protocol.rs`:

```rust
use kunkka_native_host::native_protocol::{
    decode_request, error_response, success_response, NativeCommand, NativeErrorCode,
    NativeRequest, NativeResult,
};

#[test]
fn decodes_ping_request() {
    let request = decode_request(br#"{"id":"req-1","command":"ping"}"#).unwrap();

    assert_eq!(
        request,
        NativeRequest {
            id: "req-1".to_string(),
            command: NativeCommand::Ping,
        }
    );
}

#[test]
fn serializes_status_success_response() {
    let response = success_response(
        "req-2",
        NativeResult::Status {
            worker_count: 1,
            socket_path: "/run/user/1000/kunkka/core.sock".to_string(),
            runtime_ready: true,
        },
    );

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": "req-2",
            "ok": true,
            "result": {
                "type": "status",
                "worker_count": 1,
                "socket_path": "/run/user/1000/kunkka/core.sock",
                "runtime_ready": true
            }
        })
    );
}

#[test]
fn serializes_invalid_request_without_id_as_null_id() {
    let response = error_response(
        None,
        NativeErrorCode::InvalidRequest,
        "missing request id",
    );

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": null,
            "ok": false,
            "error": {
                "code": "invalid_request",
                "message": "missing request id"
            }
        })
    );
}
```

Create `crates/kunkka-native-host/tests/native_messaging.rs`:

```rust
use kunkka_native_host::native_messaging::{
    read_native_message, write_native_message, MAX_NATIVE_MESSAGE_LEN,
};
use std::io::Cursor;

#[test]
fn reads_length_prefixed_json_body() {
    let body = br#"{"id":"req-1","command":"ping"}"#;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(body);

    let mut reader = Cursor::new(bytes);
    let decoded = read_native_message(&mut reader).unwrap().unwrap();

    assert_eq!(decoded, body);
}

#[test]
fn writes_length_prefixed_json_body() {
    let mut output = Vec::new();
    let value = serde_json::json!({"id":"req-1","ok":true,"result":{"type":"pong"}});

    write_native_message(&mut output, &value).unwrap();

    let body_len = u32::from_le_bytes(output[0..4].try_into().unwrap()) as usize;
    let body = &output[4..];

    assert_eq!(body_len, body.len());
    assert_eq!(serde_json::from_slice::<serde_json::Value>(body).unwrap(), value);
}

#[test]
fn rejects_oversized_native_message() {
    let oversized = (MAX_NATIVE_MESSAGE_LEN as u32) + 1;
    let mut reader = Cursor::new(oversized.to_le_bytes().to_vec());

    let err = read_native_message(&mut reader).unwrap_err();

    assert!(err.to_string().contains("native message too large"));
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p kunkka-native-host --test native_protocol`

Expected: FAIL with unresolved crate module exports because `kunkka-native-host` has no library modules yet.

Run: `cargo test -p kunkka-native-host --test native_messaging`

Expected: FAIL with unresolved crate module exports because `kunkka-native-host` has no library modules yet.

- [ ] **Step 3: Implement JSON protocol and length codec**

Update `crates/kunkka-native-host/Cargo.toml`:

```toml
[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
```

Create `crates/kunkka-native-host/src/lib.rs`:

```rust
pub mod error;
pub mod native_messaging;
pub mod native_protocol;

pub use error::{NativeHostError, Result};
```

Create `crates/kunkka-native-host/src/error.rs`:

```rust
use crate::native_protocol::NativeErrorCode;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, NativeHostError>;

#[derive(Debug, Error)]
pub enum NativeHostError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("core unavailable: {0}")]
    CoreUnavailable(String),

    #[error("core ipc error: {0}")]
    CoreIpc(String),

    #[error("unexpected core response: {0}")]
    UnexpectedCoreResponse(String),
}

impl NativeHostError {
    pub fn code(&self) -> NativeErrorCode {
        match self {
            Self::InvalidRequest(_) | Self::Json(_) | Self::Io(_) => NativeErrorCode::InvalidRequest,
            Self::CoreUnavailable(_) => NativeErrorCode::CoreUnavailable,
            Self::CoreIpc(_) => NativeErrorCode::CoreIpcError,
            Self::UnexpectedCoreResponse(_) => NativeErrorCode::UnexpectedCoreResponse,
        }
    }
}
```

Create `crates/kunkka-native-host/src/native_protocol.rs`:

```rust
use crate::{NativeHostError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NativeRequest {
    pub id: String,
    pub command: NativeCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCommand {
    Ping,
    Status,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeResponse {
    pub id: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<NativeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<NativeErrorBody>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NativeResult {
    Pong,
    Status {
        worker_count: u64,
        socket_path: String,
        runtime_ready: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeErrorBody {
    pub code: NativeErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeErrorCode {
    InvalidRequest,
    CoreUnavailable,
    CoreIpcError,
    UnexpectedCoreResponse,
}

impl std::fmt::Display for NativeErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::InvalidRequest => "invalid_request",
            Self::CoreUnavailable => "core_unavailable",
            Self::CoreIpcError => "core_ipc_error",
            Self::UnexpectedCoreResponse => "unexpected_core_response",
        };

        formatter.write_str(value)
    }
}

pub fn decode_request(bytes: &[u8]) -> Result<NativeRequest> {
    let request: NativeRequest = serde_json::from_slice(bytes)?;

    if request.id.is_empty() {
        return Err(NativeHostError::InvalidRequest(
            "missing request id".to_string(),
        ));
    }

    Ok(request)
}

pub fn extract_request_id(bytes: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    value
        .get("id")
        .and_then(|id| id.as_str())
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
}

pub fn success_response(id: impl Into<String>, result: NativeResult) -> NativeResponse {
    NativeResponse {
        id: Some(id.into()),
        ok: true,
        result: Some(result),
        error: None,
    }
}

pub fn error_response(
    id: Option<String>,
    code: NativeErrorCode,
    message: impl Into<String>,
) -> NativeResponse {
    NativeResponse {
        id,
        ok: false,
        result: None,
        error: Some(NativeErrorBody {
            code,
            message: message.into(),
        }),
    }
}
```

Create `crates/kunkka-native-host/src/native_messaging.rs`:

```rust
use crate::{NativeHostError, Result};
use serde::Serialize;
use std::io::{ErrorKind, Read, Write};

pub const MAX_NATIVE_MESSAGE_LEN: usize = 1024 * 1024;

pub fn read_native_message<R: Read>(reader: &mut R) -> Result<Option<Vec<u8>>> {
    let mut len_bytes = [0_u8; 4];
    let mut read_len = 0;

    while read_len < len_bytes.len() {
        let bytes_read = reader.read(&mut len_bytes[read_len..])?;
        if bytes_read == 0 {
            if read_len == 0 {
                return Ok(None);
            }

            return Err(NativeHostError::InvalidRequest(
                "native message length prefix ended early".to_string(),
            ));
        }

        read_len += bytes_read;
    }

    let len = u32::from_le_bytes(len_bytes) as usize;
    if len > MAX_NATIVE_MESSAGE_LEN {
        return Err(NativeHostError::InvalidRequest(format!(
            "native message too large: {len} bytes"
        )));
    }

    let mut body = vec![0_u8; len];
    reader.read_exact(&mut body).map_err(|err| {
        if err.kind() == ErrorKind::UnexpectedEof {
            NativeHostError::InvalidRequest(
                "native message body ended before declared length".to_string(),
            )
        } else {
            NativeHostError::Io(err)
        }
    })?;
    Ok(Some(body))
}

pub fn write_native_message<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    if body.len() > MAX_NATIVE_MESSAGE_LEN {
        return Err(NativeHostError::InvalidRequest(format!(
            "native message too large: {} bytes",
            body.len()
        )));
    }

    writer.write_all(&(body.len() as u32).to_le_bytes())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}
```

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p kunkka-native-host --test native_protocol`

Expected: PASS with JSON protocol tests.

Run: `cargo test -p kunkka-native-host --test native_messaging`

Expected: PASS with length codec tests.

- [ ] **Step 5: Commit**

Run:

```bash
git add Cargo.toml Cargo.lock crates/kunkka-native-host
git commit -m "feat: add native messaging protocol"
```

## Task 5: Add Native Host Core Socket Path Resolution

**Files:**

- Modify: `crates/kunkka-native-host/src/lib.rs`
- Modify: `crates/kunkka-native-host/Cargo.toml`
- Create: `crates/kunkka-native-host/src/path.rs`
- Create: `crates/kunkka-native-host/tests/path.rs`

- [ ] **Step 1: Write failing path tests**

Create `crates/kunkka-native-host/tests/path.rs`:

```rust
use kunkka_native_host::path::{resolve_core_socket_path_from_env, CoreSocketPathEnv};
use std::path::PathBuf;

#[test]
fn resolves_socket_under_absolute_xdg_runtime_dir() {
    let env = CoreSocketPathEnv {
        xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
    };

    let path = resolve_core_socket_path_from_env(&env);

    assert_eq!(path, PathBuf::from("/run/user/1000/kunkka/core.sock"));
}

#[test]
fn ignores_relative_xdg_runtime_dir_and_uses_tmp_fallback() {
    let env = CoreSocketPathEnv {
        xdg_runtime_dir: Some(PathBuf::from("relative-runtime")),
    };
    let uid = unsafe { libc::geteuid() as u32 };

    let path = resolve_core_socket_path_from_env(&env);

    assert_eq!(
        path,
        PathBuf::from(format!("/tmp/kunkka-runtime-{uid}/core.sock"))
    );
}

#[test]
fn uses_tmp_fallback_when_xdg_runtime_dir_is_missing() {
    let env = CoreSocketPathEnv {
        xdg_runtime_dir: None,
    };
    let uid = unsafe { libc::geteuid() as u32 };

    let path = resolve_core_socket_path_from_env(&env);

    assert_eq!(
        path,
        PathBuf::from(format!("/tmp/kunkka-runtime-{uid}/core.sock"))
    );
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p kunkka-native-host --test path`

Expected: FAIL with unresolved module `kunkka_native_host::path`.

- [ ] **Step 3: Implement path resolution**

Update `crates/kunkka-native-host/Cargo.toml` dependencies:

```toml
libc.workspace = true
```

Update `crates/kunkka-native-host/src/lib.rs`:

```rust
pub mod error;
pub mod native_messaging;
pub mod native_protocol;
pub mod path;

pub use error::{NativeHostError, Result};
```

Create `crates/kunkka-native-host/src/path.rs`:

```rust
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoreSocketPathEnv {
    pub xdg_runtime_dir: Option<PathBuf>,
}

impl CoreSocketPathEnv {
    pub fn from_process() -> Self {
        Self {
            xdg_runtime_dir: env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
        }
    }
}

pub fn resolve_core_socket_path() -> PathBuf {
    resolve_core_socket_path_from_env(&CoreSocketPathEnv::from_process())
}

pub fn resolve_core_socket_path_from_env(env: &CoreSocketPathEnv) -> PathBuf {
    let runtime_dir = env
        .xdg_runtime_dir
        .as_ref()
        .filter(|path| path.is_absolute())
        .map(|path| path.join("kunkka"))
        .unwrap_or_else(runtime_fallback_dir);

    runtime_dir.join("core.sock")
}

fn runtime_fallback_dir() -> PathBuf {
    PathBuf::from(format!("/tmp/kunkka-runtime-{}", effective_uid()))
}

fn effective_uid() -> u32 {
    unsafe {
        // SAFETY: geteuid has no preconditions and does not dereference pointers.
        libc::geteuid() as u32
    }
}
```

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p kunkka-native-host --test path`

Expected: PASS with 3 tests.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/kunkka-native-host/Cargo.toml crates/kunkka-native-host/src/lib.rs crates/kunkka-native-host/src/path.rs crates/kunkka-native-host/tests/path.rs
git commit -m "feat: resolve native host core socket"
```

## Task 6: Map Native Commands to Core-Control Messages

**Files:**

- Modify: `crates/kunkka-native-host/src/lib.rs`
- Modify: `crates/kunkka-native-host/Cargo.toml`
- Create: `crates/kunkka-native-host/src/bridge.rs`
- Create: `crates/kunkka-native-host/tests/bridge_mapping.rs`

- [ ] **Step 1: Write failing mapping tests**

Create `crates/kunkka-native-host/tests/bridge_mapping.rs`:

```rust
use kunkka_native_host::bridge::{core_message_for_command, native_result_for_core_response};
use kunkka_native_host::native_protocol::{NativeCommand, NativeResult};
use kunkka_native_host::NativeHostError;
use kunkka_protocol::core_control::{
    CoreControlMessage, CorePingRequest, CorePingResponse, CoreStatusResponse,
};

#[test]
fn maps_ping_command_to_core_ping() {
    let message = core_message_for_command(&NativeCommand::Ping);

    assert_eq!(message, CoreControlMessage::Ping(CorePingRequest));
}

#[test]
fn maps_status_result_to_native_status_result() {
    let result = native_result_for_core_response(
        &NativeCommand::Status,
        CoreControlMessage::StatusResult(CoreStatusResponse {
            worker_count: 3,
            socket_path: "/run/user/1000/kunkka/core.sock".to_string(),
            runtime_ready: true,
        }),
    )
    .unwrap();

    assert_eq!(
        result,
        NativeResult::Status {
            worker_count: 3,
            socket_path: "/run/user/1000/kunkka/core.sock".to_string(),
            runtime_ready: true,
        }
    );
}

#[test]
fn rejects_unexpected_core_response_for_ping() {
    let err = native_result_for_core_response(
        &NativeCommand::Ping,
        CoreControlMessage::StatusResult(CoreStatusResponse {
            worker_count: 0,
            socket_path: "/run/user/1000/kunkka/core.sock".to_string(),
            runtime_ready: true,
        }),
    )
    .unwrap_err();

    assert!(matches!(err, NativeHostError::UnexpectedCoreResponse(_)));
}

#[test]
fn maps_pong_to_native_pong() {
    let result = native_result_for_core_response(
        &NativeCommand::Ping,
        CoreControlMessage::Pong(CorePingResponse),
    )
    .unwrap();

    assert_eq!(result, NativeResult::Pong);
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p kunkka-native-host --test bridge_mapping`

Expected: FAIL with unresolved module `kunkka_native_host::bridge`.

- [ ] **Step 3: Implement mapping functions**

Update `crates/kunkka-native-host/Cargo.toml` dependencies:

```toml
kunkka-protocol = { path = "../kunkka-protocol" }
```

Update `crates/kunkka-native-host/src/lib.rs`:

```rust
pub mod bridge;
pub mod error;
pub mod native_messaging;
pub mod native_protocol;
pub mod path;

pub use error::{NativeHostError, Result};
```

Create `crates/kunkka-native-host/src/bridge.rs`:

```rust
use crate::native_protocol::{NativeCommand, NativeResult};
use crate::{NativeHostError, Result};
use kunkka_protocol::core_control::{
    CoreControlMessage, CorePingRequest, CorePingResponse, CoreStatusRequest,
};

pub fn core_message_for_command(command: &NativeCommand) -> CoreControlMessage {
    match command {
        NativeCommand::Ping => CoreControlMessage::Ping(CorePingRequest),
        NativeCommand::Status => CoreControlMessage::Status(CoreStatusRequest),
    }
}

pub fn native_result_for_core_response(
    command: &NativeCommand,
    message: CoreControlMessage,
) -> Result<NativeResult> {
    match (command, message) {
        (NativeCommand::Ping, CoreControlMessage::Pong(CorePingResponse)) => Ok(NativeResult::Pong),
        (NativeCommand::Status, CoreControlMessage::StatusResult(status)) => {
            Ok(NativeResult::Status {
                worker_count: status.worker_count,
                socket_path: status.socket_path,
                runtime_ready: status.runtime_ready,
            })
        }
        (command, message) => Err(NativeHostError::UnexpectedCoreResponse(format!(
            "unexpected core response for {command:?}: {message:?}"
        ))),
    }
}
```

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p kunkka-native-host --test bridge_mapping`

Expected: PASS with 4 tests.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/kunkka-native-host/Cargo.toml crates/kunkka-native-host/src/lib.rs crates/kunkka-native-host/src/bridge.rs crates/kunkka-native-host/tests/bridge_mapping.rs
git commit -m "feat: map native commands to core control"
```

## Task 7: Add Native Host Bridge Session with Cached Core Connection

**Files:**

- Modify: `crates/kunkka-native-host/src/bridge.rs`
- Modify: `crates/kunkka-native-host/Cargo.toml`
- Create: `crates/kunkka-native-host/tests/bridge_session.rs`

- [ ] **Step 1: Write failing session tests**

Create `crates/kunkka-native-host/tests/bridge_session.rs`:

```rust
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_ipc::IpcListener;
use kunkka_native_host::bridge::NativeHostSession;
use kunkka_native_host::native_protocol::{NativeCommand, NativeRequest, NativeResult};
use kunkka_worker_sdk::{AppId, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId};
use tempfile::{tempdir, TempDir};

fn test_paths() -> (TempDir, KunkkaPaths) {
    let root = tempdir().unwrap();

    let paths = KunkkaPaths {
        config_dir: root.path().join("config"),
        data_dir: root.path().join("data"),
        state_dir: root.path().join("state"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
        database_path: root.path().join("data/kunkka.db"),
        log_dir: root.path().join("state/logs"),
        socket_path: root.path().join("runtime/core.sock"),
    };

    (root, paths)
}

fn worker_request() -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new("worker-1"),
        app_id: AppId::new("example-app"),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: Some("Search notes".to_string()),
        }],
    }
}

#[tokio::test]
async fn session_ping_returns_pong() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let mut session = NativeHostSession::new(paths.socket_path.clone());
    let response = session
        .handle_request(NativeRequest {
            id: "req-1".to_string(),
            command: NativeCommand::Ping,
        })
        .await;

    assert!(response.ok);
    assert_eq!(response.id.as_deref(), Some("req-1"));
    assert_eq!(response.result, Some(NativeResult::Pong));

    drop(session);
    runtime_task.await.unwrap();
}

#[tokio::test]
async fn session_reuses_connection_for_ping_then_status() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let mut session = NativeHostSession::new(paths.socket_path.clone());

    let ping = session
        .handle_request(NativeRequest {
            id: "req-1".to_string(),
            command: NativeCommand::Ping,
        })
        .await;
    assert_eq!(ping.result, Some(NativeResult::Pong));

    let status = session
        .handle_request(NativeRequest {
            id: "req-2".to_string(),
            command: NativeCommand::Status,
        })
        .await;

    let Some(NativeResult::Status {
        worker_count,
        socket_path,
        runtime_ready,
    }) = status.result
    else {
        panic!("expected status result");
    };

    assert_eq!(worker_count, 0);
    assert_eq!(socket_path, paths.socket_path.to_string_lossy().into_owned());
    assert!(runtime_ready);

    drop(session);
    runtime_task.await.unwrap();
}

#[tokio::test]
async fn session_status_reports_registered_worker() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let register_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("worker-1"))
                .await
                .unwrap();
            client.register(worker_request()).await.unwrap()
        }
    });

    runtime.run_once().await.unwrap();
    assert!(register_task.await.unwrap().accepted);

    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });
    let mut session = NativeHostSession::new(paths.socket_path.clone());
    let response = session
        .handle_request(NativeRequest {
            id: "req-status".to_string(),
            command: NativeCommand::Status,
        })
        .await;

    let Some(NativeResult::Status { worker_count, .. }) = response.result else {
        panic!("expected status result");
    };
    assert_eq!(worker_count, 1);

    drop(session);
    runtime_task.await.unwrap();
}

#[tokio::test]
async fn core_unavailable_returns_error_response() {
    let (_root, paths) = test_paths();
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    let response = session
        .handle_request(NativeRequest {
            id: "req-1".to_string(),
            command: NativeCommand::Ping,
        })
        .await;

    assert!(!response.ok);
    assert_eq!(response.id.as_deref(), Some("req-1"));
    assert_eq!(response.error.unwrap().code.to_string(), "core_unavailable");
}

#[tokio::test]
async fn ipc_failure_clears_connection_and_next_request_reconnects() {
    let (_root, paths) = test_paths();
    paths.ensure_dirs().unwrap();

    let listener = IpcListener::bind(&paths.socket_path).await.unwrap();
    let failing_server = tokio::spawn(async move {
        let _connection = listener.accept().await.unwrap();
    });

    let mut session = NativeHostSession::new(paths.socket_path.clone());
    let failed = session
        .handle_request(NativeRequest {
            id: "req-fail".to_string(),
            command: NativeCommand::Ping,
        })
        .await;

    assert!(!failed.ok);
    assert_eq!(failed.error.unwrap().code.to_string(), "core_ipc_error");
    failing_server.await.unwrap();

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let recovered = session
        .handle_request(NativeRequest {
            id: "req-ok".to_string(),
            command: NativeCommand::Ping,
        })
        .await;

    assert!(recovered.ok);
    assert_eq!(recovered.result, Some(NativeResult::Pong));

    drop(session);
    runtime_task.await.unwrap();
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p kunkka-native-host --test bridge_session`

Expected: FAIL with unresolved `NativeHostSession`.

- [ ] **Step 3: Implement cached bridge session**

Update `crates/kunkka-native-host/Cargo.toml` dependencies and dev-dependencies:

```toml
[dependencies]
kunkka-ipc = { path = "../kunkka-ipc" }
tokio.workspace = true

[dev-dependencies]
kunkka-core = { path = "../kunkka-core" }
kunkka-worker-sdk = { path = "../kunkka-worker-sdk" }
tempfile.workspace = true
```

Append this implementation to `crates/kunkka-native-host/src/bridge.rs` after the mapping functions:

```rust
use crate::native_protocol::{error_response, success_response, NativeRequest, NativeResponse};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, RequestId, SessionId};
use kunkka_protocol::core_control::{decode_control_message, encode_control_message};
use std::path::{Path, PathBuf};

pub struct NativeHostSession {
    socket_path: PathBuf,
    connection: Option<IpcConnection>,
    source: EndpointId,
    target: EndpointId,
    session_id: SessionId,
    next_request_id: u128,
}

impl NativeHostSession {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
            connection: None,
            source: EndpointId::new("native-host"),
            target: EndpointId::new("core"),
            session_id: SessionId(1),
            next_request_id: 1,
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub async fn handle_request(&mut self, request: NativeRequest) -> NativeResponse {
        let id = request.id.clone();
        let command = request.command.clone();
        let core_message = core_message_for_command(&command);

        match self.send_core_control(core_message).await {
            Ok(response) => match native_result_for_core_response(&command, response) {
                Ok(result) => success_response(id, result),
                Err(err) => error_response(Some(id), err.code(), err.to_string()),
            },
            Err(err) => error_response(Some(id), err.code(), err.to_string()),
        }
    }

    async fn send_core_control(
        &mut self,
        message: CoreControlMessage,
    ) -> Result<CoreControlMessage> {
        self.ensure_connection().await?;
        let result = self.send_core_control_on_cached_connection(message).await;

        if matches!(
            result,
            Err(NativeHostError::CoreIpc(_)) | Err(NativeHostError::CoreUnavailable(_))
        ) {
            self.connection = None;
        }

        result
    }

    async fn ensure_connection(&mut self) -> Result<()> {
        if self.connection.is_some() {
            return Ok(());
        }

        let connection = IpcConnection::connect(&self.socket_path)
            .await
            .map_err(|err| NativeHostError::CoreUnavailable(err.to_string()))?;
        self.connection = Some(connection);
        Ok(())
    }

    async fn send_core_control_on_cached_connection(
        &mut self,
        message: CoreControlMessage,
    ) -> Result<CoreControlMessage> {
        let request_id = self.next_request_id();
        let payload = encode_control_message(&message)
            .map_err(|err| NativeHostError::CoreIpc(err.to_string()))?;
        let frame = Frame::Request {
            request_id,
            session_id: self.session_id,
            source: self.source.clone(),
            target: self.target.clone(),
            payload,
            metadata: FrameMetadata::new(),
        };

        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| NativeHostError::CoreUnavailable("core connection missing".to_string()))?;

        connection
            .send_frame(&frame)
            .await
            .map_err(|err| NativeHostError::CoreIpc(err.to_string()))?;
        let response = connection
            .recv_frame()
            .await
            .map_err(|err| NativeHostError::CoreIpc(err.to_string()))?
            .ok_or_else(|| NativeHostError::CoreIpc("core closed connection".to_string()))?;

        let Frame::Response {
            request_id: response_request_id,
            payload,
            ..
        } = response
        else {
            return Err(NativeHostError::UnexpectedCoreResponse(
                "expected response frame".to_string(),
            ));
        };

        if response_request_id != request_id {
            return Err(NativeHostError::UnexpectedCoreResponse(format!(
                "response request_id mismatch: expected {}, got {}",
                request_id.0, response_request_id.0
            )));
        }

        decode_control_message(&payload).map_err(|err| NativeHostError::CoreIpc(err.to_string()))
    }

    fn next_request_id(&mut self) -> RequestId {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        request_id
    }
}
```

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p kunkka-native-host --test bridge_session`

Expected: PASS with session integration tests.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/kunkka-native-host/Cargo.toml crates/kunkka-native-host/src/bridge.rs crates/kunkka-native-host/src/native_protocol.rs crates/kunkka-native-host/tests/bridge_session.rs
git commit -m "feat: bridge native host session to core"
```

## Task 8: Add Native Host Loop and Binary Entrypoint

**Files:**

- Modify: `crates/kunkka-native-host/src/lib.rs`
- Create: `crates/kunkka-native-host/src/host.rs`
- Modify: `crates/kunkka-native-host/src/main.rs`
- Create: `crates/kunkka-native-host/tests/host_loop.rs`

- [ ] **Step 1: Write failing host loop test**

Create `crates/kunkka-native-host/tests/host_loop.rs`:

```rust
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_native_host::bridge::NativeHostSession;
use kunkka_native_host::host::run_native_host;
use kunkka_native_host::native_messaging::{
    read_native_message, write_native_message, MAX_NATIVE_MESSAGE_LEN,
};
use serde_json::json;
use std::io::Cursor;
use tempfile::{tempdir, TempDir};

fn test_paths() -> (TempDir, KunkkaPaths) {
    let root = tempdir().unwrap();

    let paths = KunkkaPaths {
        config_dir: root.path().join("config"),
        data_dir: root.path().join("data"),
        state_dir: root.path().join("state"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
        database_path: root.path().join("data/kunkka.db"),
        log_dir: root.path().join("state/logs"),
        socket_path: root.path().join("runtime/core.sock"),
    };

    (root, paths)
}

#[tokio::test]
async fn host_loop_reads_native_message_and_writes_response() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let mut input_bytes = Vec::new();
    write_native_message(
        &mut input_bytes,
        &json!({"id":"req-1","command":"ping"}),
    )
    .unwrap();

    let mut input = Cursor::new(input_bytes);
    let mut output = Vec::new();
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    run_native_host(&mut input, &mut output, &mut session)
        .await
        .unwrap();

    drop(session);
    runtime_task.await.unwrap();

    let mut output_reader = Cursor::new(output);
    let response_bytes = read_native_message(&mut output_reader).unwrap().unwrap();
    let response: serde_json::Value = serde_json::from_slice(&response_bytes).unwrap();

    assert_eq!(
        response,
        json!({"id":"req-1","ok":true,"result":{"type":"pong"}})
    );
}

#[tokio::test]
async fn host_loop_returns_invalid_request_with_null_id_for_missing_id() {
    let (_root, paths) = test_paths();
    let mut input_bytes = Vec::new();
    write_native_message(&mut input_bytes, &json!({"command":"ping"})).unwrap();

    let mut input = Cursor::new(input_bytes);
    let mut output = Vec::new();
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    run_native_host(&mut input, &mut output, &mut session)
        .await
        .unwrap();

    let mut output_reader = Cursor::new(output);
    let response_bytes = read_native_message(&mut output_reader).unwrap().unwrap();
    let response: serde_json::Value = serde_json::from_slice(&response_bytes).unwrap();

    assert_eq!(
        response,
        json!({
            "id": null,
            "ok": false,
            "error": {
                "code": "invalid_request",
                "message": "missing request id"
            }
        })
    );
}

#[tokio::test]
async fn host_loop_returns_invalid_request_for_invalid_length_prefix() {
    let (_root, paths) = test_paths();
    let mut input_bytes = Vec::new();
    input_bytes.extend_from_slice(&((MAX_NATIVE_MESSAGE_LEN as u32) + 1).to_le_bytes());

    let mut input = Cursor::new(input_bytes);
    let mut output = Vec::new();
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    run_native_host(&mut input, &mut output, &mut session)
        .await
        .unwrap();

    let mut output_reader = Cursor::new(output);
    let response_bytes = read_native_message(&mut output_reader).unwrap().unwrap();
    let response: serde_json::Value = serde_json::from_slice(&response_bytes).unwrap();

    assert_eq!(
        response,
        json!({
            "id": null,
            "ok": false,
            "error": {
                "code": "invalid_request",
                "message": format!("native message too large: {} bytes", MAX_NATIVE_MESSAGE_LEN + 1)
            }
        })
    );
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p kunkka-native-host --test host_loop`

Expected: FAIL with unresolved module `kunkka_native_host::host`.

- [ ] **Step 3: Implement host loop and main**

Update `crates/kunkka-native-host/src/lib.rs`:

```rust
pub mod bridge;
pub mod error;
pub mod host;
pub mod native_messaging;
pub mod native_protocol;
pub mod path;

pub use error::{NativeHostError, Result};
```

Create `crates/kunkka-native-host/src/host.rs`:

```rust
use crate::bridge::NativeHostSession;
use crate::native_messaging::{read_native_message, write_native_message};
use crate::native_protocol::{
    decode_request, error_response, extract_request_id, NativeErrorCode,
};
use crate::{NativeHostError, Result};
use std::io::{Read, Write};

pub async fn run_native_host<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    session: &mut NativeHostSession,
) -> Result<()> {
    loop {
        let message_bytes = match read_native_message(reader) {
            Ok(Some(message_bytes)) => message_bytes,
            Ok(None) => return Ok(()),
            Err(NativeHostError::InvalidRequest(message)) => {
                let response = error_response(None, NativeErrorCode::InvalidRequest, message);
                write_native_message(writer, &response)?;
                continue;
            }
            Err(err) => return Err(err),
        };

        let response = match decode_request(&message_bytes) {
            Ok(request) => session.handle_request(request).await,
            Err(err) => error_response(
                extract_request_id(&message_bytes),
                NativeHostError::InvalidRequest(normalize_invalid_request_message(&err)).code(),
                normalize_invalid_request_message(&err),
            ),
        };

        write_native_message(writer, &response)?;
    }

    Ok(())
}

fn normalize_invalid_request_message(err: &NativeHostError) -> String {
    match err {
        NativeHostError::Json(json_err) if json_err.to_string().contains("missing field `id`") => {
            "missing request id".to_string()
        }
        NativeHostError::InvalidRequest(message) => message.clone(),
        NativeHostError::Json(json_err) => json_err.to_string(),
        other => other.to_string(),
    }
}
```

Update `crates/kunkka-native-host/src/main.rs`:

```rust
use kunkka_native_host::bridge::NativeHostSession;
use kunkka_native_host::host::run_native_host;
use kunkka_native_host::path::resolve_core_socket_path;

#[tokio::main]
async fn main() -> kunkka_native_host::Result<()> {
    let socket_path = resolve_core_socket_path();
    let mut session = NativeHostSession::new(socket_path);
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    run_native_host(&mut reader, &mut writer, &mut session).await
}
```

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p kunkka-native-host --test host_loop`

Expected: PASS with host loop tests.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/kunkka-native-host/src/lib.rs crates/kunkka-native-host/src/host.rs crates/kunkka-native-host/src/main.rs crates/kunkka-native-host/tests/host_loop.rs
git commit -m "feat: run native messaging host loop"
```

## Task 9: Documentation and Full Verification

**Files:**

- Modify: `README.md`
- Modify: `docs/architecture.md`
- Modify: `docs/ipc.md`
- Modify: `docs/browser-extension.md`
- Modify: `docs/development-log.md`

- [ ] **Step 1: Update README implemented slices**

Update `README.md` implemented slices to include:

```markdown
- `kunkka-protocol`：共享 typed protocol crate，当前承载 core-control v1 message 和 payload codec。
- `kunkka-native-host`：第一版 WebExtension Native Messaging JSON bridge，支持 `ping` 和 `status`，并转发到 core-control IPC。
```

- [ ] **Step 2: Update IPC and architecture docs**

Update `docs/ipc.md` to state:

```markdown
Typed protocol ownership:

- IPC frame、transport、opaque payload 仍属于 `kunkka-ipc`。
- 跨 core/frontend 共享的 typed protocol 位于 `kunkka-protocol`。
- `kunkka.core-control.v1` 当前由 `kunkka-protocol::core_control` 定义。
```

Update `docs/architecture.md` current implementation section to include:

```markdown
- `kunkka-protocol`：shared core-control protocol。
- `kunkka-native-host`：Native Messaging JSON 到 Kunkka IPC core-control 的桥接入口。
```

- [ ] **Step 3: Update browser extension boundary docs**

Update `docs/browser-extension.md` Native Host section to include:

```markdown
第一版 native-host JSON API：

- request `{ "id": "req-1", "command": "ping" }` -> response `{ "id": "req-1", "ok": true, "result": { "type": "pong" } }`
- request `{ "id": "req-2", "command": "status" }` -> response `{ "id": "req-2", "ok": true, "result": { "type": "status", "worker_count": 0, "socket_path": "/run/user/1000/kunkka/core.sock", "runtime_ready": true } }`
- core 不可用时返回 `core_unavailable`。
- native-host 不自动启动 core。
```

- [ ] **Step 4: Update development log**

Append a 2026-06-11 section to `docs/development-log.md`:

````markdown
### Native Host Bridge Plan

Commit:

```text
c4b360c docs: add native host bridge design
```

Implemented in the following slice:

- `kunkka-protocol` shared core-control protocol。
- `kunkka-native-host` Native Messaging JSON bridge for `ping` and `status`。
````

- [ ] **Step 5: Format check**

Run: `cargo fmt --all --check`

Expected: PASS.

- [ ] **Step 6: Workspace tests**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 7: Clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Expected: PASS.

- [ ] **Step 8: Git status**

Run: `git status --short`

Expected: only intended documentation changes before the final docs commit, or clean after committing.

- [ ] **Step 9: Commit docs**

Run:

```bash
git add README.md docs/architecture.md docs/ipc.md docs/browser-extension.md docs/development-log.md
git commit -m "docs: document native host bridge"
```

- [ ] **Step 10: Final verification after docs commit**

Run: `cargo test --workspace`

Expected: PASS.

Run: `git status --short`

Expected: clean worktree.
