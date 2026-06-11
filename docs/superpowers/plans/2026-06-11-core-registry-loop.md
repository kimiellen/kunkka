# Core Registry Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire worker registration into the core runtime loop.

**Architecture:** `CoreRuntime` owns `CoreIpcServer` and `WorkerRegistry`. First version uses a single-connection loop: one accept, one frame, one response, then return to the loop.

**Tech Stack:** Rust 2021, Tokio, Kunkka IPC, kunkka-worker-sdk.

---

## Tasks

### Task 1: CoreRuntime type

- Create `crates/kunkka-core/src/runtime.rs`
- Add `CoreRuntime::prepare`
- Add `CoreRuntime::run_once`
- Add `CoreRuntime::run`
- Add `CoreRuntime::registry`

### Task 2: Tests

- Create `crates/kunkka-core/tests/core_runtime_loop.rs`
- Test `prepare_core_runtime` creates dirs, binds socket, and starts with empty registry.
- Test `run_once` accepts `WorkerClient::register` and stores worker in registry.

### Task 3: Main entry

- Update `crates/kunkka-core/src/main.rs` to use `prepare_core_runtime`.
- Keep `prepare_core_server` for existing tests.

### Task 4: Verification

Run:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
