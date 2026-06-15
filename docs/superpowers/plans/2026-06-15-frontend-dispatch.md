# Frontend Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build native-host JSON app dispatch backed by a shared frontend-dispatch protocol and core-owned worker dispatch.

**Architecture:** `kunkka-protocol` owns the typed `frontend-dispatch v1` wire message and postcard payload codec. `kunkka-core` accepts that schema on frontend IPC connections, performs a temporary core-owned allow decision, and calls the existing worker dispatch manager. `kunkka-native-host` exposes a high-level Native Messaging `dispatch` JSON command and converts only JSON payloads.

**Tech Stack:** Rust 2021, Tokio, Unix Domain Socket Kunkka IPC, postcard, serde/serde_json, existing `kunkka-core` worker dispatch, existing WebExtension Native Messaging JSON bridge.

---

## Source Spec

Implement the approved design in:

```text
docs/superpowers/specs/2026-06-15-frontend-dispatch-design.md
```

This plan intentionally omits git commit steps. The current environment requires an explicit user request before committing.

## File Structure

- Create `crates/kunkka-protocol/src/frontend_dispatch.rs`: frontend-dispatch v1 typed protocol, constants, encode/decode helpers.
- Modify `crates/kunkka-protocol/src/lib.rs`: export `frontend_dispatch`.
- Create `crates/kunkka-protocol/tests/frontend_dispatch.rs`: protocol payload metadata and roundtrip tests.
- Modify `crates/kunkka-core/src/runtime.rs`: add frontend connection dispatcher, frontend-dispatch handler, temporary allow decision, platform error mapping.
- Create `crates/kunkka-core/tests/frontend_dispatch_runtime.rs`: core runtime tests for frontend-dispatch success, app error, platform error, validation, and mixed status/dispatch connection reuse.
- Modify `crates/kunkka-native-host/src/native_protocol.rs`: add dispatch command shape, dispatch result shape, and string error codes for core platform error passthrough.
- Modify `crates/kunkka-native-host/src/bridge.rs`: add JSON payload conversion, frontend-dispatch request send/receive, and response mapping.
- Modify `crates/kunkka-native-host/tests/native_protocol.rs`: dispatch JSON serde and validation tests.
- Modify `crates/kunkka-native-host/tests/bridge_mapping.rs`: dispatch payload conversion and frontend-dispatch response mapping tests.
- Modify `crates/kunkka-native-host/tests/bridge_session.rs`: native-host session integration test for `status` then `dispatch` on one cached core connection.
- Modify `README.md`: add frontend-dispatch to implemented slices after implementation.
- Modify `docs/architecture.md`: document `kunkka.frontend-dispatch.v1` runtime dispatch and native-host dispatch bridge.
- Modify `docs/ipc.md`: document frontend-dispatch schema and payload metadata.
- Modify `docs/browser-extension.md`: document the high-level Native Messaging `dispatch` command.
- Modify `docs/permissions.md`: document the temporary core-owned allow decision for frontend dispatch.
- Modify `docs/development-log.md`: add implementation and verification record.

## Task 1: Frontend Dispatch Protocol

**Files:**

- Create: `crates/kunkka-protocol/src/frontend_dispatch.rs`
- Modify: `crates/kunkka-protocol/src/lib.rs`
- Create: `crates/kunkka-protocol/tests/frontend_dispatch.rs`

- [ ] **Step 1: Add failing protocol tests**

Create `crates/kunkka-protocol/tests/frontend_dispatch.rs`:

```rust
use kunkka_ipc::{FrameMetadata, Payload};
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message,
    FrontendDispatchMessage, FrontendDispatchRequest, FrontendDispatchResponse,
    FRONTEND_DISPATCH_CONTENT_TYPE, FRONTEND_DISPATCH_SCHEMA,
};

fn json_payload(bytes: &[u8]) -> Payload {
    Payload {
        bytes: bytes.to_vec(),
        content_type: Some("application/json".to_string()),
        schema: None,
        metadata: FrameMetadata::new(),
    }
}

#[test]
fn dispatch_request_payload_sets_metadata_and_roundtrips() {
    let message = FrontendDispatchMessage::Dispatch(FrontendDispatchRequest {
        app_id: "notes".to_string(),
        method: "search".to_string(),
        payload: json_payload(br#"{"query":"kunkka"}"#),
    });

    let payload = encode_frontend_dispatch_message(&message).unwrap();

    assert_eq!(
        payload.content_type.as_deref(),
        Some(FRONTEND_DISPATCH_CONTENT_TYPE)
    );
    assert_eq!(payload.schema.as_deref(), Some(FRONTEND_DISPATCH_SCHEMA));
    assert_eq!(decode_frontend_dispatch_message(&payload).unwrap(), message);
}

#[test]
fn success_response_roundtrips() {
    let message = FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::Ok(
        json_payload(br#"{"items":[]}"#),
    ));

    let payload = encode_frontend_dispatch_message(&message).unwrap();

    assert_eq!(decode_frontend_dispatch_message(&payload).unwrap(), message);
}

#[test]
fn app_error_response_roundtrips() {
    let message = FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::AppError {
        code: "not_found".to_string(),
        message: "note not found".to_string(),
    });

    let payload = encode_frontend_dispatch_message(&message).unwrap();

    assert_eq!(decode_frontend_dispatch_message(&payload).unwrap(), message);
}

#[test]
fn platform_error_response_roundtrips() {
    let message = FrontendDispatchMessage::DispatchResult(
        FrontendDispatchResponse::PlatformError {
            code: "app_not_found".to_string(),
            message: "app not found: notes".to_string(),
        },
    );

    let payload = encode_frontend_dispatch_message(&message).unwrap();

    assert_eq!(decode_frontend_dispatch_message(&payload).unwrap(), message);
}
```

