# Worker Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build core-only worker dispatch with XDG JSON app manifests, on-demand worker process startup, live IPC dispatch, app error pass-through, and idle cleanup.

**Architecture:** `kunkka-core` owns app manifest loading, active worker lifecycle, process startup, dispatch routing, and idle cleanup. `kunkka-worker-sdk` owns worker-facing dispatch protocol and helpers. `kunkka-ipc` stays unchanged as frame/transport/opaque `Payload`; native-host/CLI/TUI dispatch entrypoints remain out of scope.

**Tech Stack:** Rust 2021, Tokio, Unix Domain Socket Kunkka IPC, postcard worker protocol codec, serde/serde_json app manifest parsing, XDG paths from `KunkkaPaths`.

---

## Source Spec

Implement the approved design in:

```text
docs/superpowers/specs/2026-06-12-worker-dispatch-design.md
```

Keep the implementation focused on Core API First. Do not add native-host, CLI, TUI, permission, SQLite, stream, cancellation, heartbeat, or multi-worker dispatch entrypoints in this plan.

## File Structure

Create or modify these files:

- Modify `crates/kunkka-core/Cargo.toml`: add `serde.workspace = true` and `serde_json.workspace = true`.
- Modify `crates/kunkka-core/src/lib.rs`: export `app_manifest` and `worker_dispatch` modules.
- Modify `crates/kunkka-core/src/error.rs`: add manifest and dispatch platform error variants.
- Create `crates/kunkka-core/src/app_manifest.rs`: JSON manifest types and XDG config loader.
- Modify `crates/kunkka-worker-sdk/src/types.rs`: add dispatch request/response and app error types.
- Modify `crates/kunkka-worker-sdk/src/lib.rs`: export new worker dispatch types.
- Modify `crates/kunkka-worker-sdk/src/client.rs`: add receive/respond helpers for worker dispatch.
- Modify `crates/kunkka-core/src/worker_registry.rs`: make registration replace by `AppId`, while preserving lookup by `WorkerId` for existing tests.
- Create `crates/kunkka-core/src/worker_dispatch.rs`: active worker lifecycle and warm/cold dispatch.
- Modify `crates/kunkka-core/src/runtime.rs`: load app registry, own `WorkerManager`, hand worker registration connections to manager, expose internal dispatch API.
- Create `crates/kunkka-core/tests/app_manifest.rs`: manifest loader tests.
- Modify `crates/kunkka-worker-sdk/tests/registration_codec.rs`: protocol codec tests for dispatch messages.
- Create `crates/kunkka-worker-sdk/tests/dispatch_client.rs`: worker client receive/respond helper tests.
- Modify `crates/kunkka-core/tests/worker_registry.rs`: replacement by `AppId` tests.
- Create `crates/kunkka-core/tests/worker_dispatch_warm.rs`: active worker warm dispatch tests.
- Create `crates/kunkka-core/tests/worker_runtime_registration.rs`: runtime registration handoff tests.
- Create `crates/kunkka-core/tests/worker_dispatch_cold.rs`: process startup and cold dispatch tests with a self-hosted fixture.
- Create `crates/kunkka-core/tests/worker_idle.rs`: idle cleanup tests.
- Modify `docs/worker.md`, `docs/architecture.md`, and `docs/development-log.md`: document implemented worker dispatch slice after code is complete.

## Task 1: App Manifest Loader

**Files:**

- Modify: `crates/kunkka-core/Cargo.toml`
- Modify: `crates/kunkka-core/src/lib.rs`
- Modify: `crates/kunkka-core/src/error.rs`
- Create: `crates/kunkka-core/src/app_manifest.rs`
- Create: `crates/kunkka-core/tests/app_manifest.rs`

- [ ] **Step 1: Add failing manifest loader tests**

Create `crates/kunkka-core/tests/app_manifest.rs`:

```rust
use kunkka_core::app_manifest::{AppManifest, AppRegistry, DEFAULT_IDLE_TIMEOUT_MS, DEFAULT_STARTUP_TIMEOUT_MS};
use kunkka_core::xdg::KunkkaPaths;
use kunkka_core::CoreError;
use std::fs;
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

fn write_manifest(paths: &KunkkaPaths, name: &str, body: &str) {
    let apps_dir = paths.config_dir.join("apps");
    fs::create_dir_all(&apps_dir).unwrap();
    fs::write(apps_dir.join(name), body).unwrap();
}

#[test]
fn loads_app_manifest_from_xdg_config_apps_dir() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": ["--serve"],
                "env": { "NOTES_ENV": "local" },
                "cwd": "/home/example"
            },
            "idle_timeout_ms": 1234,
            "startup_timeout_ms": 5678
        }"#,
    );

    let registry = AppRegistry::load(&paths).unwrap();
    let manifest = registry.get("notes").unwrap();

    assert_eq!(manifest.app_id.as_str(), "notes");
    assert_eq!(manifest.worker.program, "/usr/bin/notes-worker");
    assert_eq!(manifest.worker.args, vec!["--serve"]);
    assert_eq!(manifest.worker.env.get("NOTES_ENV").unwrap(), "local");
    assert_eq!(manifest.worker.cwd.as_deref(), Some(std::path::Path::new("/home/example")));
    assert_eq!(manifest.idle_timeout_ms, 1234);
    assert_eq!(manifest.startup_timeout_ms, 5678);
}

#[test]
fn uses_default_timeouts_when_manifest_omits_timeout_fields() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": []
            }
        }"#,
    );

    let registry = AppRegistry::load(&paths).unwrap();
    let manifest = registry.get("notes").unwrap();

    assert_eq!(manifest.idle_timeout_ms, DEFAULT_IDLE_TIMEOUT_MS);
    assert_eq!(manifest.startup_timeout_ms, DEFAULT_STARTUP_TIMEOUT_MS);
}

#[test]
fn missing_apps_dir_loads_empty_registry() {
    let (_root, paths) = test_paths();

    let registry = AppRegistry::load(&paths).unwrap();

    assert!(registry.get("notes").is_none());
    assert!(registry.is_empty());
}

#[test]
fn rejects_manifest_missing_worker_program() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "args": []
            }
        }"#,
    );

    let err = AppRegistry::load(&paths).unwrap_err();

    assert!(matches!(
        err,
        CoreError::ManifestInvalid(message) if message.contains("worker.program")
    ));
}

#[test]
fn rejects_invalid_manifest_json() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, "notes.json", "not json");

    let err = AppRegistry::load(&paths).unwrap_err();

    assert!(matches!(
        err,
        CoreError::ManifestInvalid(message) if message.contains("notes.json")
    ));
}

#[test]
fn loads_manifest_file_directly() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": []
            }
        }"#,
    );

    let manifest = AppManifest::load_file(paths.config_dir.join("apps/notes.json")).unwrap();

    assert_eq!(manifest.app_id.as_str(), "notes");
}
```

- [ ] **Step 2: Run manifest tests to verify RED**

Run:

```bash
cargo test -p kunkka-core --test app_manifest
```

Expected: FAIL with unresolved import `kunkka_core::app_manifest`.

- [ ] **Step 3: Add serde dependencies and manifest errors**

Update `crates/kunkka-core/Cargo.toml`:

```toml
[dependencies]
kunkka-ipc = { path = "../kunkka-ipc" }
kunkka-protocol = { path = "../kunkka-protocol" }
kunkka-worker-sdk = { path = "../kunkka-worker-sdk" }
libc.workspace = true
serde.workspace = true
serde_json.workspace = true
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

    #[error("app not found: {0}")]
    AppNotFound(String),

    #[error("manifest invalid: {0}")]
    ManifestInvalid(String),

    #[error("worker start failed: {0}")]
    WorkerStartFailed(String),

    #[error("worker start timeout: {0}")]
    WorkerStartTimeout(String),

    #[error("worker unavailable: {0}")]
    WorkerUnavailable(String),

    #[error("dispatch ipc error: {0}")]
    DispatchIpcError(String),

    #[error("unexpected worker response: {0}")]
    UnexpectedWorkerResponse(String),
}
```

- [ ] **Step 4: Implement manifest loader**

Create `crates/kunkka-core/src/app_manifest.rs`:

