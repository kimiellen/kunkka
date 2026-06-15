# CLI Frontend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `crates/kunkka-cli` as a CLI frontend that supports `ping`, `status`, and `dispatch` commands via Kunkka IPC.

**Architecture:** CLI uses `kunkka-core::xdg::KunkkaPaths` to find the core socket, then directly uses `kunkka-protocol` typed messages over UDS. Output is JSON. CLI does not auto-start core, read manifests, make permission decisions, or talk to workers directly.

**Tech Stack:** Rust, clap, tokio, serde, serde_json, kunkka-ipc, kunkka-protocol, kunkka-core

---

### Task 1: Workspace setup and crate skeleton

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/kunkka-cli/Cargo.toml`
- Create: `crates/kunkka-cli/src/lib.rs`
- Create: `crates/kunkka-cli/src/main.rs`

- [ ] **Step 1: Add clap to workspace dependencies**

Add to `Cargo.toml` `[workspace.dependencies]` section:

```toml
clap = { version = "4", features = ["derive"] }
```

Add `crates/kunkka-cli` to workspace members:

```toml
members = [
  "crates/kunkka-ipc",
  "crates/kunkka-protocol",
  "crates/kunkka-core",
  "crates/kunkka-worker-sdk",
  "crates/kunkka-native-host",
  "crates/kunkka-cli",
]
```

- [ ] **Step 2: Create crate Cargo.toml**

Create `crates/kunkka-cli/Cargo.toml`:

```toml
[package]
name = "kunkka-cli"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[dependencies]
clap.workspace = true
kunkka-ipc = { path = "../kunkka-ipc" }
kunkka-protocol = { path = "../kunkka-protocol" }
kunkka-core = { path = "../kunkka-core" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 3: Create minimal lib.rs**

Create `crates/kunkka-cli/src/lib.rs`:

```rust
pub mod cli;
pub mod client;
pub mod error;
pub mod output;
```

- [ ] **Step 4: Create minimal main.rs**

Create `crates/kunkka-cli/src/main.rs`:

```rust
fn main() {
    eprintln!("kunkka-cli: not yet implemented");
    std::process::exit(1);
}
```

- [ ] **Step 5: Verify crate compiles**

Run: `cargo check -p kunkka-cli`
Expected: FAIL (modules `cli`, `client`, `error`, `output` don't exist yet)

- [ ] **Step 6: Create stub modules**

Create `crates/kunkka-cli/src/cli.rs`:

```rust
```

Create `crates/kunkka-cli/src/client.rs`:

```rust
```

Create `crates/kunkka-cli/src/error.rs`:

```rust
```

Create `crates/kunkka-cli/src/output.rs`:

```rust
```

- [ ] **Step 7: Verify crate compiles**

Run: `cargo check -p kunkka-cli`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/kunkka-cli/
git commit -m "feat: initialize kunkka-cli crate skeleton"
```

---

### Task 2: Output types

**Files:**
- Modify: `crates/kunkka-cli/src/output.rs`
- Test: `crates/kunkka-cli/tests/output.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/kunkka-cli/tests/output.rs`:

```rust
use kunkka_cli::output::{CliErrorBody, CliOutput, CliResult};
use serde_json::json;

#[test]
fn pong_output_serializes_to_json() {
    let output = CliOutput {
        ok: true,
        result: Some(CliResult::Pong),
        error: None,
    };
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(value, json!({"ok":true,"result":{"type":"pong"}}));
}

#[test]
fn status_output_serializes_to_json() {
    let output = CliOutput {
        ok: true,
        result: Some(CliResult::Status {
            worker_count: 2,
            socket_path: "/tmp/core.sock".to_string(),
            runtime_ready: true,
        }),
        error: None,
    };
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(
        value,
        json!({"ok":true,"result":{"type":"status","worker_count":2,"socket_path":"/tmp/core.sock","runtime_ready":true}})
    );
}

#[test]
fn dispatch_output_serializes_to_json() {
    let output = CliOutput {
        ok: true,
        result: Some(CliResult::Dispatch {
            payload: json!({"items": []}),
        }),
        error: None,
    };
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(
        value,
        json!({"ok":true,"result":{"type":"dispatch","payload":{"items":[]}}})
    );
}

#[test]
fn dispatch_error_output_serializes_to_json() {
    let output = CliOutput {
        ok: true,
        result: Some(CliResult::DispatchError {
            code: "not_found".to_string(),
            message: "note not found".to_string(),
        }),
        error: None,
    };
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(
        value,
        json!({"ok":true,"result":{"type":"dispatch_error","code":"not_found","message":"note not found"}})
    );
}

#[test]
fn error_output_serializes_to_json() {
    let output: CliOutput = CliOutput::error("permission_denied", "frontend dispatch method \"delete\" is not allowed");
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(
        value,
        json!({"ok":false,"error":{"code":"permission_denied","message":"frontend dispatch method \"delete\" is not allowed"}})
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kunkka-cli --test output`
Expected: FAIL with "module `output` is private" or similar

- [ ] **Step 3: Implement output.rs**

Create `crates/kunkka-cli/src/output.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliOutput {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<CliResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CliErrorBody>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CliResult {
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
pub struct CliErrorBody {
    pub code: String,
    pub message: String,
}

impl CliOutput {
    pub fn success(result: CliResult) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(CliErrorBody {
                code: code.into(),
                message: message.into(),
            }),
        }
    }

    pub fn is_success(&self) -> bool {
        self.ok
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p kunkka-cli --test output`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-cli/src/output.rs crates/kunkka-cli/tests/output.rs
git commit -m "feat: add kunkka-cli output types"
```

---

### Task 3: Error types

**Files:**
- Modify: `crates/kunkka-cli/src/error.rs`

- [ ] **Step 1: Implement error.rs**

Create `crates/kunkka-cli/src/error.rs`:

```rust
use crate::output::CliOutput;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("core unavailable: {0}")]
    CoreUnavailable(String),

    #[error("core ipc error: {0}")]
    CoreIpc(String),

    #[error("unexpected core response: {0}")]
    UnexpectedCoreResponse(String),

    #[error("core platform error: {code}: {message}")]
    CorePlatform { code: String, message: String },
}

impl CliError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::CoreUnavailable(_) => "core_unavailable",
            Self::CoreIpc(_) => "core_ipc_error",
            Self::UnexpectedCoreResponse(_) => "unexpected_core_response",
            Self::CorePlatform { .. } => "core_error",
        }
    }

    pub fn exit_code(&self) -> i32 {
        1
    }

    pub fn to_output(&self) -> CliOutput {
        match self {
            Self::CorePlatform { code, message } => CliOutput::error(code, message),
            _ => CliOutput::error(self.code(), self.to_string()),
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p kunkka-cli`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-cli/src/error.rs
git commit -m "feat: add kunkka-cli error types"
```

---

### Task 4: CLI args

**Files:**
- Modify: `crates/kunkka-cli/src/cli.rs`
- Test: `crates/kunkka-cli/tests/cli_args.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/kunkka-cli/tests/cli_args.rs`:

```rust
use clap::Parser;
use kunkka_cli::cli::{Cli, CliCommand};

#[test]
fn parses_ping_command() {
    let cli = Cli::try_parse_from(["kunkka", "ping"]).unwrap();
    assert!(matches!(cli.command, CliCommand::Ping));
}

#[test]
fn parses_status_command() {
    let cli = Cli::try_parse_from(["kunkka", "status"]).unwrap();
    assert!(matches!(cli.command, CliCommand::Status));
}

#[test]
fn parses_dispatch_command() {
    let cli = Cli::try_parse_from([
        "kunkka",
        "dispatch",
        "--app",
        "notes",
        "--method",
        "search",
        "--payload",
        r#"{"query":"kunkka"}"#,
    ])
    .unwrap();
    match cli.command {
        CliCommand::Dispatch {
            app_id,
            method,
            payload,
        } => {
            assert_eq!(app_id, "notes");
            assert_eq!(method, "search");
            assert_eq!(payload, serde_json::json!({"query": "kunkka"}));
        }
        _ => panic!("expected dispatch command"),
    }
}

#[test]
fn rejects_dispatch_missing_app() {
    let result = Cli::try_parse_from([
        "kunkka",
        "dispatch",
        "--method",
        "search",
        "--payload",
        "{}",
    ]);
    assert!(result.is_err());
}

#[test]
fn rejects_dispatch_missing_method() {
    let result = Cli::try_parse_from([
        "kunkka",
        "dispatch",
        "--app",
        "notes",
        "--payload",
        "{}",
    ]);
    assert!(result.is_err());
}

#[test]
fn rejects_dispatch_invalid_json_payload() {
    let result = Cli::try_parse_from([
        "kunkka",
        "dispatch",
        "--app",
        "notes",
        "--method",
        "search",
        "--payload",
        "not json",
    ]);
    assert!(result.is_err());
}

#[test]
fn rejects_dispatch_empty_app() {
    let result = Cli::try_parse_from([
        "kunkka",
        "dispatch",
        "--app",
        "",
        "--method",
        "search",
        "--payload",
        "{}",
    ]);
    assert!(result.is_err());
}

#[test]
fn rejects_dispatch_empty_method() {
    let result = Cli::try_parse_from([
        "kunkka",
        "dispatch",
        "--app",
        "notes",
        "--method",
        "",
        "--payload",
        "{}",
    ]);
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kunkka-cli --test cli_args`
Expected: FAIL with "module `cli` is private" or similar

- [ ] **Step 3: Implement cli.rs**

Create `crates/kunkka-cli/src/cli.rs`:

```rust
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "kunkka", version, about = "Kunkka CLI frontend")]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    Ping,
    Status,
    Dispatch {
        #[arg(long, value_parser = validate_non_empty)]
        app_id: String,
        #[arg(long, value_parser = validate_non_empty)]
        method: String,
        #[arg(long, value_parser = parse_json_payload)]
        payload: serde_json::Value,
    },
}

fn validate_non_empty(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        Err("must not be empty".to_string())
    } else {
        Ok(value.to_string())
    }
}

fn parse_json_payload(value: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(value).map_err(|err| format!("invalid JSON: {err}"))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p kunkka-cli --test cli_args`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-cli/src/cli.rs crates/kunkka-cli/tests/cli_args.rs
git commit -m "feat: add kunkka-cli clap args"
```

---

### Task 5: Client

**Files:**
- Modify: `crates/kunkka-cli/src/client.rs`
- Test: `crates/kunkka-cli/tests/client_mapping.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/kunkka-cli/tests/client_mapping.rs`:

```rust
use kunkka_cli::cli::CliCommand;
use kunkka_cli::client::core_message_for_command;
use kunkka_protocol::core_control::CoreControlMessage;

#[test]
fn ping_command_maps_to_core_ping() {
    let message = core_message_for_command(&CliCommand::Ping).unwrap();
    assert!(matches!(message, CoreControlMessage::Ping(_)));
}

#[test]
fn status_command_maps_to_core_status() {
    let message = core_message_for_command(&CliCommand::Status).unwrap();
    assert!(matches!(message, CoreControlMessage::Status(_)));
}

#[test]
fn dispatch_command_returns_none_for_control() {
    let result = core_message_for_command(&CliCommand::Dispatch {
        app_id: "notes".to_string(),
        method: "search".to_string(),
        payload: serde_json::json!({}),
    });
    assert!(result.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kunkka-cli --test client_mapping`
Expected: FAIL with "module `client` is private" or similar

- [ ] **Step 3: Implement client.rs**

Create `crates/kunkka-cli/src/client.rs`:

```rust
use crate::cli::CliCommand;
use crate::error::CliError;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingRequest,
    CorePingResponse, CoreStatusRequest,
};
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message, FrontendDispatchMessage,
    FrontendDispatchRequest, FrontendDispatchResponse,
};
use std::path::Path;

const JSON_CONTENT_TYPE: &str = "application/json";

pub fn core_message_for_command(command: &CliCommand) -> Option<CoreControlMessage> {
    match command {
        CliCommand::Ping => Some(CoreControlMessage::Ping(CorePingRequest)),
        CliCommand::Status => Some(CoreControlMessage::Status(CoreStatusRequest)),
        CliCommand::Dispatch { .. } => None,
    }
}

pub async fn send_core_control(
    socket_path: &Path,
    message: CoreControlMessage,
) -> Result<CoreControlMessage, CliError> {
    let mut connection = IpcConnection::connect(socket_path)
        .await
        .map_err(|err| CliError::CoreUnavailable(err.to_string()))?;

    let request_id = RequestId(1);
    let session_id = SessionId(1);
    let payload = encode_control_message(&message)
        .map_err(|err| CliError::CoreIpc(err.to_string()))?;
    let frame = Frame::Request {
        request_id,
        session_id,
        source: EndpointId::new("cli"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    connection
        .send_frame(&frame)
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?;

    let response = connection
        .recv_frame()
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?
        .ok_or_else(|| CliError::CoreIpc("core closed connection".to_string()))?;

    let Frame::Response {
        request_id: response_request_id,
        payload,
        ..
    } = response
    else {
        return Err(CliError::UnexpectedCoreResponse(
            "expected response frame".to_string(),
        ));
    };

    if response_request_id != request_id {
        return Err(CliError::UnexpectedCoreResponse(format!(
            "response request_id mismatch: expected {}, got {}",
            request_id.0, response_request_id.0
        )));
    }

    decode_control_message(&payload)
        .map_err(|err| CliError::CoreIpc(err.to_string()))
}

pub async fn send_frontend_dispatch(
    socket_path: &Path,
    request: FrontendDispatchRequest,
) -> Result<FrontendDispatchResponse, CliError> {
    let mut connection = IpcConnection::connect(socket_path)
        .await
        .map_err(|err| CliError::CoreUnavailable(err.to_string()))?;

    let request_id = RequestId(1);
    let session_id = SessionId(1);
    let payload = encode_frontend_dispatch_message(&FrontendDispatchMessage::Dispatch(request))
        .map_err(|err| CliError::CoreIpc(err.to_string()))?;
    let frame = Frame::Request {
        request_id,
        session_id,
        source: EndpointId::new("cli"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    connection
        .send_frame(&frame)
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?;

    let response = connection
        .recv_frame()
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?
        .ok_or_else(|| CliError::CoreIpc("core closed connection".to_string()))?;

    let Frame::Response {
        request_id: response_request_id,
        payload,
        ..
    } = response
    else {
        return Err(CliError::UnexpectedCoreResponse(
            "expected response frame".to_string(),
        ));
    };

    if response_request_id != request_id {
        return Err(CliError::UnexpectedCoreResponse(format!(
            "response request_id mismatch: expected {}, got {}",
            request_id.0, response_request_id.0
        )));
    }

    decode_frontend_dispatch_message(&payload)
        .map_err(|err| CliError::CoreIpc(err.to_string()))
}

pub fn build_frontend_dispatch_request(
    app_id: String,
    method: String,
    payload: serde_json::Value,
) -> FrontendDispatchRequest {
    FrontendDispatchRequest {
        app_id,
        method,
        payload: Payload {
            bytes: serde_json::to_vec(&payload).unwrap_or_default(),
            content_type: Some(JSON_CONTENT_TYPE.to_string()),
            schema: None,
            metadata: FrameMetadata::new(),
        },
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p kunkka-cli --test client_mapping`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-cli/src/client.rs crates/kunkka-cli/tests/client_mapping.rs
git commit -m "feat: add kunkka-cli client"
```

---

### Task 6: Main entry and lib public API

**Files:**
- Modify: `crates/kunkka-cli/src/lib.rs`
- Modify: `crates/kunkka-cli/src/main.rs`

- [ ] **Step 1: Update lib.rs with public API**

Create `crates/kunkka-cli/src/lib.rs`:

```rust
pub mod cli;
pub mod client;
pub mod error;
pub mod output;

use cli::{Cli, CliCommand};
use client::{build_frontend_dispatch_request, core_message_for_command, send_core_control, send_frontend_dispatch};
use error::CliError;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_protocol::frontend_dispatch::FrontendDispatchResponse;
use output::{CliOutput, CliResult};

pub async fn run_command(cli: &Cli) -> Result<CliOutput, CliError> {
    let paths = KunkkaPaths::resolve().map_err(|err| {
        CliError::CoreUnavailable(format!("failed to resolve kunkka paths: {err}"))
    })?;

    match &cli.command {
        CliCommand::Ping => {
            let message = core_message_for_command(&cli.command)
                .ok_or_else(|| CliError::InvalidRequest("expected control command".to_string()))?;
            let response = send_core_control(&paths.socket_path, message).await?;
            match response {
                kunkka_protocol::core_control::CoreControlMessage::Pong(_) => {
                    Ok(CliOutput::success(CliResult::Pong))
                }
                other => Err(CliError::UnexpectedCoreResponse(format!(
                    "expected pong, got {other:?}"
                ))),
            }
        }
        CliCommand::Status => {
            let message = core_message_for_command(&cli.command)
                .ok_or_else(|| CliError::InvalidRequest("expected control command".to_string()))?;
            let response = send_core_control(&paths.socket_path, message).await?;
            match response {
                kunkka_protocol::core_control::CoreControlMessage::StatusResult(status) => {
                    Ok(CliOutput::success(CliResult::Status {
                        worker_count: status.worker_count,
                        socket_path: status.socket_path,
                        runtime_ready: status.runtime_ready,
                    }))
                }
                other => Err(CliError::UnexpectedCoreResponse(format!(
                    "expected status result, got {other:?}"
                ))),
            }
        }
        CliCommand::Dispatch {
            app_id,
            method,
            payload,
        } => {
            let request = build_frontend_dispatch_request(
                app_id.clone(),
                method.clone(),
                payload.clone(),
            );
            let response = send_frontend_dispatch(&paths.socket_path, request).await?;
            match response {
                FrontendDispatchResponse::Ok(payload) => {
                    let value: serde_json::Value = serde_json::from_slice(&payload.bytes)
                        .map_err(|err| CliError::CoreIpc(format!("invalid JSON payload: {err}")))?;
                    Ok(CliOutput::success(CliResult::Dispatch { payload: value }))
                }
                FrontendDispatchResponse::AppError { code, message } => {
                    Ok(CliOutput::success(CliResult::DispatchError { code, message }))
                }
                FrontendDispatchResponse::PlatformError { code, message } => {
                    Err(CliError::CorePlatform { code, message })
                }
            }
        }
    }
}
```

- [ ] **Step 2: Implement main.rs**

Create `crates/kunkka-cli/src/main.rs`:

```rust
use clap::Parser;
use kunkka_cli::cli::Cli;
use kunkka_cli::run_command;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match run_command(&cli).await {
        Ok(output) => {
            println!("{}", serde_json::to_string(&output).unwrap());
            if output.is_success() {
                std::process::exit(0);
            } else {
                std::process::exit(1);
            }
        }
        Err(err) => {
            let output = err.to_output();
            eprintln!("{}", serde_json::to_string(&output).unwrap());
            std::process::exit(err.exit_code());
        }
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p kunkka-cli`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/kunkka-cli/src/lib.rs crates/kunkka-cli/src/main.rs
git commit -m "feat: add kunkka-cli main entry and public API"
```

---

### Task 7: Integration tests for ping and status

**Files:**
- Create: `crates/kunkka-cli/tests/integration.rs`

- [ ] **Step 1: Write the integration test**

Create `crates/kunkka-cli/tests/integration.rs`:

```rust
use kunkka_cli::cli::{Cli, CliCommand};
use kunkka_cli::run_command;
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
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
async fn cli_ping_returns_pong() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Ping,
            };
            run_command(&cli).await
        }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let result = cli_task.await.unwrap().unwrap();
    assert!(result.is_success());
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::json!({"ok":true,"result":{"type":"pong"}})
    );
}

#[tokio::test]
async fn cli_status_returns_status() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Status,
            };
            run_command(&cli).await
        }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let result = cli_task.await.unwrap().unwrap();
    assert!(result.is_success());
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["result"]["type"], "status");
    assert_eq!(value["result"]["worker_count"], 0);
    assert!(value["result"]["socket_path"].as_str().is_some());
    assert_eq!(value["result"]["runtime_ready"], true);
}

#[tokio::test]
async fn cli_core_unavailable_returns_error() {
    let (_root, paths) = test_paths();

    let cli = Cli {
        command: CliCommand::Ping,
    };
    let result = run_command(&cli).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "core_unavailable");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kunkka-cli --test integration`
Expected: FAIL (test may fail because `KunkkaPaths::resolve()` reads from actual env, not test paths)

- [ ] **Step 3: Add test helper for paths**

The integration test needs to set `HOME` env var so `KunkkaPaths::resolve()` uses test paths. Update `crates/kunkka-cli/src/lib.rs` to accept an optional socket path override for testing:

Add to `crates/kunkka-cli/src/lib.rs` a new function:

```rust
pub async fn run_command_with_socket(
    cli: &Cli,
    socket_path: &std::path::Path,
) -> Result<CliOutput, CliError> {
    match &cli.command {
        CliCommand::Ping => {
            let message = core_message_for_command(&cli.command)
                .ok_or_else(|| CliError::InvalidRequest("expected control command".to_string()))?;
            let response = send_core_control(socket_path, message).await?;
            match response {
                kunkka_protocol::core_control::CoreControlMessage::Pong(_) => {
                    Ok(CliOutput::success(CliResult::Pong))
                }
                other => Err(CliError::UnexpectedCoreResponse(format!(
                    "expected pong, got {other:?}"
                ))),
            }
        }
        CliCommand::Status => {
            let message = core_message_for_command(&cli.command)
                .ok_or_else(|| CliError::InvalidRequest("expected control command".to_string()))?;
            let response = send_core_control(socket_path, message).await?;
            match response {
                kunkka_protocol::core_control::CoreControlMessage::StatusResult(status) => {
                    Ok(CliOutput::success(CliResult::Status {
                        worker_count: status.worker_count,
                        socket_path: status.socket_path,
                        runtime_ready: status.runtime_ready,
                    }))
                }
                other => Err(CliError::UnexpectedCoreResponse(format!(
                    "expected status result, got {other:?}"
                ))),
            }
        }
        CliCommand::Dispatch {
            app_id,
            method,
            payload,
        } => {
            let request = build_frontend_dispatch_request(
                app_id.clone(),
                method.clone(),
                payload.clone(),
            );
            let response = send_frontend_dispatch(socket_path, request).await?;
            match response {
                FrontendDispatchResponse::Ok(payload) => {
                    let value: serde_json::Value = serde_json::from_slice(&payload.bytes)
                        .map_err(|err| CliError::CoreIpc(format!("invalid JSON payload: {err}")))?;
                    Ok(CliOutput::success(CliResult::Dispatch { payload: value }))
                }
                FrontendDispatchResponse::AppError { code, message } => {
                    Ok(CliOutput::success(CliResult::DispatchError { code, message }))
                }
                FrontendDispatchResponse::PlatformError { code, message } => {
                    Err(CliError::CorePlatform { code, message })
                }
            }
        }
    }
}
```

- [ ] **Step 4: Update integration tests to use run_command_with_socket**

Update `crates/kunkka-cli/tests/integration.rs` to use `run_command_with_socket` instead of `run_command`:

Replace `run_command(&cli).await` with `run_command_with_socket(&cli, &paths.socket_path).await` in all tests. Remove the unused `KunkkaPaths` import. Remove the `test_paths` function and use direct tempdir + socket path construction.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p kunkka-cli --test integration`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/kunkka-cli/src/lib.rs crates/kunkka-cli/tests/integration.rs
git commit -m "feat: add kunkka-cli integration tests for ping and status"
```

---

### Task 8: Dispatch integration test

**Files:**
- Modify: `crates/kunkka-cli/tests/integration.rs`

- [ ] **Step 1: Write the dispatch integration test**

Add to `crates/kunkka-cli/tests/integration.rs`:

```rust
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, Payload, RequestId, SessionId};
use kunkka_worker_sdk::{
    AppId, DispatchWorkerResponse, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId,
};

fn write_manifest(config_dir: &std::path::Path, body: &str) {
    let apps_dir = config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();
    std::fs::write(apps_dir.join("notes.json"), body).unwrap();
}

#[tokio::test]
async fn cli_dispatch_returns_worker_payload() {
    let root = tempdir().unwrap();
    let socket_path = root.path().join("core.sock");
    let config_dir = root.path().join("config");
    let data_dir = root.path().join("data");
    let state_dir = root.path().join("state");
    let cache_dir = root.path().join("cache");
    let runtime_dir = root.path().join("runtime");

    write_manifest(
        &config_dir,
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": ["--serve"]
            },
            "permissions": {
                "frontend_dispatch": {
                    "allowed_methods": ["search"]
                }
            }
        }"#,
    );

    let paths = KunkkaPaths {
        config_dir,
        data_dir,
        state_dir,
        cache_dir,
        runtime_dir,
        database_path: root.path().join("data/kunkka.db"),
        log_dir: root.path().join("state/logs"),
        socket_path: socket_path.clone(),
    };
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("notes"))
                .await
                .unwrap();
            let registration = client
                .register(RegisterWorkerRequest {
                    worker_id: WorkerId::new("notes"),
                    app_id: AppId::new("notes"),
                    capabilities: vec![WorkerCapability {
                        name: "notes.search".to_string(),
                        description: None,
                    }],
                })
                .await
                .unwrap();
            let request = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                client.recv_dispatch(),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(request.request.app_id.as_str(), "notes");
            assert_eq!(request.request.method, "search");
            client
                .respond_dispatch(
                    request,
                    DispatchWorkerResponse::Ok(Payload {
                        bytes: br#"{"items":["a","b"]}"#.to_vec(),
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

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let cli_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Dispatch {
                    app_id: "notes".to_string(),
                    method: "search".to_string(),
                    payload: serde_json::json!({"query": "kunkka"}),
                },
            };
            run_command_with_socket(&cli, &socket_path).await
        }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let result = cli_task.await.unwrap().unwrap();
    assert!(result.is_success());
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["result"]["type"], "dispatch");
    assert_eq!(value["result"]["payload"]["items"], serde_json::json!(["a", "b"]));

    let registration = tokio::time::timeout(std::time::Duration::from_secs(5), worker_task)
        .await
        .unwrap()
        .unwrap();
    assert!(registration.accepted);
}
```

- [ ] **Step 2: Run dispatch integration test**

Run: `cargo test -p kunkka-cli --test integration cli_dispatch_returns_worker_payload`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-cli/tests/integration.rs
git commit -m "feat: add kunkka-cli dispatch integration test"
```

---

### Task 9: Update documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/architecture.md`
- Modify: `docs/development-log.md`

- [ ] **Step 1: Update README.md**

Add `kunkka-cli` to the "已实现切片" section:

```markdown
- `kunkka-cli`：CLI frontend，支持 `ping`、`status`、`dispatch` 命令，通过 Kunkka IPC over Unix Domain Socket 连接 core。
```

- [ ] **Step 2: Update architecture.md**

Add `kunkka-cli` to the "当前实现切片" section:

```markdown
- `kunkka-cli`：CLI frontend，支持 `ping`、`status`、`dispatch`，通过 Kunkka IPC 直接连接 core。
```

Update the workspace layout section to include `kunkka-cli`.

- [ ] **Step 3: Update development-log.md**

Add to the top of `docs/development-log.md`:

```markdown
### CLI Frontend

Implemented:

- `crates/kunkka-cli` CLI frontend crate with `clap` arg parsing.
- `ping` command: sends `CorePingRequest`, outputs `{"ok":true,"result":{"type":"pong"}}`.
- `status` command: sends `CoreStatusRequest`, outputs `{"ok":true,"result":{"type":"status",...}}`.
- `dispatch` command: sends `FrontendDispatchRequest`, outputs worker payload or app error.
- CLI output JSON schema with `ok`, `result`, `error` fields.
- Error handling with consistent error codes and exit codes.
- Integration tests for ping, status, and dispatch.

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
```

- [ ] **Step 4: Run workspace verification**

Run:
```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add README.md docs/architecture.md docs/development-log.md
git commit -m "docs: add kunkka-cli to architecture and development log"
```