- [ ] **Step 2: Run protocol test to verify RED**

Run:

```bash
cargo test -p kunkka-protocol --test frontend_dispatch
```

Expected: FAIL with unresolved import `kunkka_protocol::frontend_dispatch`.

- [ ] **Step 3: Implement frontend-dispatch protocol module**

Create `crates/kunkka-protocol/src/frontend_dispatch.rs`:

```rust
use crate::Result;
use kunkka_ipc::{FrameMetadata, Payload};
use serde::{Deserialize, Serialize};

pub const FRONTEND_DISPATCH_CONTENT_TYPE: &str =
    "application/vnd.kunkka.frontend-dispatch.v1+postcard";
pub const FRONTEND_DISPATCH_SCHEMA: &str = "kunkka.frontend-dispatch.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontendDispatchRequest {
    pub app_id: String,
    pub method: String,
    pub payload: Payload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrontendDispatchResponse {
    Ok(Payload),
    AppError { code: String, message: String },
    PlatformError { code: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrontendDispatchMessage {
    Dispatch(FrontendDispatchRequest),
    DispatchResult(FrontendDispatchResponse),
}

pub fn encode_frontend_dispatch_message(message: &FrontendDispatchMessage) -> Result<Payload> {
    let bytes = postcard::to_stdvec(message)?;

    Ok(Payload {
        bytes,
        content_type: Some(FRONTEND_DISPATCH_CONTENT_TYPE.to_string()),
        schema: Some(FRONTEND_DISPATCH_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_frontend_dispatch_message(payload: &Payload) -> Result<FrontendDispatchMessage> {
    Ok(postcard::from_bytes(&payload.bytes)?)
}
```

Modify `crates/kunkka-protocol/src/lib.rs`:

```rust
pub mod core_control;
pub mod error;
pub mod frontend_dispatch;

pub use error::{ProtocolError, Result};
```

- [ ] **Step 4: Run protocol test to verify GREEN**

Run:

```bash
cargo test -p kunkka-protocol --test frontend_dispatch
cargo test -p kunkka-protocol
```

Expected: PASS.

## Task 2: Core Runtime Frontend Dispatch

**Files:**

- Modify: `crates/kunkka-core/src/runtime.rs`
- Create: `crates/kunkka-core/tests/frontend_dispatch_runtime.rs`

- [ ] **Step 1: Add failing warm dispatch runtime test**

Create `crates/kunkka-core/tests/frontend_dispatch_runtime.rs` with the shared imports and helpers plus the first test:

```rust
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_core::CoreError;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CoreStatusRequest,
};
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message,
    FrontendDispatchMessage, FrontendDispatchRequest, FrontendDispatchResponse,
};
use kunkka_worker_sdk::{
    AppId, DispatchWorkerResponse, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId,
};
use std::future::Future;
use tempfile::{tempdir, TempDir};
use tokio::time::{timeout, Duration};

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

async fn wait_for<T>(future: impl Future<Output = T>) -> T {
    timeout(TEST_TIMEOUT, future)
        .await
        .expect("test operation timed out")
}

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

fn json_payload(bytes: &[u8]) -> Payload {
    Payload {
        bytes: bytes.to_vec(),
        content_type: Some("application/json".to_string()),
        schema: None,
        metadata: FrameMetadata::new(),
    }
}

fn worker_request() -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new("notes"),
        app_id: AppId::new("notes"),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: None,
        }],
    }
}

async fn register_worker_and_wait_for_dispatch(
    socket_path: std::path::PathBuf,
    response: DispatchWorkerResponse,
) -> kunkka_worker_sdk::RegisterWorkerResponse {
    let mut client = WorkerClient::connect(&socket_path, WorkerId::new("notes"))
        .await
        .unwrap();
    let registration = client.register(worker_request()).await.unwrap();
    let request = wait_for(client.recv_dispatch()).await.unwrap();
    assert_eq!(request.request.app_id.as_str(), "notes");
    assert_eq!(request.request.method, "search");
    client.respond_dispatch(request, response).await.unwrap();
    registration
}

fn dispatch_frame(request_id: u128, app_id: &str, method: &str) -> Frame {
    let payload = encode_frontend_dispatch_message(&FrontendDispatchMessage::Dispatch(
        FrontendDispatchRequest {
            app_id: app_id.to_string(),
            method: method.to_string(),
            payload: json_payload(br#"{"query":"kunkka"}"#),
        },
    ))
    .unwrap();

    Frame::Request {
        request_id: RequestId(request_id),
        session_id: SessionId(1),
        source: EndpointId::new("native-host"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    }
}

#[tokio::test]
async fn frontend_dispatch_calls_warm_worker_and_returns_payload() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let worker_task = tokio::spawn(register_worker_and_wait_for_dispatch(
        paths.socket_path.clone(),
        DispatchWorkerResponse::Ok(json_payload(br#"{"items":[]}"#)),
    ));
    wait_for(runtime.run_once()).await.unwrap();

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection
                .send_frame(&dispatch_frame(10, "notes", "search"))
                .await
                .unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let response_frame = wait_for(frontend_task).await.unwrap();
    let Frame::Response {
        request_id, payload, ..
    } = response_frame
    else {
        panic!("expected response frame");
    };
    assert_eq!(request_id, RequestId(10));
    assert_eq!(
        decode_frontend_dispatch_message(&payload).unwrap(),
        FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::Ok(json_payload(
            br#"{"items":[]}"#
        )))
    );
    assert!(wait_for(worker_task).await.unwrap().accepted);
}
```

