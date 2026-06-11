# Core Registry Loop Design

## Goal

Wire the existing worker registration handler into the core runtime loop.

## Context

Current implementation has:

- `CoreIpcServer`
- `WorkerRegistry`
- `handle_worker_registration_frame`
- `WorkerClient`

Current tests manually accept one connection and call the handler. The core runtime does not yet own a registry loop.

## Design

Add `CoreRuntime` in `crates/kunkka-core/src/runtime.rs`.

`CoreRuntime` owns:

- `CoreIpcServer`
- `WorkerRegistry`

Public API:

```rust
pub struct CoreRuntime { ... }

impl CoreRuntime {
    pub async fn prepare(paths: &KunkkaPaths) -> Result<Self>;
    pub async fn run_once(&mut self) -> Result<()>;
    pub async fn run(mut self) -> Result<()>;
    pub fn registry(&self) -> &WorkerRegistry;
}
```

## First-Version Behavior

`run_once()`:

1. Accepts one IPC connection.
2. Reads one frame.
3. Handles the frame as a worker registration request.
4. Sends the registration response frame.
5. Updates the in-memory registry.

`run()`:

1. Loops forever.
2. Calls `run_once()`.
3. Returns on first error.

## Non-Goals

- No concurrent connection handling.
- No `tokio::spawn` per connection.
- No heartbeat loop.
- No worker request dispatch.
- No permissions.
- No SQLite persistence.