```rust
use crate::xdg::KunkkaPaths;
use crate::{CoreError, Result};
use kunkka_worker_sdk::AppId;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_IDLE_TIMEOUT_MS: u64 = 300_000;
pub const DEFAULT_STARTUP_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AppManifest {
    pub app_id: AppId,
    pub worker: WorkerCommand,
    #[serde(default = "default_idle_timeout_ms")]
    pub idle_timeout_ms: u64,
    #[serde(default = "default_startup_timeout_ms")]
    pub startup_timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WorkerCommand {
    pub program: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct AppRegistry {
    manifests: BTreeMap<AppId, AppManifest>,
}

impl AppManifest {
    pub fn load_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path)?;
        let manifest: Self = serde_json::from_slice(&bytes).map_err(|err| {
            CoreError::ManifestInvalid(format!("{}: {err}", path.display()))
        })?;
        manifest.validate(path)?;
        Ok(manifest)
    }

    fn validate(&self, path: &Path) -> Result<()> {
        if self.app_id.as_str().is_empty() {
            return Err(CoreError::ManifestInvalid(format!(
                "{}: app_id is required",
                path.display()
            )));
        }

        if self.worker.program.is_empty() {
            return Err(CoreError::ManifestInvalid(format!(
                "{}: worker.program is required",
                path.display()
            )));
        }

        Ok(())
    }
}

impl AppRegistry {
    pub fn load(paths: &KunkkaPaths) -> Result<Self> {
        let apps_dir = paths.config_dir.join("apps");
        if !apps_dir.exists() {
            return Ok(Self::default());
        }

        let mut manifests = BTreeMap::new();
        for entry in fs::read_dir(&apps_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let manifest = AppManifest::load_file(&path)?;
            manifests.insert(manifest.app_id.clone(), manifest);
        }

        Ok(Self { manifests })
    }

    pub fn get(&self, app_id: &str) -> Option<&AppManifest> {
        self.manifests.get(&AppId::new(app_id))
    }

    pub fn get_app(&self, app_id: &AppId) -> Option<&AppManifest> {
        self.manifests.get(app_id)
    }

    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }
}

fn default_idle_timeout_ms() -> u64 {
    DEFAULT_IDLE_TIMEOUT_MS
}

fn default_startup_timeout_ms() -> u64 {
    DEFAULT_STARTUP_TIMEOUT_MS
}
```

Update `crates/kunkka-core/src/lib.rs`:

```rust
pub mod app_manifest;
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

- [ ] **Step 5: Run manifest tests to verify GREEN**

Run:

```bash
cargo test -p kunkka-core --test app_manifest
```

Expected: PASS with 6 tests.

- [ ] **Step 6: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 7: Commit manifest loader**

Run:

```bash
git add Cargo.lock crates/kunkka-core/Cargo.toml crates/kunkka-core/src/lib.rs crates/kunkka-core/src/error.rs crates/kunkka-core/src/app_manifest.rs crates/kunkka-core/tests/app_manifest.rs
git commit -m "feat: load app worker manifests"
```

## Task 2: Worker Dispatch Protocol Messages

**Files:**

- Modify: `crates/kunkka-worker-sdk/src/types.rs`
- Modify: `crates/kunkka-worker-sdk/src/lib.rs`
- Modify: `crates/kunkka-worker-sdk/tests/registration_codec.rs`

- [ ] **Step 1: Add failing protocol codec tests**

Append to `crates/kunkka-worker-sdk/tests/registration_codec.rs`:

```rust
use kunkka_ipc::{FrameMetadata, Payload};
use kunkka_worker_sdk::{
    decode_worker_message, encode_worker_message, AppId, DispatchWorkerRequest,
    DispatchWorkerResponse, WorkerAppError, WorkerProtocolMessage,
};

fn payload(bytes: &[u8]) -> Payload {
    Payload {
        bytes: bytes.to_vec(),
        content_type: Some("application/json".to_string()),
        schema: Some("example.notes.v1".to_string()),
        metadata: FrameMetadata::new(),
    }
}