- [ ] **Step 2: Run core frontend dispatch test to verify RED**

Run:

```bash
cargo test -p kunkka-core --test frontend_dispatch_runtime frontend_dispatch_calls_warm_worker_and_returns_payload
```

Expected: FAIL because runtime does not handle `kunkka.frontend-dispatch.v1`.

- [ ] **Step 3: Implement core frontend connection dispatcher and handler**

Modify `crates/kunkka-core/src/runtime.rs` imports:

```rust
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message,
    FrontendDispatchMessage, FrontendDispatchRequest, FrontendDispatchResponse,
    FRONTEND_DISPATCH_SCHEMA,
};
```

Replace the non-worker branch in `run_connection` so core-control and frontend-dispatch share one frontend loop:

```rust
        Some(CORE_CONTROL_SCHEMA | FRONTEND_DISPATCH_SCHEMA) => {
            run_frontend_connection(server, worker_manager, connection, first_frame).await
        }
```

Rename `run_control_connection` to `run_frontend_connection` and use this body:

```rust
async fn run_frontend_connection(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    mut connection: IpcConnection,
    first_frame: Frame,
) -> Result<()> {
    let response = handle_frontend_frame(server, worker_manager, first_frame).await?;
    connection.send_frame(&response).await?;
    let mut reap_interval = interval(IDLE_REAP_INTERVAL);
    reap_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            frame = connection.recv_frame() => {
                let Some(frame) = frame? else {
                    return Ok(());
                };
                let response = handle_frontend_frame(server, worker_manager, frame).await?;
                connection.send_frame(&response).await?;
            }
            _ = reap_interval.tick() => {
                worker_manager.reap_idle_workers();
            }
        }
    }
}
```

Add these helpers below `run_frontend_connection`:

```rust
async fn handle_frontend_frame(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    frame: Frame,
) -> Result<Frame> {
    match frame_schema(&frame) {
        Some(CORE_CONTROL_SCHEMA) => handle_control_frame(server, worker_manager.registry(), frame),
        Some(FRONTEND_DISPATCH_SCHEMA) => {
            handle_frontend_dispatch_frame(server, worker_manager, frame).await
        }
        Some(schema) => Err(CoreError::InvalidCoreFrame(format!(
            "unknown payload schema: {schema}"
        ))),
        None => Err(CoreError::InvalidCoreFrame(
            "missing payload schema".to_string(),
        )),
    }
}

async fn handle_frontend_dispatch_frame(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    frame: Frame,
) -> Result<Frame> {
    let Frame::Request {
        request_id,
        session_id,
        source,
        target,
        payload,
        ..
    } = frame
    else {
        return Err(CoreError::InvalidCoreFrame(
            "expected request frame".to_string(),
        ));
    };

    let response = match decode_frontend_dispatch_message(&payload)? {
        FrontendDispatchMessage::Dispatch(request) => {
            handle_frontend_dispatch_request(server, worker_manager, request).await
        }
        _ => {
            return Err(CoreError::InvalidCoreFrame(
                "expected frontend dispatch request".to_string(),
            ));
        }
    };

    let payload = encode_frontend_dispatch_message(
        &FrontendDispatchMessage::DispatchResult(response),
    )?;

    Ok(Frame::Response {
        request_id,
        session_id,
        source: target_or_core(target),
        target: source,
        payload,
        metadata: FrameMetadata::new(),
    })
}

async fn handle_frontend_dispatch_request(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    request: FrontendDispatchRequest,
) -> FrontendDispatchResponse {
    if request.app_id.is_empty() {
        return platform_error("invalid_request", "dispatch app_id is empty");
    }
    if request.method.is_empty() {
        return platform_error("invalid_request", "dispatch method is empty");
    }
    if !allow_frontend_dispatch_v1(&request) {
        return platform_error("permission_denied", "frontend dispatch is not allowed");
    }

    match worker_manager
        .dispatch_with_start(
            server,
            AppId::new(request.app_id),
            request.method,
            request.payload,
        )
        .await
    {
        Ok(DispatchResult::Ok(payload)) => FrontendDispatchResponse::Ok(payload),
        Ok(DispatchResult::AppError { code, message }) => {
            FrontendDispatchResponse::AppError { code, message }
        }
        Err(err) => platform_error(dispatch_platform_error_code(&err), err.to_string()),
    }
}

fn allow_frontend_dispatch_v1(_request: &FrontendDispatchRequest) -> bool {
    true
}

fn platform_error(
    code: impl Into<String>,
    message: impl Into<String>,
) -> FrontendDispatchResponse {
    FrontendDispatchResponse::PlatformError {
        code: code.into(),
        message: message.into(),
    }
}

fn dispatch_platform_error_code(error: &CoreError) -> &'static str {
    match error {
        CoreError::AppNotFound(_) => "app_not_found",
        CoreError::WorkerStartFailed(_) => "worker_start_failed",
        CoreError::WorkerStartTimeout(_) => "worker_start_timeout",
        CoreError::WorkerUnavailable(_) => "worker_unavailable",
        CoreError::DispatchIpcError(_) => "dispatch_ipc_error",
        CoreError::UnexpectedWorkerResponse(_) => "unexpected_worker_response",
        CoreError::InvalidCoreFrame(_) => "invalid_request",
        _ => "core_error",
    }
}
```

