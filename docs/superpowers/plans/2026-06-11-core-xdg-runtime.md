# Core XDG Runtime Implementation Plan

> Historical plan backfilled after implementation.

**Goal:** Implement XDG path resolution, private runtime directory setup, and minimal core IPC server binding.

**Implemented Commit:** `266d7ee feat: add core xdg runtime`

## Tasks Completed

- Added `KunkkaPaths` and `PathEnv`.
- Implemented XDG config/data/state/cache/runtime path resolution.
- Implemented `/tmp/kunkka-runtime-<uid>` fallback.
- Ensured private directories use `0700`.
- Added `CoreIpcServer`.
- Added `prepare_core_server`.
- Wired `kunkka-core` binary startup to XDG paths and IPC server.

## Verification

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