#[test]
fn dispatch_worker_request_roundtrips_through_payload() {
    let message = WorkerProtocolMessage::DispatchWorker(DispatchWorkerRequest {
        app_id: AppId::new("notes"),
        method: "search".to_string(),
        payload: payload(br#"{"query":"kunkka"}"#),
    });

    let encoded = encode_worker_message(&message).unwrap();
    let decoded = decode_worker_message(&encoded).unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn dispatch_worker_success_response_roundtrips_through_payload() {
    let message = WorkerProtocolMessage::DispatchWorkerResult(DispatchWorkerResponse::Ok(
        payload(br#"{"items":[]}"#),
    ));

    let encoded = encode_worker_message(&message).unwrap();
    let decoded = decode_worker_message(&encoded).unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn dispatch_worker_error_response_roundtrips_through_payload() {
    let message = WorkerProtocolMessage::DispatchWorkerResult(DispatchWorkerResponse::Err(
        WorkerAppError {
            code: "not_found".to_string(),
            message: "note missing".to_string(),
        },
    ));

    let encoded = encode_worker_message(&message).unwrap();
    let decoded = decode_worker_message(&encoded).unwrap();

    assert_eq!(decoded, message);
}
```

Merge these imports into the existing `registration_codec.rs` import lists so each symbol is imported once.

- [ ] **Step 2: Run codec tests to verify RED**

Run:

```bash
cargo test -p kunkka-worker-sdk --test registration_codec
```

Expected: FAIL with unresolved imports for `DispatchWorkerRequest`, `DispatchWorkerResponse`, and `WorkerAppError`.

- [ ] **Step 3: Add dispatch protocol types**

Update `crates/kunkka-worker-sdk/src/types.rs`:

```rust
use kunkka_ipc::Payload;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkerId(String);

impl WorkerId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AppId(String);

impl AppId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerCapability {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterWorkerRequest {
    pub worker_id: WorkerId,
    pub app_id: AppId,
    pub capabilities: Vec<WorkerCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterWorkerResponse {
    pub worker_id: WorkerId,
    pub accepted: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchWorkerRequest {
    pub app_id: AppId,
    pub method: String,
    pub payload: Payload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerAppError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchWorkerResponse {
    Ok(Payload),
    Err(WorkerAppError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerProtocolMessage {
    RegisterWorker(RegisterWorkerRequest),
    RegisterWorkerAccepted(RegisterWorkerResponse),
    DispatchWorker(DispatchWorkerRequest),
    DispatchWorkerResult(DispatchWorkerResponse),
}
```

Update `crates/kunkka-worker-sdk/src/lib.rs` exports:

```rust
pub mod client;
pub mod codec;
pub mod error;
pub mod types;

pub use client::WorkerClient;
pub use codec::{
    decode_worker_message, encode_worker_message, WORKER_PROTOCOL_CONTENT_TYPE,
    WORKER_PROTOCOL_SCHEMA,
};
pub use error::{Result, WorkerSdkError};
pub use kunkka_ipc as ipc;
pub use types::{
    AppId, DispatchWorkerRequest, DispatchWorkerResponse, RegisterWorkerRequest,
    RegisterWorkerResponse, WorkerAppError, WorkerCapability, WorkerId, WorkerProtocolMessage,
};
```

- [ ] **Step 4: Run codec tests to verify GREEN**

Run:

```bash
cargo test -p kunkka-worker-sdk --test registration_codec
```

Expected: PASS with existing registration codec tests plus 3 dispatch tests.

- [ ] **Step 5: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 6: Commit worker protocol dispatch messages**

Run:

```bash
git add crates/kunkka-worker-sdk/src/types.rs crates/kunkka-worker-sdk/src/lib.rs crates/kunkka-worker-sdk/tests/registration_codec.rs
git commit -m "feat: add worker dispatch protocol"
```

## Task 3: Worker SDK Dispatch Helpers

**Files:**

- Modify: `crates/kunkka-worker-sdk/src/client.rs`
- Modify: `crates/kunkka-worker-sdk/src/lib.rs`
- Modify: `crates/kunkka-worker-sdk/Cargo.toml`
- Create: `crates/kunkka-worker-sdk/tests/dispatch_client.rs`

- [ ] **Step 1: Add failing dispatch helper tests**

Create `crates/kunkka-worker-sdk/tests/dispatch_client.rs`:

```rust
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, IpcListener, Payload, RequestId, SessionId};
use kunkka_worker_sdk::{
    decode_worker_message, encode_worker_message, AppId, DispatchWorkerRequest,
    DispatchWorkerResponse, WorkerAppError, WorkerClient, WorkerId, WorkerProtocolMessage,
};
use tempfile::{tempdir, TempDir};

fn socket_path() -> (TempDir, std::path::PathBuf) {
    let root = tempdir().unwrap();
    (root, root.path().join("worker.sock"))
}

fn payload(bytes: &[u8]) -> Payload {
    Payload {
        bytes: bytes.to_vec(),
        content_type: Some("application/json".to_string()),
        schema: Some("example.notes.v1".to_string()),
        metadata: FrameMetadata::new(),
    }
}

#[tokio::test]
async fn worker_client_receives_dispatch_request_and_sends_success_response() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let server_task = tokio::spawn(async move {
        let mut connection = listener.accept().await.unwrap();
        let request = DispatchWorkerRequest {
            app_id: AppId::new("notes"),
            method: "search".to_string(),
            payload: payload(br#"{"query":"kunkka"}"#),
        };
        let frame = Frame::Request {
            request_id: RequestId(10),
            session_id: SessionId(20),
            source: EndpointId::new("core"),
            target: EndpointId::new("worker:notes"),
            payload: encode_worker_message(&WorkerProtocolMessage::DispatchWorker(request)).unwrap(),
            metadata: FrameMetadata::new(),
        };

        connection.send_frame(&frame).await.unwrap();
        connection.recv_frame().await.unwrap().unwrap()
    });

    let connection = IpcConnection::connect(&socket_path).await.unwrap();
    let mut client = WorkerClient::from_connection(connection, WorkerId::new("notes"), SessionId(20));
    let request = client.recv_dispatch().await.unwrap();

    assert_eq!(request.request.app_id.as_str(), "notes");
    assert_eq!(request.request.method, "search");

    client
        .respond_dispatch(
            request,
            DispatchWorkerResponse::Ok(payload(br#"{"items":[]}"#)),
        )
        .await
        .unwrap();

    let response_frame = server_task.await.unwrap();
    let Frame::Response { request_id, payload, .. } = response_frame else {
        panic!("expected response frame");
    };

    assert_eq!(request_id, RequestId(10));
    assert_eq!(
        decode_worker_message(&payload).unwrap(),
        WorkerProtocolMessage::DispatchWorkerResult(DispatchWorkerResponse::Ok(payload(br#"{"items":[]}"#)))
    );
}

#[tokio::test]
async fn worker_client_sends_app_error_response() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let server_task = tokio::spawn(async move {
        let mut connection = listener.accept().await.unwrap();
        let request = DispatchWorkerRequest {
            app_id: AppId::new("notes"),
            method: "missing".to_string(),
            payload: payload(b"{}"),
        };
        let frame = Frame::Request {
            request_id: RequestId(11),
            session_id: SessionId(21),
            source: EndpointId::new("core"),
            target: EndpointId::new("worker:notes"),
            payload: encode_worker_message(&WorkerProtocolMessage::DispatchWorker(request)).unwrap(),
            metadata: FrameMetadata::new(),
        };

        connection.send_frame(&frame).await.unwrap();
        connection.recv_frame().await.unwrap().unwrap()
    });

    let connection = IpcConnection::connect(&socket_path).await.unwrap();
    let mut client = WorkerClient::from_connection(connection, WorkerId::new("notes"), SessionId(21));
    let request = client.recv_dispatch().await.unwrap();

    client
        .respond_dispatch(
            request,
            DispatchWorkerResponse::Err(WorkerAppError {
                code: "not_found".to_string(),
                message: "missing note".to_string(),
            }),
        )
        .await
        .unwrap();

    let response_frame = server_task.await.unwrap();
    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };

    assert_eq!(
        decode_worker_message(&payload).unwrap(),
        WorkerProtocolMessage::DispatchWorkerResult(DispatchWorkerResponse::Err(WorkerAppError {
            code: "not_found".to_string(),
            message: "missing note".to_string(),
        }))
    );
}
```

- [ ] **Step 2: Run helper tests to verify RED**

Run:

```bash
cargo test -p kunkka-worker-sdk --test dispatch_client
```

Expected: FAIL with methods `recv_dispatch` and `respond_dispatch` not found.

- [ ] **Step 3: Add test dependencies**

Update `crates/kunkka-worker-sdk/Cargo.toml`:

```toml
[package]
name = "kunkka-worker-sdk"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[dependencies]
kunkka-ipc = { path = "../kunkka-ipc" }
postcard.workspace = true
serde.workspace = true
thiserror.workspace = true

[dev-dependencies]
tempfile.workspace = true
tokio.workspace = true
```

- [ ] **Step 4: Implement WorkerClient dispatch helpers**

Update `crates/kunkka-worker-sdk/src/client.rs`:

```rust
use crate::{
    decode_worker_message, encode_worker_message, DispatchWorkerRequest,
    DispatchWorkerResponse, RegisterWorkerRequest, RegisterWorkerResponse, Result, WorkerId,
    WorkerProtocolMessage, WorkerSdkError,
};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, RequestId, SessionId};
use std::path::Path;

pub struct WorkerClient {
    connection: IpcConnection,
    worker_endpoint: EndpointId,
    core_endpoint: EndpointId,
    session_id: SessionId,
    next_request_id: u128,
}

#[derive(Debug)]
pub struct DispatchRequestContext {
    pub request_id: RequestId,
    pub session_id: SessionId,
    pub source: EndpointId,
    pub target: EndpointId,
    pub request: DispatchWorkerRequest,
}

impl WorkerClient {
    pub async fn connect(path: impl AsRef<Path>, worker_id: WorkerId) -> Result<Self> {
        let connection = IpcConnection::connect(path).await?;
        Ok(Self::from_connection(connection, worker_id, SessionId(1)))
    }

    pub fn from_connection(
        connection: IpcConnection,
        worker_id: WorkerId,
        session_id: SessionId,
    ) -> Self {
        Self {
            connection,
            worker_endpoint: EndpointId::new(format!("worker:{}", worker_id.as_str())),
            core_endpoint: EndpointId::new("core"),
            session_id,
            next_request_id: 1,
        }
    }

    pub async fn register(
        &mut self,
        request: RegisterWorkerRequest,
    ) -> Result<RegisterWorkerResponse> {
        let request_id = self.next_request_id();
        let payload = encode_worker_message(&WorkerProtocolMessage::RegisterWorker(request))?;

        let frame = Frame::Request {
            request_id,
            session_id: self.session_id,
            source: self.worker_endpoint.clone(),
            target: self.core_endpoint.clone(),
            payload,
            metadata: FrameMetadata::new(),
        };

        self.connection.send_frame(&frame).await?;

        let response = self
            .connection
            .recv_frame()
            .await?
            .ok_or(kunkka_ipc::IpcError::ConnectionClosed)?;

        let Frame::Response {
            request_id: response_request_id,
            payload,
            ..
        } = response
        else {
            return Err(WorkerSdkError::Protocol(
                "expected registration response frame".to_string(),
            ));
        };

        if response_request_id != request_id {
            return Err(WorkerSdkError::Protocol(format!(
                "response request_id mismatch: expected {}, got {}",
                request_id.0, response_request_id.0
            )));
        }

        let message = decode_worker_message(&payload)?;

        match message {
            WorkerProtocolMessage::RegisterWorkerAccepted(response) => Ok(response),
            other => Err(WorkerSdkError::Protocol(format!(
                "expected RegisterWorkerAccepted, got {other:?}"
            ))),
        }
    }

    pub async fn recv_dispatch(&mut self) -> Result<DispatchRequestContext> {
        let frame = self
            .connection
            .recv_frame()
            .await?
            .ok_or(kunkka_ipc::IpcError::ConnectionClosed)?;

        let Frame::Request {
            request_id,
            session_id,
            source,
            target,
            payload,
            ..
        } = frame
        else {
            return Err(WorkerSdkError::Protocol(
                "expected dispatch request frame".to_string(),
            ));
        };

        let message = decode_worker_message(&payload)?;
        let WorkerProtocolMessage::DispatchWorker(request) = message else {
            return Err(WorkerSdkError::Protocol(
                "expected DispatchWorker request".to_string(),
            ));
        };

        Ok(DispatchRequestContext {
            request_id,
            session_id,
            source,
            target,
            request,
        })
    }

    pub async fn respond_dispatch(
        &mut self,
        context: DispatchRequestContext,
        response: DispatchWorkerResponse,
    ) -> Result<()> {
        let payload = encode_worker_message(&WorkerProtocolMessage::DispatchWorkerResult(response))?;
        let frame = Frame::Response {
            request_id: context.request_id,
            session_id: context.session_id,
            source: context.target,
            target: context.source,
            payload,
            metadata: FrameMetadata::new(),
        };

        self.connection.send_frame(&frame).await?;
        Ok(())
    }

    fn next_request_id(&mut self) -> RequestId {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        request_id
    }
}
```

Update `crates/kunkka-worker-sdk/src/lib.rs` to export `DispatchRequestContext`:

```rust
pub use client::{DispatchRequestContext, WorkerClient};
```

Keep the existing module/export lines around this change.

- [ ] **Step 5: Run helper tests to verify GREEN**

Run:

```bash
cargo test -p kunkka-worker-sdk --test dispatch_client
```

Expected: PASS with 2 tests.

- [ ] **Step 6: Run worker SDK tests**

Run:

```bash
cargo test -p kunkka-worker-sdk
```

Expected: PASS.

- [ ] **Step 7: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 8: Commit SDK helpers**

Run:

```bash
git add Cargo.lock crates/kunkka-worker-sdk/Cargo.toml crates/kunkka-worker-sdk/src/client.rs crates/kunkka-worker-sdk/src/lib.rs crates/kunkka-worker-sdk/tests/dispatch_client.rs
git commit -m "feat: add worker dispatch client helpers"
```

## Task 4: Worker Registry Replaces by AppId

**Files:**

- Modify: `crates/kunkka-core/src/worker_registry.rs`
- Modify: `crates/kunkka-core/tests/worker_registry.rs`

- [ ] **Step 1: Add failing registry replacement tests**

Append to `crates/kunkka-core/tests/worker_registry.rs`:

```rust
#[test]
fn duplicate_app_id_replaces_existing_worker() {
    let mut registry = WorkerRegistry::new();

    registry.register(request("worker-1", "notes", "notes.search"));
    registry.register(request("worker-2", "notes", "notes.write"));

    assert_eq!(registry.len(), 1);
    assert!(registry.get(&WorkerId::new("worker-1")).is_none());

    let registered = registry.get(&WorkerId::new("worker-2")).unwrap();
    assert_eq!(registered.app_id.as_str(), "notes");
    assert_eq!(registered.capabilities[0].name, "notes.write");

    let by_app = registry.get_by_app_id(&AppId::new("notes")).unwrap();
    assert_eq!(by_app.worker_id.as_str(), "worker-2");
}
```

- [ ] **Step 2: Run registry tests to verify RED**

Run:

```bash
cargo test -p kunkka-core --test worker_registry
```

Expected: FAIL with method `get_by_app_id` not found, or `worker-1` still present after duplicate `AppId` registration.

- [ ] **Step 3: Implement registry keyed by WorkerId and AppId**

Update `crates/kunkka-core/src/worker_registry.rs`:

```rust
use crate::{CoreError, Result};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata};
use kunkka_worker_sdk::{
    decode_worker_message, encode_worker_message, AppId, RegisterWorkerRequest,
    RegisterWorkerResponse, WorkerCapability, WorkerId, WorkerProtocolMessage,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredWorker {
    pub worker_id: WorkerId,
    pub app_id: AppId,
    pub capabilities: Vec<WorkerCapability>,
}

#[derive(Debug, Default)]
pub struct WorkerRegistry {
    workers: BTreeMap<WorkerId, RegisteredWorker>,
    app_workers: BTreeMap<AppId, WorkerId>,
}

impl WorkerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, request: RegisterWorkerRequest) -> RegisterWorkerResponse {
        let worker_id = request.worker_id.clone();
        let app_id = request.app_id.clone();

        if let Some(old_worker_id) = self.app_workers.insert(app_id.clone(), worker_id.clone()) {
            if old_worker_id != worker_id {
                self.workers.remove(&old_worker_id);
            }
        }

        let registered = RegisteredWorker {
            worker_id: request.worker_id,
            app_id: request.app_id,
            capabilities: request.capabilities,
        };

        self.workers.insert(worker_id.clone(), registered);

        RegisterWorkerResponse {
            worker_id,
            accepted: true,
            message: None,
        }
    }

    pub fn remove(&mut self, worker_id: &WorkerId) -> Option<RegisteredWorker> {
        let registered = self.workers.remove(worker_id)?;
        self.app_workers.remove(&registered.app_id);
        Some(registered)
    }

    pub fn remove_by_app_id(&mut self, app_id: &AppId) -> Option<RegisteredWorker> {
        let worker_id = self.app_workers.remove(app_id)?;
        self.workers.remove(&worker_id)
    }

    pub fn get(&self, worker_id: &WorkerId) -> Option<&RegisteredWorker> {
        self.workers.get(worker_id)
    }

    pub fn get_by_app_id(&self, app_id: &AppId) -> Option<&RegisteredWorker> {
        let worker_id = self.app_workers.get(app_id)?;
        self.workers.get(worker_id)
    }

    pub fn len(&self) -> usize {
        self.workers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.workers.is_empty()
    }
}

pub fn handle_worker_registration_frame(
    registry: &mut WorkerRegistry,
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
        return Err(CoreError::InvalidWorkerFrame(
            "expected request frame".to_string(),
        ));
    };

    let message = decode_worker_message(&payload)?;

    let WorkerProtocolMessage::RegisterWorker(request) = message else {
        return Err(CoreError::InvalidWorkerFrame(
            "expected worker registration request".to_string(),
        ));
    };

    let response = registry.register(request);
    let payload = encode_worker_message(&WorkerProtocolMessage::RegisterWorkerAccepted(response))?;

    Ok(Frame::Response {
        request_id,
        session_id,
        source: target_or_core(target),
        target: source,
        payload,
        metadata: FrameMetadata::new(),
    })
}

fn target_or_core(target: EndpointId) -> EndpointId {
    if target.as_str().is_empty() {
        EndpointId::new("core")
    } else {
        target
    }
}
```

- [ ] **Step 4: Run registry tests to verify GREEN**

Run:

```bash
cargo test -p kunkka-core --test worker_registry
```

Expected: PASS.

- [ ] **Step 5: Run affected core tests**

Run:

```bash
cargo test -p kunkka-core --test worker_registration
cargo test -p kunkka-core --test core_runtime_loop
cargo test -p kunkka-core --test core_runtime_control
```

Expected: PASS.

- [ ] **Step 6: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 7: Commit registry replacement**

Run:

```bash
git add crates/kunkka-core/src/worker_registry.rs crates/kunkka-core/tests/worker_registry.rs
git commit -m "feat: replace active worker by app id"
```

## Task 5: Warm-Path Worker Dispatch Manager

**Files:**

- Modify: `crates/kunkka-core/src/lib.rs`
- Create: `crates/kunkka-core/src/worker_dispatch.rs`
- Create: `crates/kunkka-core/tests/worker_dispatch_warm.rs`

- [ ] **Step 1: Add failing warm dispatch tests**

Create `crates/kunkka-core/tests/worker_dispatch_warm.rs`:

```rust
use kunkka_core::worker_dispatch::{DispatchResult, WorkerManager};
use kunkka_ipc::{Frame, FrameMetadata, IpcConnection, IpcListener, Payload, RequestId};
use kunkka_worker_sdk::{
    encode_worker_message, AppId, DispatchWorkerResponse, RegisterWorkerRequest,
    WorkerCapability, WorkerClient, WorkerId, WorkerProtocolMessage,
};
use tempfile::{tempdir, TempDir};

fn socket_path() -> (TempDir, std::path::PathBuf) {
    let root = tempdir().unwrap();
    (root, root.path().join("worker.sock"))
}

fn payload(bytes: &[u8]) -> Payload {
    Payload {
        bytes: bytes.to_vec(),
        content_type: Some("application/json".to_string()),
        schema: Some("example.notes.v1".to_string()),
        metadata: FrameMetadata::new(),
    }
}

fn registration() -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new("notes"),
        app_id: AppId::new("notes"),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: None,
        }],
    }
}

#[tokio::test]
async fn dispatch_sends_request_to_active_worker_and_returns_payload() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let connection = IpcConnection::connect(&socket_path).await.unwrap();
            let mut worker = WorkerClient::from_connection(connection, WorkerId::new("notes"), kunkka_ipc::SessionId(1));
            let request = worker.recv_dispatch().await.unwrap();
            assert_eq!(request.request.app_id.as_str(), "notes");
            assert_eq!(request.request.method, "search");
            worker
                .respond_dispatch(
                    request,
                    DispatchWorkerResponse::Ok(payload(br#"{"items":[]}"#)),
                )
                .await
                .unwrap();
        }
    });

    let core_connection = listener.accept().await.unwrap();
    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 300_000);

    let result = manager
        .dispatch(AppId::new("notes"), "search".to_string(), payload(br#"{"query":"kunkka"}"#))
        .await
        .unwrap();

    assert_eq!(result, DispatchResult::Ok(payload(br#"{"items":[]}"#)));
    worker_task.await.unwrap();
}

#[tokio::test]
async fn dispatch_returns_worker_app_error_without_removing_worker() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let connection = IpcConnection::connect(&socket_path).await.unwrap();
            let mut worker = WorkerClient::from_connection(connection, WorkerId::new("notes"), kunkka_ipc::SessionId(1));
            let request = worker.recv_dispatch().await.unwrap();
            worker
                .respond_dispatch(
                    request,
                    DispatchWorkerResponse::Err(kunkka_worker_sdk::WorkerAppError {
                        code: "not_found".to_string(),
                        message: "missing note".to_string(),
                    }),
                )
                .await
                .unwrap();
        }
    });

    let core_connection = listener.accept().await.unwrap();
    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 300_000);

    let result = manager
        .dispatch(AppId::new("notes"), "missing".to_string(), payload(b"{}"))
        .await
        .unwrap();

    assert_eq!(
        result,
        DispatchResult::AppError {
            code: "not_found".to_string(),
            message: "missing note".to_string(),
        }
    );
    assert!(manager.is_active(&AppId::new("notes")));
    worker_task.await.unwrap();
}

#[tokio::test]
async fn dispatch_ipc_failure_removes_active_worker() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let _connection = IpcConnection::connect(&socket_path).await.unwrap();
        }
    });

    let core_connection = listener.accept().await.unwrap();
    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 300_000);
    worker_task.await.unwrap();

    let err = manager
        .dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}"))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("dispatch ipc error"));
    assert!(!manager.is_active(&AppId::new("notes")));
}