- [ ] **Step 4: Run warm dispatch test to verify GREEN**

Run:

```bash
cargo test -p kunkka-core --test frontend_dispatch_runtime frontend_dispatch_calls_warm_worker_and_returns_payload
```

Expected: PASS.

- [ ] **Step 5: Add app error, platform error, validation, non-request, and mixed connection tests**

Append these tests to `crates/kunkka-core/tests/frontend_dispatch_runtime.rs`:

```rust
#[tokio::test]
async fn frontend_dispatch_returns_worker_app_error() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let worker_task = tokio::spawn(register_worker_and_wait_for_dispatch(
        paths.socket_path.clone(),
        DispatchWorkerResponse::Err(kunkka_worker_sdk::WorkerAppError {
            code: "not_found".to_string(),
            message: "note not found".to_string(),
        }),
    ));
    wait_for(runtime.run_once()).await.unwrap();

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection
                .send_frame(&dispatch_frame(11, "notes", "search"))
                .await
                .unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let Frame::Response { payload, .. } = wait_for(frontend_task).await.unwrap() else {
        panic!("expected response frame");
    };
    assert_eq!(
        decode_frontend_dispatch_message(&payload).unwrap(),
        FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::AppError {
            code: "not_found".to_string(),
            message: "note not found".to_string(),
        })
    );
    assert!(wait_for(worker_task).await.unwrap().accepted);
}

#[tokio::test]
async fn frontend_dispatch_maps_missing_manifest_to_platform_error() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection
                .send_frame(&dispatch_frame(12, "missing", "search"))
                .await
                .unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let Frame::Response { payload, .. } = wait_for(frontend_task).await.unwrap() else {
        panic!("expected response frame");
    };
    let FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::PlatformError {
        code,
        message,
    }) = decode_frontend_dispatch_message(&payload).unwrap()
    else {
        panic!("expected platform error");
    };
    assert_eq!(code, "app_not_found");
    assert!(message.contains("missing"));
}

#[tokio::test]
async fn frontend_dispatch_rejects_empty_app_id_as_platform_error() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection
                .send_frame(&dispatch_frame(13, "", "search"))
                .await
                .unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let Frame::Response { payload, .. } = wait_for(frontend_task).await.unwrap() else {
        panic!("expected response frame");
    };
    let FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::PlatformError {
        code,
        message,
    }) = decode_frontend_dispatch_message(&payload).unwrap()
    else {
        panic!("expected platform error");
    };
    assert_eq!(code, "invalid_request");
    assert!(message.contains("app_id"));
}

#[tokio::test]
async fn frontend_dispatch_event_returns_invalid_core_frame() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let Frame::Request { payload, .. } = dispatch_frame(14, "notes", "search") else {
        panic!("expected request frame");
    };
    let event = Frame::Event {
        session_id: SessionId(1),
        source: EndpointId::new("native-host"),
        target: EndpointId::new("core"),
        name: "frontend-dispatch".to_string(),
        payload,
        metadata: FrameMetadata::new(),
    };

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&event).await.unwrap();
        }
    });

    let err = wait_for(runtime.run_once()).await.unwrap_err();
    wait_for(frontend_task).await.unwrap();
    assert!(matches!(
        err,
        CoreError::InvalidCoreFrame(message) if message.contains("expected request frame")
    ));
}

#[tokio::test]
async fn one_frontend_connection_can_handle_status_then_dispatch() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let worker_task = tokio::spawn(register_worker_and_wait_for_dispatch(
        paths.socket_path.clone(),
        DispatchWorkerResponse::Ok(json_payload(br#"{"items":["a"]}"#)),
    ));
    wait_for(runtime.run_once()).await.unwrap();

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            let status_payload =
                encode_control_message(&CoreControlMessage::Status(CoreStatusRequest)).unwrap();
            let status_frame = Frame::Request {
                request_id: RequestId(20),
                session_id: SessionId(1),
                source: EndpointId::new("native-host"),
                target: EndpointId::new("core"),
                payload: status_payload,
                metadata: FrameMetadata::new(),
            };
            connection.send_frame(&status_frame).await.unwrap();
            let status_response = connection.recv_frame().await.unwrap().unwrap();

            connection
                .send_frame(&dispatch_frame(21, "notes", "search"))
                .await
                .unwrap();
            let dispatch_response = connection.recv_frame().await.unwrap().unwrap();

            (status_response, dispatch_response)
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let (status_response, dispatch_response) = wait_for(frontend_task).await.unwrap();
    let Frame::Response {
        payload: status_payload,
        ..
    } = status_response
    else {
        panic!("expected status response");
    };
    assert!(matches!(
        decode_control_message(&status_payload).unwrap(),
        CoreControlMessage::StatusResult(_)
    ));

    let Frame::Response {
        payload: dispatch_payload,
        ..
    } = dispatch_response
    else {
        panic!("expected dispatch response");
    };
    assert_eq!(
        decode_frontend_dispatch_message(&dispatch_payload).unwrap(),
        FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::Ok(json_payload(
            br#"{"items":["a"]}"#
        )))
    );
    assert!(wait_for(worker_task).await.unwrap().accepted);
}
```

