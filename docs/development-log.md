# Kunkka Development Log

## 2026-06-18

### HTTP Capability (External API Request)

Implemented:

- `HttpRequestParams`/`HttpResponse` protocol types with postcard codec in `capability/http.rs`.
- App manifest `capabilities.http.domains` domain whitelist field with validation.
- Domain whitelist matching: exact match, case-insensitive, `http` and `https` schemes only.
- HTTP client with reqwest: fixed 30s timeout, auto-redirect (max 10) with whitelist enforcement, gzip/deflate compression, HTTP/1.1 + HTTP/2 auto-negotiation, no proxy.
- Error codes: `invalid_params`, `permission_denied`, `scheme_not_allowed`, `timeout`, `io_error`.
- Tests: manifest loading (4), unit tests (7), runtime integration tests (4).

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## 2026-06-17

### Capability Layer (File System)

Implemented:

- `kunkka.capability.v1` payload schema with `CapabilityRequest`/`CapabilityResponse`/`CapabilityError` types and postcard codec in `kunkka-core/src/capability/mod.rs`.
- App manifest `capabilities.fs.paths` whitelist field with path validation (absolute paths only) in `app_manifest.rs`.
- Path permission checking with normalization (`.`, `..`, double slashes) and directory prefix / exact file matching in `capability/permissions.rs`.
- File system operations: `read_file`, `write_file`, `list_dir` with proper error codes (`permission_denied`, `not_found`, `io_error`, `not_utf8`, `unknown_method`) in `capability/fs.rs`.
- Runtime capability dispatch: `CAPABILITY_SCHEMA` branch in `run_connection()`, short-lived connection model, app_id-based manifest lookup.
- Security fix: directory prefix sibling escape prevention (path-component-aware boundary check).
- 22 tests: 8 permission tests, 6 fs ops tests, 3 manifest tests, 6 integration tests through IPC.

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## 2026-06-16

### TUI Frontend Skeleton

Implemented:

- `crates/kunkka-tui` as 7th workspace crate with Ratatui + crossterm dependencies.
- `TuiError` enum with `CoreUnavailable`, `CoreIpc`, `UnexpectedCoreResponse` variants.
- `resolve_socket_path()` with XDG socket path resolution matching CLI/native-host pattern.
- `ping_core()` async IPC client using kunkka-ipc + kunkka-protocol.
- `App` state machine with `PingStatus` enum (Idle/Loading/Ok/Err).
- Ratatui UI rendering with centered layout, colored status display.
- Event loop with crossterm keyboard input and async IPC via tokio mpsc.
- Main entry point with terminal raw mode and alternate screen management.
- Integration test verifying TUI client can ping core and receive pong.

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### Frontend Dispatch Audit

Implemented:

- `kunkka-core` core database migration `0002_frontend_dispatch_audit.sql`.
- `CoreDatabase` frontend dispatch audit write helper with decision/reason validation.
- frontend dispatch runtime now writes audit rows for `allow/allowed`, `deny/permission_denied`, and `deny/app_not_found` decisions before returning or dispatching.
- integration tests verify both allow and deny paths persist audit rows.

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### Core Database Foundation

Implemented:

- `crates/kunkka-core/src/database.rs` with `CoreDatabase` struct.
- SQLite connection pool via sqlx with `runtime-tokio` and `sqlite` features.
- Embedded migrations via `sqlx::migrate!()`.
- First migration creates `core_metadata` table with `schema_version` = `1`.
- SQLite pragmas: `foreign_keys = ON`, `journal_mode = WAL`.
- Integrated into `CoreRuntime::prepare()` lifecycle.

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

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

### CLI Workspace Skeleton

Scope:

- Add `crates/kunkka-cli` as a new frontend crate in the workspace.
- Keep this slice limited to Cargo workspace setup, crate manifest, minimal binary entrypoint, and placeholder library modules.
- Add shared `clap` dependency for future CLI argument parsing without implementing CLI behavior in this task.

Verification plan:

1. Run `cargo check -p kunkka-cli` after adding `lib.rs` module declarations but before placeholder files; expected failure from missing modules.
2. Add empty placeholder modules.
3. Re-run `cargo check -p kunkka-cli`; expected pass.

### Manifest Frontend Dispatch Permissions

Implemented:

- `AppPermissions` and `FrontendDispatchPermissions` types in `kunkka-core/src/app_manifest.rs`.
- `permissions.rs` module with `decide_frontend_dispatch` and `PermissionDecision`.
- Runtime handler uses manifest permissions instead of `allow_frontend_dispatch_v1()`.
- Deny-by-default: missing permissions, empty `allowed_methods`, or method not in list returns `permission_denied`.
- Manifest loading validates that `allowed_methods` does not contain blank strings.

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## 2026-06-15