#[tokio::test]
async fn dispatch_request_id_mismatch_removes_active_worker() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            let frame = connection.recv_frame().await.unwrap().unwrap();
            let Frame::Request {
                session_id,
                source,
                target,
                ..
            } = frame
            else {
                panic!("expected dispatch request frame");
            };
            let response = Frame::Response {
                request_id: RequestId(999),
                session_id,
                source: target,
                target: source,
                payload: encode_worker_message(&WorkerProtocolMessage::DispatchWorkerResult(
                    DispatchWorkerResponse::Ok(payload(br#"{"items":[]}"#)),
                ))
                .unwrap(),
                metadata: FrameMetadata::new(),
            };
            connection.send_frame(&response).await.unwrap();
        }
    });

    let core_connection = listener.accept().await.unwrap();
    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 300_000);

    let err = manager
        .dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}"))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("response request_id mismatch"));
    assert!(!manager.is_active(&AppId::new("notes")));
    worker_task.await.unwrap();
}
```

- [ ] **Step 2: Run warm dispatch tests to verify RED**

Run:

```bash
cargo test -p kunkka-core --test worker_dispatch_warm
```

Expected: FAIL with unresolved import `kunkka_core::worker_dispatch`.

- [ ] **Step 3: Implement warm-path WorkerManager**

Create `crates/kunkka-core/src/worker_dispatch.rs`:

```rust
use crate::worker_registry::WorkerRegistry;
use crate::{CoreError, Result};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_worker_sdk::{
    decode_worker_message, encode_worker_message, AppId, DispatchWorkerRequest,
    DispatchWorkerResponse, RegisterWorkerRequest, WorkerId, WorkerProtocolMessage,
};
use std::collections::BTreeMap;
use std::process::Child;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchResult {
    Ok(Payload),
    AppError { code: String, message: String },
}