- [ ] **Step 6: Run core tests to verify GREEN**

Run:

```bash
cargo test -p kunkka-core --test frontend_dispatch_runtime
cargo test -p kunkka-core --test core_runtime_control
```

Expected: PASS.

## Task 3: Native Protocol Dispatch JSON

**Files:**

- Modify: `crates/kunkka-native-host/src/native_protocol.rs`
- Modify: `crates/kunkka-native-host/tests/native_protocol.rs`

- [ ] **Step 1: Add failing native protocol JSON tests**

Append to `crates/kunkka-native-host/tests/native_protocol.rs`:

```rust
#[test]
fn decodes_dispatch_request_with_json_payload() {
    let request = decode_request(
        br#"{"id":"req-3","command":"dispatch","app_id":"notes","method":"search","payload":{"query":"kunkka"}}"#,
    )
    .unwrap();

    assert_eq!(request.id, "req-3");
    assert_eq!(
        request.command,
        NativeCommand::Dispatch {
            app_id: "notes".to_string(),
            method: "search".to_string(),
            payload: serde_json::json!({"query":"kunkka"}),
        }
    );
}

#[test]
fn rejects_dispatch_request_with_empty_app_id() {
    let err = decode_request(
        br#"{"id":"req-4","command":"dispatch","app_id":"","method":"search","payload":{}}"#,
    )
    .unwrap_err();

    assert!(err.to_string().contains("app_id"));
}

#[test]
fn serializes_dispatch_success_response() {
    let response = success_response(
        "req-5",
        NativeResult::Dispatch {
            payload: serde_json::json!({"items": []}),
        },
    );

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": "req-5",
            "ok": true,
            "result": {
                "type": "dispatch",
                "payload": { "items": [] }
            }
        })
    );
}

#[test]
fn serializes_dispatch_app_error_response() {
    let response = success_response(
        "req-6",
        NativeResult::DispatchError {
            code: "not_found".to_string(),
            message: "note not found".to_string(),
        },
    );

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": "req-6",
            "ok": true,
            "result": {
                "type": "dispatch_error",
                "code": "not_found",
                "message": "note not found"
            }
        })
    );
}

#[test]
fn serializes_platform_error_code_as_string() {
    let response = error_response(Some("req-7".to_string()), "app_not_found", "missing app");

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": "req-7",
            "ok": false,
            "error": {
                "code": "app_not_found",
                "message": "missing app"
            }
        })
    );
}
```

- [ ] **Step 2: Run native protocol test to verify RED**

Run:

```bash
cargo test -p kunkka-native-host --test native_protocol
```

Expected: FAIL because `NativeCommand::Dispatch`, `NativeResult::Dispatch`, and string platform error codes do not exist yet.

- [ ] **Step 3: Implement native protocol dispatch shape**

Modify `crates/kunkka-native-host/src/native_protocol.rs` to this shape:

```rust
use crate::{NativeHostError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NativeRequest {
    pub id: String,
    #[serde(flatten)]
    pub command: NativeCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum NativeCommand {
    Ping,
    Status,
    Dispatch {
        app_id: String,
        method: String,
        payload: serde_json::Value,
    },
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
    Dispatch {
        payload: serde_json::Value,
    },
    DispatchError {
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeErrorBody {
    pub code: String,
    pub message: String,
}
```

Keep `NativeErrorCode` and its `Display` implementation so existing `NativeHostError::code()` keeps working. Replace `decode_request`, `success_response`, and `error_response` with:

```rust
pub fn decode_request(bytes: &[u8]) -> Result<NativeRequest> {
    let request: NativeRequest = serde_json::from_slice(bytes)?;

    if request.id.is_empty() {
        return Err(NativeHostError::InvalidRequest(
            "missing request id".to_string(),
        ));
    }

    if let NativeCommand::Dispatch { app_id, method, .. } = &request.command {
        if app_id.is_empty() {
            return Err(NativeHostError::InvalidRequest(
                "dispatch app_id is empty".to_string(),
            ));
        }
        if method.is_empty() {
            return Err(NativeHostError::InvalidRequest(
                "dispatch method is empty".to_string(),
            ));
        }
    }

    Ok(request)
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
    code: impl ToString,
    message: impl Into<String>,
) -> NativeResponse {
    NativeResponse {
        id,
        ok: false,
        result: None,
        error: Some(NativeErrorBody {
            code: code.to_string(),
            message: message.into(),
        }),
    }
}
```

- [ ] **Step 4: Run native protocol test to verify GREEN**

Run:

```bash
cargo test -p kunkka-native-host --test native_protocol
```

Expected: PASS.

## Task 4: Native Host Dispatch Bridge Mapping

**Files:**

- Modify: `crates/kunkka-native-host/src/bridge.rs`
- Modify: `crates/kunkka-native-host/tests/bridge_mapping.rs`

- [ ] **Step 1: Add failing bridge mapping tests**

Append to `crates/kunkka-native-host/tests/bridge_mapping.rs`:

```rust
use kunkka_native_host::bridge::{
    frontend_dispatch_request_for_command, native_result_for_frontend_dispatch_response,
};
use kunkka_protocol::frontend_dispatch::FrontendDispatchResponse;

#[test]
fn maps_dispatch_command_to_frontend_dispatch_request() {
    let request = frontend_dispatch_request_for_command(&NativeCommand::Dispatch {
        app_id: "notes".to_string(),
        method: "search".to_string(),
        payload: serde_json::json!({"query":"kunkka"}),
    })
    .unwrap();

    assert_eq!(request.app_id, "notes");
    assert_eq!(request.method, "search");
    assert_eq!(request.payload.content_type.as_deref(), Some("application/json"));
    assert_eq!(request.payload.schema, None);
    assert_eq!(request.payload.bytes, br#"{"query":"kunkka"}"#);
}

#[test]
fn maps_frontend_dispatch_success_to_native_dispatch_result() {
    let result = native_result_for_frontend_dispatch_response(FrontendDispatchResponse::Ok(
        kunkka_ipc::Payload {
            bytes: br#"{"items":[]}"#.to_vec(),
            content_type: Some("application/json".to_string()),
            schema: None,
            metadata: kunkka_ipc::FrameMetadata::new(),
        },
    ))
    .unwrap();

    assert_eq!(
        result,
        NativeResult::Dispatch {
            payload: serde_json::json!({"items": []}),
        }
    );
}

#[test]
fn maps_frontend_dispatch_app_error_to_native_dispatch_error() {
    let result = native_result_for_frontend_dispatch_response(
        FrontendDispatchResponse::AppError {
            code: "not_found".to_string(),
            message: "note not found".to_string(),
        },
    )
    .unwrap();

    assert_eq!(
        result,
        NativeResult::DispatchError {
            code: "not_found".to_string(),
            message: "note not found".to_string(),
        }
    );
}

#[test]
fn rejects_frontend_dispatch_success_with_non_json_content_type() {
    let err = native_result_for_frontend_dispatch_response(FrontendDispatchResponse::Ok(
        kunkka_ipc::Payload {
            bytes: b"raw".to_vec(),
            content_type: Some("application/octet-stream".to_string()),
            schema: None,
            metadata: kunkka_ipc::FrameMetadata::new(),
        },
    ))
    .unwrap_err();

    assert!(matches!(err, NativeHostError::UnexpectedCoreResponse(_)));
}
```

- [ ] **Step 2: Run bridge mapping test to verify RED**

Run:

```bash
cargo test -p kunkka-native-host --test bridge_mapping
```

Expected: FAIL because dispatch bridge mapping helpers do not exist.

- [ ] **Step 3: Implement bridge mapping helpers**

Modify `crates/kunkka-native-host/src/bridge.rs` imports:

```rust
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message,
    FrontendDispatchMessage, FrontendDispatchRequest, FrontendDispatchResponse,
};
```

Add these helpers near the existing mapping functions:

```rust
const JSON_CONTENT_TYPE: &str = "application/json";

pub fn frontend_dispatch_request_for_command(
    command: &NativeCommand,
) -> Result<FrontendDispatchRequest> {
    let NativeCommand::Dispatch {
        app_id,
        method,
        payload,
    } = command
    else {
        return Err(NativeHostError::InvalidRequest(
            "expected dispatch command".to_string(),
        ));
    };

    Ok(FrontendDispatchRequest {
        app_id: app_id.clone(),
        method: method.clone(),
        payload: json_payload_for_native_value(payload)?,
    })
}

fn json_payload_for_native_value(value: &serde_json::Value) -> Result<Payload> {
    let bytes = serde_json::to_vec(value)?;

    Ok(Payload {
        bytes,
        content_type: Some(JSON_CONTENT_TYPE.to_string()),
        schema: None,
        metadata: FrameMetadata::new(),
    })
}

pub fn native_result_for_frontend_dispatch_response(
    response: FrontendDispatchResponse,
) -> Result<NativeResult> {
    match response {
        FrontendDispatchResponse::Ok(payload) => Ok(NativeResult::Dispatch {
            payload: native_value_for_json_payload(&payload)?,
        }),
        FrontendDispatchResponse::AppError { code, message } => {
            Ok(NativeResult::DispatchError { code, message })
        }
        FrontendDispatchResponse::PlatformError { code, message } => {
            Err(NativeHostError::CorePlatform { code, message })
        }
    }
}

fn native_value_for_json_payload(payload: &Payload) -> Result<serde_json::Value> {
    if payload.content_type.as_deref() != Some(JSON_CONTENT_TYPE) {
        return Err(NativeHostError::UnexpectedCoreResponse(format!(
            "expected JSON dispatch payload, got {:?}",
            payload.content_type
        )));
    }

    serde_json::from_slice(&payload.bytes).map_err(NativeHostError::from)
}
```

Modify `crates/kunkka-native-host/src/error.rs` to add platform passthrough:

```rust
    #[error("core platform error {code}: {message}")]
    CorePlatform { code: String, message: String },
```

Update `NativeHostError::code()` by adding this fallback branch. In `handle_request`, match `NativeHostError::CorePlatform` explicitly and call `error_response(Some(id), code, message)`, so this fallback is only used if a caller treats a platform error like a generic native-host error:

```rust
            Self::CorePlatform { .. } => NativeErrorCode::UnexpectedCoreResponse,
```

This branch is only a fallback; `handle_request` will handle it explicitly.

- [ ] **Step 4: Run bridge mapping test to verify GREEN**

Run:

```bash
cargo test -p kunkka-native-host --test bridge_mapping
```

Expected: PASS.

## Task 5: Native Host Dispatch Session

**Files:**

- Modify: `crates/kunkka-native-host/src/bridge.rs`
- Modify: `crates/kunkka-native-host/tests/bridge_session.rs`

