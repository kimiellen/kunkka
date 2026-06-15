# Kunkka Development Log

## 2026-06-11

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