pub struct WorkerManager {
    registry: WorkerRegistry,
    active_workers: BTreeMap<AppId, ActiveWorker>,
    next_request_id: u128,
}

struct ActiveWorker {
    worker_id: WorkerId,
    connection: IpcConnection,
    child: Option<Child>,
    last_used_at: Instant,
    idle_timeout: Duration,
}

impl WorkerManager {
    pub fn new_empty() -> Self {
        Self {
            registry: WorkerRegistry::new(),
            active_workers: BTreeMap::new(),
            next_request_id: 1,
        }
    }

    pub fn registry(&self) -> &WorkerRegistry {
        &self.registry
    }

    pub fn is_active(&self, app_id: &AppId) -> bool {
        self.active_workers.contains_key(app_id)
    }

    pub fn active_worker_count(&self) -> usize {
        self.active_workers.len()
    }

    pub fn register_active_for_test(
        &mut self,
        request: RegisterWorkerRequest,
        connection: IpcConnection,
        idle_timeout_ms: u64,
    ) {
        self.insert_active_worker(request, connection, None, idle_timeout_ms);
    }

    pub fn insert_active_worker(
        &mut self,
        request: RegisterWorkerRequest,
        connection: IpcConnection,
        child: Option<Child>,
        idle_timeout_ms: u64,
    ) {
        let app_id = request.app_id.clone();
        let worker_id = request.worker_id.clone();
        self.registry.register(request);

        if let Some(mut old_worker) = self.active_workers.remove(&app_id) {
            old_worker.terminate();
        }

        self.active_workers.insert(
            app_id,
            ActiveWorker {
                worker_id,
                connection,
                child,
                last_used_at: Instant::now(),
                idle_timeout: Duration::from_millis(idle_timeout_ms),
            },
        );
    }

    pub async fn dispatch(
        &mut self,
        app_id: AppId,
        method: String,
        payload: Payload,
    ) -> Result<DispatchResult> {
        if method.is_empty() {
            return Err(CoreError::UnexpectedWorkerResponse(
                "dispatch method is empty".to_string(),
            ));
        }

        let request_id = self.next_request_id();
        let active = self.active_workers.get_mut(&app_id).ok_or_else(|| {
            CoreError::WorkerUnavailable(format!("no active worker for app {}", app_id.as_str()))
        })?;

        let request = DispatchWorkerRequest {
            app_id: app_id.clone(),
            method,
            payload,
        };
        let frame_payload = encode_worker_message(&WorkerProtocolMessage::DispatchWorker(request))?;
        let frame = Frame::Request {
            request_id,
            session_id: SessionId(1),
            source: EndpointId::new("core"),
            target: EndpointId::new(format!("worker:{}", active.worker_id.as_str())),
            payload: frame_payload,
            metadata: FrameMetadata::new(),
        };

        let result = send_dispatch_frame(active, frame, request_id).await;
        match result {
            Ok(result) => {
                active.last_used_at = Instant::now();
                Ok(result)
            }
            Err(err) => {
                if let Some(mut removed) = self.active_workers.remove(&app_id) {
                    removed.terminate();
                }
                self.registry.remove_by_app_id(&app_id);
                Err(err)
            }
        }
    }

    fn next_request_id(&mut self) -> RequestId {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        request_id
    }
}

impl ActiveWorker {
    fn terminate(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

async fn send_dispatch_frame(
    active: &mut ActiveWorker,
    frame: Frame,
    request_id: RequestId,
) -> Result<DispatchResult> {
    active
        .connection
        .send_frame(&frame)
        .await
        .map_err(|err| CoreError::DispatchIpcError(err.to_string()))?;

    let response = active
        .connection
        .recv_frame()
        .await
        .map_err(|err| CoreError::DispatchIpcError(err.to_string()))?
        .ok_or_else(|| CoreError::DispatchIpcError("worker closed connection".to_string()))?;

    let Frame::Response {
        request_id: response_request_id,
        payload,
        ..
    } = response
    else {
        return Err(CoreError::UnexpectedWorkerResponse(
            "expected response frame".to_string(),
        ));
    };

    if response_request_id != request_id {
        return Err(CoreError::UnexpectedWorkerResponse(format!(
            "response request_id mismatch: expected {}, got {}",
            request_id.0, response_request_id.0
        )));
    }

    match decode_worker_message(&payload)? {
        WorkerProtocolMessage::DispatchWorkerResult(DispatchWorkerResponse::Ok(payload)) => {
            Ok(DispatchResult::Ok(payload))
        }
        WorkerProtocolMessage::DispatchWorkerResult(DispatchWorkerResponse::Err(err)) => {
            Ok(DispatchResult::AppError {
                code: err.code,
                message: err.message,
            })
        }
        other => Err(CoreError::UnexpectedWorkerResponse(format!(
            "expected DispatchWorkerResult, got {other:?}"
        ))),
    }
}
```

Update `crates/kunkka-core/src/lib.rs`:

```rust
pub mod app_manifest;
pub mod error;
pub mod ipc_server;
pub mod runtime;
pub mod worker_dispatch;
pub mod worker_registry;
pub mod xdg;
```

Keep the rest of `lib.rs` unchanged.

- [ ] **Step 4: Run warm dispatch tests to verify GREEN**

Run:

```bash
cargo test -p kunkka-core --test worker_dispatch_warm
```

Expected: PASS with 4 tests.

- [ ] **Step 5: Run core tests affected by exports and registry**

Run:

```bash
cargo test -p kunkka-core
```

Expected: PASS.

- [ ] **Step 6: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 7: Commit warm dispatch manager**

Run:

```bash
git add crates/kunkka-core/src/lib.rs crates/kunkka-core/src/worker_dispatch.rs crates/kunkka-core/tests/worker_dispatch_warm.rs
git commit -m "feat: dispatch to active worker"
```

## Task 6: Runtime Registration Handoff

**Files:**

- Modify: `crates/kunkka-core/src/runtime.rs`
- Modify: `crates/kunkka-core/src/worker_dispatch.rs`
- Modify: `crates/kunkka-core/tests/core_runtime_loop.rs`
- Create: `crates/kunkka-core/tests/worker_runtime_registration.rs`

- [ ] **Step 1: Add failing runtime registration handoff test**

Create `crates/kunkka-core/tests/worker_runtime_registration.rs`:

```rust
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
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

fn request(worker_id: &str, app_id: &str) -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new(worker_id),
        app_id: AppId::new(app_id),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: None,
        }],
    }
}

#[tokio::test]
async fn runtime_hands_registered_worker_connection_to_worker_manager() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let register_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("notes"))
                .await
                .unwrap();
            client.register(request("notes", "notes")).await.unwrap()
        }
    });

    runtime.run_once().await.unwrap();
    let response = register_task.await.unwrap();

    assert!(response.accepted);
    assert!(runtime.registry().get_by_app_id(&AppId::new("notes")).is_some());
    assert!(runtime.worker_manager().is_active(&AppId::new("notes")));
}