- [ ] **Step 1: Add failing native-host session integration test**

Append to `crates/kunkka-native-host/tests/bridge_session.rs`:

```rust
use kunkka_worker_sdk::DispatchWorkerResponse;

fn dispatch_request(id: &str) -> NativeRequest {
    NativeRequest {
        id: id.to_string(),
        command: NativeCommand::Dispatch {
            app_id: "example-app".to_string(),
            method: "search".to_string(),
            payload: serde_json::json!({"query":"kunkka"}),
        },
    }
}

#[tokio::test]
async fn session_reuses_connection_for_status_then_dispatch() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let worker_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("example-app"))
                .await
                .unwrap();
            let registration = client.register(RegisterWorkerRequest {
                worker_id: WorkerId::new("example-app"),
                app_id: AppId::new("example-app"),
                capabilities: vec![WorkerCapability {
                    name: "notes.search".to_string(),
                    description: None,
                }],
            })
            .await
            .unwrap();
            let dispatch = wait_for(client.recv_dispatch()).await.unwrap();
            assert_eq!(dispatch.request.method, "search");
            assert_eq!(dispatch.request.payload.content_type.as_deref(), Some("application/json"));
            assert_eq!(dispatch.request.payload.bytes, br#"{"query":"kunkka"}"#);
            client
                .respond_dispatch(
                    dispatch,
                    DispatchWorkerResponse::Ok(Payload {
                        bytes: br#"{"items":[]}"#.to_vec(),
                        content_type: Some("application/json".to_string()),
                        schema: None,
                        metadata: FrameMetadata::new(),
                    }),
                )
                .await
                .unwrap();
            registration
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    let status = wait_for(session.handle_request(NativeRequest {
        id: "req-status".to_string(),
        command: NativeCommand::Status,
    }))
    .await;
    assert!(matches!(status.result, Some(NativeResult::Status { .. })));

    let dispatch = wait_for(session.handle_request(dispatch_request("req-dispatch"))).await;
    assert_eq!(dispatch.id.as_deref(), Some("req-dispatch"));
    assert_eq!(
        dispatch.result,
        Some(NativeResult::Dispatch {
            payload: serde_json::json!({"items": []}),
        })
    );

    assert!(wait_for(worker_task).await.unwrap().accepted);
    drop(session);
    wait_for(runtime_task).await.unwrap();
}
```

If the test name conflicts with the existing `session_reuses_connection_for_ping_then_status`, keep both names distinct exactly as shown.

- [ ] **Step 2: Run native-host session test to verify RED**

Run:

```bash
cargo test -p kunkka-native-host --test bridge_session session_reuses_connection_for_status_then_dispatch
```

Expected: FAIL because `NativeHostSession` does not send frontend-dispatch requests.

- [ ] **Step 3: Implement frontend-dispatch session send/receive**

Modify `NativeHostSession::handle_request` in `crates/kunkka-native-host/src/bridge.rs`:

```rust
    pub async fn handle_request(&mut self, request: NativeRequest) -> NativeResponse {
        let id = request.id.clone();

        match request.command.clone() {
            NativeCommand::Ping | NativeCommand::Status => {
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
            NativeCommand::Dispatch { .. } => {
                let dispatch_request = match frontend_dispatch_request_for_command(&request.command) {
                    Ok(dispatch_request) => dispatch_request,
                    Err(err) => return error_response(Some(id), err.code(), err.to_string()),
                };

                match self.send_frontend_dispatch(dispatch_request).await {
                    Ok(FrontendDispatchResponse::PlatformError { code, message }) => {
                        error_response(Some(id), code, message)
                    }
                    Ok(response) => match native_result_for_frontend_dispatch_response(response) {
                        Ok(result) => success_response(id, result),
                        Err(NativeHostError::CorePlatform { code, message }) => {
                            error_response(Some(id), code, message)
                        }
                        Err(err) => error_response(Some(id), err.code(), err.to_string()),
                    },
                    Err(err) => error_response(Some(id), err.code(), err.to_string()),
                }
            }
        }
    }
```

Add the send helpers inside `impl NativeHostSession`:

```rust
    async fn send_frontend_dispatch(
        &mut self,
        request: FrontendDispatchRequest,
    ) -> Result<FrontendDispatchResponse> {
        self.ensure_connection().await?;
        let result = self
            .send_frontend_dispatch_on_cached_connection(request)
            .await;

        if result.is_err() {
            self.connection = None;
        }

        result
    }

    async fn send_frontend_dispatch_on_cached_connection(
        &mut self,
        request: FrontendDispatchRequest,
    ) -> Result<FrontendDispatchResponse> {
        let request_id = self.next_request_id();
        let payload = encode_frontend_dispatch_message(&FrontendDispatchMessage::Dispatch(request))
            .map_err(|err| NativeHostError::CoreIpc(err.to_string()))?;
        let frame = Frame::Request {
            request_id,
            session_id: self.session_id,
            source: self.source.clone(),
            target: self.target.clone(),
            payload,
            metadata: FrameMetadata::new(),
        };

        let connection = self.connection.as_mut().ok_or_else(|| {
            NativeHostError::CoreUnavailable("core connection missing".to_string())
        })?;

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

        match decode_frontend_dispatch_message(&payload)
            .map_err(|err| NativeHostError::CoreIpc(err.to_string()))?
        {
            FrontendDispatchMessage::DispatchResult(response) => Ok(response),
            message => Err(NativeHostError::UnexpectedCoreResponse(format!(
                "expected frontend dispatch result, got {message:?}"
            ))),
        }
    }
```

