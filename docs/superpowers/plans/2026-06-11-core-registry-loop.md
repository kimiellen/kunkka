# Core Registry Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire worker registration into the core runtime loop.

**Architecture:** `CoreRuntime` owns `CoreIpcServer` and `WorkerRegistry`. First version uses a single-connection loop: one accept, one frame, one response, then return to the loop.

**Tech Stack:** Rust 2021, Tokio, Kunkka IPC, kunkka-worker-sdk.

---

## Files

- Create: `crates/kunkka-core/src/runtime.rs`
- Modify: `crates/kunkka-core/src/lib.rs`
- Modify: `crates/kunkka-core/src/main.rs`
- Create: `crates/kunkka-core/tests/core_runtime_loop.rs`
- Modify: `README.md`
- Modify: `docs/worker.md`

### Task 1: CoreRuntime prepare and registry access

- [ ] **Step 1: Write the failing test**

Create `crates/kunkka-core/tests/core_runtime_loop.rs` with:

```rust
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
async fn prepare_core_runtime_creates_dirs_binds_socket_and_starts_empty() {
    let (_root, paths) = test_paths();

    let runtime = prepare_core_runtime(&paths).await.unwrap();

    assert!(paths.config_dir.exists());
    assert!(paths.data_dir.exists());
    assert!(paths.state_dir.exists());
    assert!(paths.cache_dir.exists());
    assert!(paths.runtime_dir.exists());
    assert!(paths.log_dir.exists());
    assert!(paths.socket_path.exists());
    assert!(runtime.registry().is_empty());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p kunkka-core --test core_runtime_loop prepare_core_runtime_creates_dirs_binds_socket_and_starts_empty`

Expected: FAIL because `prepare_core_runtime` and `CoreRuntime` do not exist.

- [ ] **Step 3: Implement minimal CoreRuntime prepare path**

Create `crates/kunkka-core/src/runtime.rs`:

```rust
use crate::ipc_server::CoreIpcServer;
use crate::worker_registry::WorkerRegistry;
use crate::xdg::KunkkaPaths;
use crate::Result;

pub struct CoreRuntime {
    server: CoreIpcServer,
    registry: WorkerRegistry,
}

impl CoreRuntime {
    pub async fn prepare(paths: &KunkkaPaths) -> Result<Self> {
        paths.ensure_dirs()?;
        let server = CoreIpcServer::bind(paths).await?;

        Ok(Self {
            server,
            registry: WorkerRegistry::new(),
        })
    }

    pub fn registry(&self) -> &WorkerRegistry {
        &self.registry
    }
}
```

Update `crates/kunkka-core/src/lib.rs`:

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

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p kunkka-core --test core_runtime_loop prepare_core_runtime_creates_dirs_binds_socket_and_starts_empty`

Expected: PASS.

### Task 2: CoreRuntime run_once worker registration loop

- [ ] **Step 1: Add the failing run_once test**

Append to `crates/kunkka-core/tests/core_runtime_loop.rs`:

```rust
use kunkka_worker_sdk::{
    AppId, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId,
};

fn request() -> RegisterWorkerRequest {
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
async fn run_once_accepts_worker_registration_and_updates_registry() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let register_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("worker-1"))
                .await
                .unwrap();

            client.register(request()).await.unwrap()
        }
    });

    runtime.run_once().await.unwrap();

    let response = register_task.await.unwrap();

    assert!(response.accepted);
    assert_eq!(response.worker_id.as_str(), "worker-1");
    assert!(runtime.registry().get(&WorkerId::new("worker-1")).is_some());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p kunkka-core --test core_runtime_loop run_once_accepts_worker_registration_and_updates_registry`

Expected: FAIL because `CoreRuntime::run_once` does not exist.

- [ ] **Step 3: Implement run_once and run**

Update `crates/kunkka-core/src/runtime.rs`:

```rust
use crate::ipc_server::CoreIpcServer;
use crate::worker_registry::{handle_worker_registration_frame, WorkerRegistry};
use crate::xdg::KunkkaPaths;
use crate::Result;

pub struct CoreRuntime {
    server: CoreIpcServer,
    registry: WorkerRegistry,
}

impl CoreRuntime {
    pub async fn prepare(paths: &KunkkaPaths) -> Result<Self> {
        paths.ensure_dirs()?;
        let server = CoreIpcServer::bind(paths).await?;

        Ok(Self {
            server,
            registry: WorkerRegistry::new(),
        })
    }

    pub async fn run_once(&mut self) -> Result<()> {
        let mut connection = self.server.accept_one().await?;
        let Some(frame) = connection.recv_frame().await? else {
            return Ok(());
        };

        let response = handle_worker_registration_frame(&mut self.registry, frame)?;
        connection.send_frame(&response).await?;

        Ok(())
    }

    pub async fn run(mut self) -> Result<()> {
        loop {
            self.run_once().await?;
        }
    }

    pub fn registry(&self) -> &WorkerRegistry {
        &self.registry
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p kunkka-core --test core_runtime_loop run_once_accepts_worker_registration_and_updates_registry`

Expected: PASS.

### Task 3: Main entry and docs

- [ ] **Step 1: Update `crates/kunkka-core/src/main.rs`**

Replace `prepare_core_server` with `prepare_core_runtime`:

```rust
use kunkka_core::xdg::KunkkaPaths;

#[tokio::main]
async fn main() -> kunkka_core::Result<()> {
    let paths = KunkkaPaths::resolve()?;
    let socket_path = paths.socket_path.clone();
    let runtime = kunkka_core::prepare_core_runtime(&paths).await?;

    println!("kunkka-core listening on {}", socket_path.display());

    runtime.run().await
}
```

- [ ] **Step 2: Update documentation**

Update `README.md` `kunkka-core` bullet to:

```md
- `kunkka-core`: XDG path resolution, private runtime directory setup, minimal core IPC socket binding, in-memory worker registration, and a single-connection worker registration runtime loop.
```

Update `docs/worker.md` current core registry behavior list to include:

```md
- accept one worker registration connection through core runtime loop
```

- [ ] **Step 3: Verify binary build**

Run: `cargo check -p kunkka-core --bin kunkka-core`

Expected: PASS.

### Task 4: Full verification

- [ ] **Step 1: Format check**

Run: `cargo fmt --all --check`

Expected: PASS.

- [ ] **Step 2: Workspace tests**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Expected: PASS.

- [ ] **Step 4: Git status**

Run: `git status --short`

Expected: shows only this task's intended changes.