#[tokio::test]
async fn duplicate_app_registration_replaces_runtime_active_worker() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    for worker_id in ["worker-1", "worker-2"] {
        let register_task = tokio::spawn({
            let socket_path = paths.socket_path.clone();
            async move {
                let mut client = WorkerClient::connect(&socket_path, WorkerId::new(worker_id))
                    .await
                    .unwrap();
                client.register(request(worker_id, "notes")).await.unwrap()
            }
        });
        runtime.run_once().await.unwrap();
        assert!(register_task.await.unwrap().accepted);
    }

    assert_eq!(runtime.registry().len(), 1);
    let registered = runtime.registry().get_by_app_id(&AppId::new("notes")).unwrap();
    assert_eq!(registered.worker_id.as_str(), "worker-2");
    assert_eq!(runtime.worker_manager().active_worker_count(), 1);
}
```

- [ ] **Step 2: Run handoff tests to verify RED**

Run:

```bash
cargo test -p kunkka-core --test worker_runtime_registration
```

Expected: FAIL with method `worker_manager` not found or active worker not retained.

- [ ] **Step 3: Add WorkerManager registration handoff method**

In `crates/kunkka-core/src/worker_dispatch.rs`, add these imports to the existing import list:

```rust
use kunkka_worker_sdk::RegisterWorkerResponse;
```

Add this method inside `impl WorkerManager`:

```rust
pub async fn handle_registration_connection(
    &mut self,
    frame: Frame,
    mut connection: IpcConnection,
    mut child: Option<Child>,
    idle_timeout_ms: u64,
) -> Result<()> {
    let result = async {
        let Frame::Request {
            request_id,
            session_id,
            source,
            target,
            payload,
            ..
        } = frame
        else {
            return Err(CoreError::InvalidWorkerFrame(
                "expected request frame".to_string(),
            ));
        };

        let message = decode_worker_message(&payload)?;
        let WorkerProtocolMessage::RegisterWorker(request) = message else {
            return Err(CoreError::InvalidWorkerFrame(
                "expected worker registration request".to_string(),
            ));
        };

        let worker_id = request.worker_id.clone();
        let response = RegisterWorkerResponse {
            worker_id,
            accepted: true,
            message: None,
        };
        let response_payload = encode_worker_message(&WorkerProtocolMessage::RegisterWorkerAccepted(response))?;
        let response_frame = Frame::Response {
            request_id,
            session_id,
            source: target_or_core(target),
            target: source,
            payload: response_payload,
            metadata: FrameMetadata::new(),
        };

        connection.send_frame(&response_frame).await?;
        Ok::<_, CoreError>((request, connection))
    }
    .await;

    match result {
        Ok((request, connection)) => {
            self.insert_active_worker(request, connection, child.take(), idle_timeout_ms);
            Ok(())
        }
        Err(err) => {
            if let Some(child) = child.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
            Err(err)
        }
    }
}
```

Add this helper at the bottom of `worker_dispatch.rs`:

```rust
fn target_or_core(target: EndpointId) -> EndpointId {
    if target.as_str().is_empty() {
        EndpointId::new("core")
    } else {
        target
    }
}
```

The method above needs `CoreError` in scope for `Ok::<_, CoreError>`.

- [ ] **Step 4: Refactor runtime to own WorkerManager and hand off worker connections**

Update `crates/kunkka-core/src/runtime.rs`:

```rust
use crate::app_manifest::AppRegistry;
use crate::ipc_server::CoreIpcServer;
use crate::worker_dispatch::WorkerManager;
use crate::worker_registry::WorkerRegistry;
use crate::xdg::KunkkaPaths;
use crate::{CoreError, Result};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingResponse,
    CoreStatusResponse, CORE_CONTROL_SCHEMA,
};
use kunkka_worker_sdk::WORKER_PROTOCOL_SCHEMA;

pub struct CoreRuntime {
    server: CoreIpcServer,
    worker_manager: WorkerManager,
}

impl CoreRuntime {
    pub async fn prepare(paths: &KunkkaPaths) -> Result<Self> {
        paths.ensure_dirs()?;
        let server = CoreIpcServer::bind(paths).await?;
        let app_registry = AppRegistry::load(paths)?;

        Ok(Self {
            server,
            worker_manager: WorkerManager::with_app_registry(
                app_registry,
                paths.socket_path.clone(),
            ),
        })
    }

    pub fn registry(&self) -> &WorkerRegistry {
        self.worker_manager.registry()
    }

    pub fn worker_manager(&self) -> &WorkerManager {
        &self.worker_manager
    }

    pub async fn run_once(&mut self) -> Result<()> {
        let mut connection = self.server.accept_one().await?;
        let Some(first_frame) = connection.recv_frame().await? else {
            return Ok(());
        };

        match frame_schema(&first_frame) {
            Some(WORKER_PROTOCOL_SCHEMA) => {
                self.worker_manager
                    .handle_registration_connection(first_frame, connection, None, 300_000)
                    .await
            }
            Some(CORE_CONTROL_SCHEMA) => self.run_control_connection(connection, first_frame).await,
            Some(schema) => Err(CoreError::InvalidCoreFrame(format!(
                "unknown payload schema: {schema}"
            ))),
            None => Err(CoreError::InvalidCoreFrame(
                "missing payload schema".to_string(),
            )),
        }
    }

    pub async fn run(mut self) -> Result<()> {
        loop {
            self.run_once().await?;
        }
    }

    async fn run_control_connection(
        &mut self,
        mut connection: IpcConnection,
        first_frame: Frame,
    ) -> Result<()> {
        let response = self.handle_control_frame(first_frame)?;
        connection.send_frame(&response).await?;

        while let Some(frame) = connection.recv_frame().await? {
            let response = self.handle_control_frame(frame)?;
            connection.send_frame(&response).await?;
        }

        Ok(())
    }

    fn handle_control_frame(&self, frame: Frame) -> Result<Frame> {
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

        let response_message = match decode_control_message(&payload)? {
            CoreControlMessage::Ping(_) => CoreControlMessage::Pong(CorePingResponse),
            CoreControlMessage::Status(_) => CoreControlMessage::StatusResult(CoreStatusResponse {
                worker_count: self.registry().len() as u64,
                socket_path: self.server.socket_path().to_string_lossy().into_owned(),
                runtime_ready: true,
            }),
            _ => {
                return Err(CoreError::InvalidCoreFrame(
                    "expected core control request".to_string(),
                ));
            }
        };

        let payload = encode_control_message(&response_message)?;

        Ok(Frame::Response {
            request_id,
            session_id,
            source: target_or_core(target),
            target: source,
            payload,
            metadata: FrameMetadata::new(),
        })
    }
}

fn frame_schema(frame: &Frame) -> Option<&str> {
    match frame {
        Frame::Request { payload, .. }
        | Frame::Response { payload, .. }
        | Frame::Event { payload, .. }
        | Frame::Stream { payload, .. } => payload.schema.as_deref(),
        Frame::Cancel { .. } | Frame::Heartbeat { .. } | Frame::Error { .. } => None,
    }
}

fn target_or_core(target: EndpointId) -> EndpointId {
    if target.as_str().is_empty() {
        EndpointId::new("core")
    } else {
        target
    }
}
```

Update the `WorkerManager` struct and add `WorkerManager::with_app_registry` in `worker_dispatch.rs`:

```rust
use crate::app_manifest::AppRegistry;
use std::path::PathBuf;

pub struct WorkerManager {
    registry: WorkerRegistry,
    app_registry: AppRegistry,
    socket_path: PathBuf,
    active_workers: BTreeMap<AppId, ActiveWorker>,
    next_request_id: u128,
}

impl WorkerManager {
    pub fn new_empty() -> Self {
        Self {
            registry: WorkerRegistry::new(),
            app_registry: AppRegistry::default(),
            socket_path: PathBuf::new(),
            active_workers: BTreeMap::new(),
            next_request_id: 1,
        }
    }

    pub fn with_app_registry(app_registry: AppRegistry, socket_path: PathBuf) -> Self {
        Self {
            registry: WorkerRegistry::new(),
            app_registry,
            socket_path,
            active_workers: BTreeMap::new(),
            next_request_id: 1,
        }
    }
}
```

- [ ] **Step 5: Update core runtime loop test expectation**

In `crates/kunkka-core/tests/core_runtime_loop.rs`, keep the existing assertion:

```rust
assert!(runtime.registry().get(&WorkerId::new("worker-1")).is_some());
```

Add this assertion after it:

```rust
assert!(runtime.worker_manager().is_active(&AppId::new("example-app")));
```

- [ ] **Step 6: Run runtime registration tests to verify GREEN**

Run:

```bash
cargo test -p kunkka-core --test worker_runtime_registration
cargo test -p kunkka-core --test core_runtime_loop
cargo test -p kunkka-core --test core_runtime_control
```

Expected: PASS.

- [ ] **Step 7: Run core tests**

Run:

```bash
cargo test -p kunkka-core
```

Expected: PASS.

- [ ] **Step 8: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 9: Commit runtime registration handoff**

Run:

```bash
git add crates/kunkka-core/src/runtime.rs crates/kunkka-core/src/worker_dispatch.rs crates/kunkka-core/tests/core_runtime_loop.rs crates/kunkka-core/tests/worker_runtime_registration.rs
git commit -m "feat: retain registered worker connections"
```

## Task 7: Cold Dispatch Process Startup

**Files:**

- Modify: `crates/kunkka-core/src/runtime.rs`
- Modify: `crates/kunkka-core/src/worker_dispatch.rs`
- Create: `crates/kunkka-core/tests/worker_dispatch_cold.rs`

- [ ] **Step 1: Add failing cold dispatch tests with self-hosted worker fixture**

Create `crates/kunkka-core/tests/worker_dispatch_cold.rs`:

```rust
use kunkka_core::worker_dispatch::DispatchResult;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_core::{prepare_core_runtime, CoreError};
use kunkka_ipc::{FrameMetadata, Payload};
use kunkka_worker_sdk::{
    AppId, DispatchWorkerResponse, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId,
};
use std::fs;
use std::time::Duration;
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

fn payload(bytes: &[u8]) -> Payload {
    Payload {
        bytes: bytes.to_vec(),
        content_type: Some("application/json".to_string()),
        schema: Some("example.notes.v1".to_string()),
        metadata: FrameMetadata::new(),
    }
}

fn write_manifest(paths: &KunkkaPaths, body: &str) {
    let apps_dir = paths.config_dir.join("apps");
    fs::create_dir_all(&apps_dir).unwrap();
    fs::write(apps_dir.join("notes.json"), body).unwrap();
}