- [ ] **Step 4: Run native-host session test to verify GREEN**

Run:

```bash
cargo test -p kunkka-native-host --test bridge_session session_reuses_connection_for_status_then_dispatch
```

Expected: PASS.

- [ ] **Step 5: Run all native-host tests**

Run:

```bash
cargo test -p kunkka-native-host
```

Expected: PASS.

## Task 6: Documentation Updates

**Files:**

- Modify: `README.md`
- Modify: `docs/architecture.md`
- Modify: `docs/ipc.md`
- Modify: `docs/browser-extension.md`
- Modify: `docs/permissions.md`
- Modify: `docs/development-log.md`

- [ ] **Step 1: Update README implemented slices**

Modify the `kunkka-protocol`, `kunkka-core`, and `kunkka-native-host` bullets in `README.md` to include frontend dispatch:

```markdown
- `kunkka-protocol`：共享 typed protocol crate，当前承载 core-control v1、frontend-dispatch v1 message 和 payload codec。
- `kunkka-core`：XDG path resolution、private runtime directory setup、minimal core IPC socket binding、in-memory worker registration、single-connection worker registration runtime loop、core control protocol、XDG app manifest loading、按需 worker startup、core-internal worker dispatch、frontend-dispatch runtime handler，以及 idle worker cleanup。
- `kunkka-native-host`：WebExtension Native Messaging JSON bridge，支持 `ping`、`status` 和 JSON `dispatch`，并转发到 core IPC。
```

- [ ] **Step 2: Update architecture current slice and schema list**

Modify `docs/architecture.md` current implementation slice to include frontend dispatch:

```markdown
- `kunkka-protocol`：shared core-control protocol 和 frontend-dispatch protocol。
- `kunkka-core`：XDG path management、runtime socket setup、single-connection runtime loop、in-memory worker registry、core control protocol、XDG app manifest registry、worker startup / active registry / idle cleanup manager、core-internal dispatch API、frontend-dispatch runtime handler。
- `kunkka-native-host`：Native Messaging JSON 到 Kunkka IPC core-control/frontend-dispatch 的桥接入口。
```

Add the schema to the runtime list:

```markdown
- `kunkka.frontend-dispatch.v1` 处理 frontend 到 app worker 的 dispatch request。
```

- [ ] **Step 3: Update IPC documentation**

In `docs/ipc.md`, add a frontend-dispatch subsection with this content:

```markdown
## Frontend Dispatch v1

Frontend dispatch payload metadata:

```text
content_type = application/vnd.kunkka.frontend-dispatch.v1+postcard
schema = kunkka.frontend-dispatch.v1
```

`kunkka-protocol` owns the typed message and codec. `kunkka-core` handles this schema on frontend IPC connections and routes to worker dispatch. `kunkka-ipc` remains unaware of app dispatch semantics.
```
```

- [ ] **Step 4: Update Browser Extension boundary document**

In `docs/browser-extension.md`, document the Native Messaging dispatch command:

```markdown
## Native Messaging Dispatch

Browser Extension may request app dispatch through `kunkka-native-host` with high-level JSON:

```json
{ "id": "req-1", "command": "dispatch", "app_id": "notes", "method": "search", "payload": { "query": "hello" } }
```

The extension does not see Kunkka IPC frames, Unix sockets, postcard payloads, or worker protocol messages. `kunkka-native-host` forwards the request to `kunkka-core` using `kunkka.frontend-dispatch.v1`.
```
```

- [ ] **Step 5: Update permissions document**

Append to `docs/permissions.md`:

```markdown
## Current Frontend Dispatch Status

Frontend dispatch is currently allowed by an explicit temporary decision inside `kunkka-core`. This keeps the permission decision owner in core and avoids native-host-side authorization logic.

The temporary allow decision must be replaced by the real permission system before Kunkka treats worker invocation as enforceable per subject or per app.
```

- [ ] **Step 6: Update development log**

Prepend to `docs/development-log.md` under `## 2026-06-15`:

```markdown
### Frontend Dispatch

Implemented:

- `kunkka-protocol` frontend-dispatch v1 protocol and payload codec。
- `kunkka-core` frontend-dispatch runtime handler backed by existing worker dispatch。
- Core-owned temporary allow decision for frontend dispatch。
- `kunkka-native-host` Native Messaging JSON `dispatch` command。
- JSON payload conversion between Native Messaging and opaque Kunkka IPC payloads。

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
```

- [ ] **Step 7: Run markdown and code-facing checks**

Run:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

## Task 7: Full Verification and Cleanup

**Files:**

- Inspect all modified files.

- [ ] **Step 1: Run full workspace verification**

Run:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 2: Inspect git diff**

Run:

```bash
git diff --stat
git diff -- docs/superpowers/specs/2026-06-15-frontend-dispatch-design.md docs/superpowers/plans/2026-06-15-frontend-dispatch.md
git diff -- crates/kunkka-protocol crates/kunkka-core crates/kunkka-native-host README.md docs
```

Expected: Diff contains only frontend-dispatch protocol, core handler, native-host dispatch bridge, tests, and documentation updates.

- [ ] **Step 3: Confirm no unrelated worktree changes were introduced**

Run:

```bash
git status --short
```

Expected: Only files touched by this plan are modified or added.