### Frontend Dispatch

Implemented:

- `kunkka-protocol` frontend-dispatch v1 protocol and payload codec.
- `kunkka-core` frontend-dispatch runtime handler backed by existing worker dispatch.
- Core-owned temporary allow decision for frontend dispatch.
- `kunkka-native-host` Native Messaging JSON `dispatch` command.
- JSON payload conversion between Native Messaging and opaque Kunkka IPC payloads.

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### Worker Dispatch

Implemented:

- XDG JSON app manifest loading from `$XDG_CONFIG_HOME/kunkka/apps/*.json`（即 `KunkkaPaths.config_dir/apps/*.json`）。
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
cargo test --workspace  # post-commit
git status --short      # clean
```

## 2026-06-11

### Native Host Bridge

Commit:

```text
c4b360c docs: add native host bridge design
51b7c86 docs: add native host bridge plan
e136400 feat: add shared protocol crate
79fc475 refactor: use shared core control protocol
afd567e feat: resolve native host core socket
70cfd6f feat: map native commands to core control
2d5d141 feat: bridge native host session to core
0f603fd feat: run native messaging host loop
866c859 fix: stop native host loop after framing errors
```

Implemented:

- `kunkka-protocol` shared core-control protocol。
- `kunkka-native-host` Native Messaging JSON bridge for `ping` and `status`。
- native-host keeps core IPC connection cached, clears it after IPC/protocol failures, and does not auto-start core。

Verification:

```text
cargo test -p kunkka-native-host
cargo clippy -p kunkka-native-host --all-targets -- -D warnings
```

### Native Host Core-Control Mapping

Scope:

- Add a bridge mapping layer in `kunkka-native-host` from native commands to `kunkka-protocol` core-control messages.
- Convert expected core-control responses back into native host results.
- Reject mismatched core responses with `UnexpectedCoreResponse`.
- Do not connect to core/session or implement transport work in this slice.

TDD and verification:

1. Add integration tests for ping/status mapping and unexpected response rejection.
2. Run the bridge mapping test first to confirm RED while `bridge` is absent.
3. Add only the `kunkka-protocol` dependency and minimal bridge module needed for GREEN.
4. Re-run the bridge mapping test and formatting check before committing.

Commit:

```text
70cfd6f feat: map native commands to core control
```

Verification:

```text
cargo test -p kunkka-native-host --test bridge_mapping
cargo fmt --all --check
```

### Native Messaging JSON and Length Codec

Scope:

- Add JSON request/response protocol types for `kunkka-native-host`.
- Add WebExtension Native Messaging little-endian length-prefixed read/write helpers.
- Keep native host as a bridge-only crate; do not add core bridge/path work or app business logic.

TDD verification plan:

1. Add integration tests for JSON protocol encode/decode behavior and native messaging length codec.
2. Run native-host test targets first to confirm RED while library modules are absent.
3. Implement minimal modules and manifest dependencies needed for the tests.
4. Re-run the same tests and formatting check before committing.

Commit:

```text
f0f669e feat: add native messaging protocol
```

Verification:

```text
cargo test -p kunkka-native-host --test native_protocol
cargo test -p kunkka-native-host --test native_messaging
cargo fmt --all --check
```

### IPC Workspace

Commit:

```text
f22bc64 feat: initialize ipc workspace
```

Implemented:

- Rust Cargo workspace
- `kunkka-ipc`
- IPC frame enum
- strong ID newtypes
- opaque payload
- postcard encode/decode
- UDS listener/connection
- minimal `kunkka-core`, `kunkka-worker-sdk`, `kunkka-native-host` crates

Verification:

```text
cargo test --workspace
```

### Core XDG Runtime

Commit:

```text
266d7ee feat: add core xdg runtime
```

Implemented:

- `KunkkaPaths`
- XDG config/data/state/cache/runtime path resolution
- `/tmp/kunkka-runtime-<uid>` fallback
- private directory setup with `0700`
- minimal core IPC socket binding
- core runtime startup helper

Verification:

```text
cargo test --workspace
```

### Worker Registration

Commit:

```text
8962b00 feat: add worker registration
```

Implemented:

- worker registration protocol in `kunkka-worker-sdk`
- typed worker payload codec
- `WorkerClient`
- in-memory `WorkerRegistry`
- registration frame handler
- end-to-end worker registration over UDS

Verification:

```text
cargo test --workspace
```