fn worker_fixture_manifest(mode: &str, startup_timeout_ms: u64) -> String {
    let current_exe = std::env::current_exe().unwrap();
    format!(
        r#"{{
            "app_id": "notes",
            "worker": {{
                "program": {},
                "args": ["worker_fixture_entrypoint", "--exact", "--nocapture"],
                "env": {{ "KUNKKA_WORKER_FIXTURE": {} }}
            }},
            "idle_timeout_ms": 300000,
            "startup_timeout_ms": {}
        }}"#,
        serde_json::to_string(current_exe.to_str().unwrap()).unwrap(),
        serde_json::to_string(mode).unwrap(),
        startup_timeout_ms
    )
}

#[test]
fn worker_fixture_entrypoint() {
    let Ok(mode) = std::env::var("KUNKKA_WORKER_FIXTURE") else {
        return;
    };

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async move {
        if mode == "never-register" {
            tokio::time::sleep(Duration::from_millis(500)).await;
            return;
        }

        let socket_path = std::env::var("KUNKKA_CORE_SOCKET").unwrap();
        let app_id = std::env::var("KUNKKA_APP_ID").unwrap();
        let worker_id = std::env::var("KUNKKA_WORKER_ID").unwrap();
        let mut client = WorkerClient::connect(&socket_path, WorkerId::new(worker_id.clone()))
            .await
            .unwrap();
        client
            .register(RegisterWorkerRequest {
                worker_id: WorkerId::new(worker_id),
                app_id: AppId::new(app_id),
                capabilities: vec![WorkerCapability {
                    name: "notes.search".to_string(),
                    description: None,
                }],
            })
            .await
            .unwrap();

        let request = client.recv_dispatch().await.unwrap();
        if mode == "app-error" {
            client
                .respond_dispatch(
                    request,
                    DispatchWorkerResponse::Err(kunkka_worker_sdk::WorkerAppError {
                        code: "not_found".to_string(),
                        message: "missing note".to_string(),
                    }),
                )
                .await
                .unwrap();
        } else {
            client
                .respond_dispatch(request, DispatchWorkerResponse::Ok(payload(br#"{"items":[]}"#)))
                .await
                .unwrap();
        }
    });
}

#[tokio::test]
async fn cold_dispatch_starts_worker_and_returns_payload() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, &worker_fixture_manifest("ok", 5000));
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let result = runtime
        .dispatch(AppId::new("notes"), "search".to_string(), payload(br#"{"query":"kunkka"}"#))
        .await
        .unwrap();

    assert_eq!(result, DispatchResult::Ok(payload(br#"{"items":[]}"#)));
    assert!(runtime.worker_manager().is_active(&AppId::new("notes")));
}

#[tokio::test]
async fn cold_dispatch_returns_worker_app_error() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, &worker_fixture_manifest("app-error", 5000));
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let result = runtime
        .dispatch(AppId::new("notes"), "missing".to_string(), payload(b"{}"))
        .await
        .unwrap();

    assert_eq!(
        result,
        DispatchResult::AppError {
            code: "not_found".to_string(),
            message: "missing note".to_string(),
        }
    );
}

#[tokio::test]
async fn missing_manifest_returns_app_not_found() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let err = runtime
        .dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}"))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CoreError::AppNotFound(message) if message.contains("notes")
    ));
}

#[tokio::test]
async fn invalid_worker_executable_returns_start_failed() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/path/that/does/not/exist/kunkka-worker",
                "args": []
            },
            "idle_timeout_ms": 300000,
            "startup_timeout_ms": 5000
        }"#,
    );
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let err = runtime
        .dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}"))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CoreError::WorkerStartFailed(message) if message.contains("notes")
    ));
    assert!(!runtime.worker_manager().is_active(&AppId::new("notes")));
}

#[tokio::test]
async fn worker_that_never_registers_returns_start_timeout() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, &worker_fixture_manifest("never-register", 50));
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let err = runtime
        .dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}"))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CoreError::WorkerStartTimeout(message) if message.contains("notes")
    ));
    assert!(!runtime.worker_manager().is_active(&AppId::new("notes")));
}
```

- [ ] **Step 2: Run cold dispatch tests to verify RED**

Run:

```bash
cargo test -p kunkka-core --test worker_dispatch_cold
```

Expected: FAIL with method `dispatch` not found on `CoreRuntime`, or cold start returning `WorkerUnavailable` because no active worker exists.

- [ ] **Step 3: Add CoreRuntime dispatch API**

In `crates/kunkka-core/src/runtime.rs`, add imports:

```rust
use crate::worker_dispatch::DispatchResult;
use kunkka_ipc::Payload;
use kunkka_worker_sdk::AppId;
```

Add this method inside `impl CoreRuntime`:

```rust
pub async fn dispatch(
    &mut self,
    app_id: AppId,
    method: String,
    payload: Payload,
) -> Result<DispatchResult> {
    self.worker_manager
        .dispatch_with_start(&self.server, app_id, method, payload)
        .await
}
```

- [ ] **Step 4: Implement process startup in WorkerManager**

In `crates/kunkka-core/src/worker_dispatch.rs`, add imports:

```rust
use crate::app_manifest::{AppManifest, AppRegistry};
use crate::ipc_server::CoreIpcServer;
use std::process::Command;
use tokio::time::timeout;
```

Add these methods inside `impl WorkerManager`:

```rust
pub async fn dispatch_with_start(
    &mut self,
    server: &CoreIpcServer,
    app_id: AppId,
    method: String,
    payload: Payload,
) -> Result<DispatchResult> {
    if !self.is_active(&app_id) {
        self.start_and_wait_for_registration(server, &app_id).await?;
    }

    self.dispatch(app_id, method, payload).await
}

async fn start_and_wait_for_registration(
    &mut self,
    server: &CoreIpcServer,
    app_id: &AppId,
) -> Result<()> {
    let manifest = self
        .app_registry
        .get_app(app_id)
        .cloned()
        .ok_or_else(|| CoreError::AppNotFound(app_id.as_str().to_string()))?;

    let mut child = spawn_worker(&manifest, &self.socket_path)?;
    let startup_timeout = Duration::from_millis(manifest.startup_timeout_ms);
    let registration = timeout(startup_timeout, async {
        let mut connection = server.accept_one().await?;
        let frame = connection
            .recv_frame()
            .await?
            .ok_or_else(|| CoreError::WorkerUnavailable("worker closed before registration".to_string()))?;
        Ok::<_, CoreError>((frame, connection))
    })
    .await;

    match registration {
        Ok(Ok((frame, connection))) => {
            self.handle_registration_connection(
                frame,
                connection,
                Some(child),
                manifest.idle_timeout_ms,
            )
            .await
        }
        Ok(Err(err)) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(err)
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(CoreError::WorkerStartTimeout(format!(
                "worker for app {} did not register within {} ms",
                app_id.as_str(),
                manifest.startup_timeout_ms
            )))
        }
    }
}
```

Add this helper:

```rust
fn spawn_worker(manifest: &AppManifest, socket_path: &std::path::Path) -> Result<Child> {
    let mut command = Command::new(&manifest.worker.program);
    command.args(&manifest.worker.args);
    command.env("KUNKKA_CORE_SOCKET", socket_path.as_os_str());
    command.env("KUNKKA_APP_ID", manifest.app_id.as_str());
    command.env("KUNKKA_WORKER_ID", manifest.app_id.as_str());
    for (key, value) in &manifest.worker.env {
        command.env(key, value);
    }
    if let Some(cwd) = &manifest.worker.cwd {
        command.current_dir(cwd);
    }

    command.spawn().map_err(|err| {
        CoreError::WorkerStartFailed(format!(
            "failed to start worker for app {}: {err}",
            manifest.app_id.as_str()
        ))
    })
}
```

- [ ] **Step 5: Run cold dispatch tests to verify GREEN**

Run:

```bash
cargo test -p kunkka-core --test worker_dispatch_cold
```

Expected: PASS with 6 tests.

- [ ] **Step 6: Run core tests**

Run:

```bash
cargo test -p kunkka-core
```

Expected: PASS.

- [ ] **Step 7: Run formatting and clippy checks for touched crates**

Run:

```bash
cargo fmt --all --check
cargo clippy -p kunkka-core --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 8: Commit cold dispatch startup**

Run:

```bash
git add crates/kunkka-core/src/runtime.rs crates/kunkka-core/src/worker_dispatch.rs crates/kunkka-core/tests/worker_dispatch_cold.rs
git commit -m "feat: start workers for cold dispatch"
```

## Task 8: Idle Worker Cleanup

**Files:**

- Modify: `crates/kunkka-core/src/worker_dispatch.rs`
- Modify: `crates/kunkka-core/src/runtime.rs`
- Modify: `crates/kunkka-core/tests/worker_dispatch_cold.rs`
- Create: `crates/kunkka-core/tests/worker_idle.rs`

- [ ] **Step 1: Add failing idle cleanup tests**

Create `crates/kunkka-core/tests/worker_idle.rs`:

```rust
use kunkka_core::worker_dispatch::WorkerManager;
use kunkka_ipc::{IpcConnection, IpcListener};
use kunkka_worker_sdk::{AppId, RegisterWorkerRequest, WorkerCapability, WorkerId};
use tempfile::{tempdir, TempDir};

fn socket_path() -> (TempDir, std::path::PathBuf) {
    let root = tempdir().unwrap();
    (root, root.path().join("worker.sock"))
}

fn registration() -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new("notes"),
        app_id: AppId::new("notes"),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: None,
        }],
    }
}

#[tokio::test]
async fn reap_idle_workers_removes_expired_active_worker() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let _connection = IpcConnection::connect(&socket_path).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });
    let core_connection = listener.accept().await.unwrap();

    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 1);
    assert!(manager.is_active(&AppId::new("notes")));

    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    manager.reap_idle_workers();

    assert!(!manager.is_active(&AppId::new("notes")));
    assert_eq!(manager.active_worker_count(), 0);
    worker_task.await.unwrap();
}

#[tokio::test]
async fn reap_idle_workers_keeps_recent_active_worker() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let _connection = IpcConnection::connect(&socket_path).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    });
    let core_connection = listener.accept().await.unwrap();

    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 60_000);
    manager.reap_idle_workers();

    assert!(manager.is_active(&AppId::new("notes")));
    worker_task.await.unwrap();
}
```

- [ ] **Step 2: Add idle restart regression to cold dispatch test**

Update `worker_fixture_manifest` in `crates/kunkka-core/tests/worker_dispatch_cold.rs` to accept idle timeout:

```rust
fn worker_fixture_manifest(mode: &str, idle_timeout_ms: u64, startup_timeout_ms: u64) -> String {
    let current_exe = std::env::current_exe().unwrap();
    format!(
        r#"{{
            "app_id": "notes",
            "worker": {{
                "program": {},
                "args": ["worker_fixture_entrypoint", "--exact", "--nocapture"],
                "env": {{ "KUNKKA_WORKER_FIXTURE": {} }}
            }},
            "idle_timeout_ms": {},
            "startup_timeout_ms": {}
        }}"#,
        serde_json::to_string(current_exe.to_str().unwrap()).unwrap(),
        serde_json::to_string(mode).unwrap(),
        idle_timeout_ms,
        startup_timeout_ms
    )
}
```

Update existing calls in `worker_dispatch_cold.rs`:

```rust
worker_fixture_manifest("ok", 300_000, 5000)
worker_fixture_manifest("app-error", 300_000, 5000)
worker_fixture_manifest("never-register", 300_000, 50)
```

Append this test to `crates/kunkka-core/tests/worker_dispatch_cold.rs`:

```rust
#[tokio::test]
async fn dispatch_after_idle_cleanup_restarts_worker() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, &worker_fixture_manifest("ok", 1, 5000));
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let first = runtime
        .dispatch(AppId::new("notes"), "search".to_string(), payload(br#"{"query":"first"}"#))
        .await
        .unwrap();
    assert_eq!(first, DispatchResult::Ok(payload(br#"{"items":[]}"#)));

    tokio::time::sleep(Duration::from_millis(5)).await;
    runtime.reap_idle_workers();
    assert!(!runtime.worker_manager().is_active(&AppId::new("notes")));

    let second = runtime
        .dispatch(AppId::new("notes"), "search".to_string(), payload(br#"{"query":"second"}"#))
        .await
        .unwrap();
    assert_eq!(second, DispatchResult::Ok(payload(br#"{"items":[]}"#)));
    assert!(runtime.worker_manager().is_active(&AppId::new("notes")));
}
```

- [ ] **Step 3: Run idle tests to verify RED**

Run:

```bash
cargo test -p kunkka-core --test worker_idle
cargo test -p kunkka-core --test worker_dispatch_cold dispatch_after_idle_cleanup_restarts_worker
```

Expected: FAIL with method `reap_idle_workers` not found.

- [ ] **Step 4: Implement idle cleanup**

Add this method inside `impl WorkerManager` in `crates/kunkka-core/src/worker_dispatch.rs`:

```rust
pub fn reap_idle_workers(&mut self) {
    let now = Instant::now();
    let expired: Vec<AppId> = self
        .active_workers
        .iter()
        .filter_map(|(app_id, worker)| {
            if now.duration_since(worker.last_used_at) >= worker.idle_timeout {
                Some(app_id.clone())
            } else {
                None
            }
        })
        .collect();

    for app_id in expired {
        if let Some(mut worker) = self.active_workers.remove(&app_id) {
            worker.terminate();
        }
        self.registry.remove_by_app_id(&app_id);
    }
}
```

Add this method inside `impl CoreRuntime` in `crates/kunkka-core/src/runtime.rs`:

```rust
pub fn reap_idle_workers(&mut self) {
    self.worker_manager.reap_idle_workers();
}
```

- [ ] **Step 5: Run idle tests to verify GREEN**

Run:

```bash
cargo test -p kunkka-core --test worker_idle
cargo test -p kunkka-core --test worker_dispatch_cold dispatch_after_idle_cleanup_restarts_worker
```

Expected: PASS with 2 worker_idle tests and 1 cold restart test.

- [ ] **Step 6: Run core tests**

Run:

```bash
cargo test -p kunkka-core
```

Expected: PASS.

- [ ] **Step 7: Run formatting and clippy checks**

Run:

```bash
cargo fmt --all --check
cargo clippy -p kunkka-core --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 8: Commit idle cleanup**

Run:

```bash
git add crates/kunkka-core/src/runtime.rs crates/kunkka-core/src/worker_dispatch.rs crates/kunkka-core/tests/worker_dispatch_cold.rs crates/kunkka-core/tests/worker_idle.rs
git commit -m "feat: reap idle workers"
```

## Task 9: Documentation and Full Verification

**Files:**

- Modify: `README.md`
- Modify: `docs/architecture.md`
- Modify: `docs/worker.md`
- Modify: `docs/development-log.md`

- [ ] **Step 1: Update README implemented slices**

Update `README.md` implemented slices to mention worker dispatch:

```markdown
- `kunkka-core`：XDG path resolution、private runtime directory setup、minimal core IPC socket binding、in-memory worker registration、single-connection worker registration runtime loop、core control protocol、XDG app manifest loading、按需 worker startup、core-internal worker dispatch，以及 idle worker cleanup。
- `kunkka-worker-sdk`：共享 worker registration/dispatch protocol、typed payload codec、registration client 和 dispatch receive/respond helpers。
```

- [ ] **Step 2: Update architecture current implementation section**

Update `docs/architecture.md` current implementation slice:

```markdown
- `kunkka-core`：XDG path management、runtime socket setup、single-connection runtime loop、in-memory worker registry、core control protocol、XDG app manifest registry、worker lifecycle manager、core-internal dispatch API。
- `kunkka-worker-sdk`：worker registration/dispatch protocol、payload codec、registration and dispatch helpers。
```

Also update the later sentence so `worker request dispatch` is no longer listed as a future slice, while frontend dispatch entrypoints and permission checks remain future work.

- [ ] **Step 3: Update worker docs**

Update `docs/worker.md` with these sections:

```markdown
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
```

Keep the existing “尚未实现” list, but remove items completed by this plan: `worker process spawning`, `request dispatch to worker`, and basic idle lifecycle. Keep `heartbeat loop`, `worker lifecycle restart policy`, `worker streams`, `worker cancellation`, `SQLite persistence`, and `permission checks`.

- [ ] **Step 4: Update development log**

Append under the current date in `docs/development-log.md`:

```markdown
### Worker Dispatch

Implemented:

- XDG JSON app manifest loading from `config/apps/*.json`。
- Worker dispatch protocol in `kunkka-worker-sdk`。
- Core active worker registry keyed by `AppId`。
- Runtime worker registration connection handoff。
- Core-internal warm and cold worker dispatch。
- On-demand worker process startup with `KUNKKA_CORE_SOCKET`, `KUNKKA_APP_ID`, and `KUNKKA_WORKER_ID`。
- Idle worker cleanup。

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
```

- [ ] **Step 5: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 6: Run workspace tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 7: Run workspace clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 8: Check git status and diff**

Run:

```bash
git status --short
git diff --check
git diff --stat
```

Expected: only intended documentation changes before commit, no whitespace errors.

- [ ] **Step 9: Commit docs**

Run:

```bash
git add README.md docs/architecture.md docs/worker.md docs/development-log.md
git commit -m "docs: document worker dispatch"
```

- [ ] **Step 10: Final verification after docs commit**

Run:

```bash
cargo test --workspace
git status --short
```

Expected: workspace tests PASS and clean worktree.

## Final Review Checklist

Before marking this plan complete, verify:

- `kunkka-ipc` has no app/worker dispatch semantics added.
- `kunkka-native-host` has no app worker dispatch entrypoint added.
- `kunkka-core` dispatch API is internal only.
- App manifest loading uses `KunkkaPaths.config_dir` and does not write runtime state.
- Worker startup uses env vars `KUNKKA_CORE_SOCKET`, `KUNKKA_APP_ID`, and `KUNKKA_WORKER_ID`.
- First version keeps `WorkerId == AppId` for spawned workers.
- Active worker map is keyed by `AppId` and duplicate registration replaces the old active worker.
- Same active worker has only one in-flight dispatch through exclusive mutable access.
- Worker app errors do not remove the active worker.
- IPC/protocol failures remove the active worker.
- Idle cleanup removes expired workers.
- `cargo fmt --all --check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` pass.
