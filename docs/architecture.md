# Kunkka 架构

## 项目定位

Kunkka 是本地能力平台和多形态应用 runtime。

它支持多种 frontend form：

- Browser Extension UI
- CLI frontend
- TUI frontend
- Future desktop or webview frontends

Kunkka 为 app frontend 和 app backend worker 提供统一的本地能力基础。

## 高层架构

```text
Browser Extension / CLI / TUI
        |
        | frontend request
        v
kunkka-native-host / kunkka-cli / kunkka-tui
        |
        | Kunkka IPC over Unix Domain Socket
        v
kunkka-core
        |
        +-- capability platform
        +-- permission system
        +-- worker manager
        +-- app backend registry
        +-- database layer
        +-- file system layer
        +-- shell execution layer
        +-- LLM provider layer
        +-- external API request layer
        +-- scheduler
        +-- CLI command registry
        |
        v
app backend workers
```

## 已确认技术栈

- Language: Rust
- Async runtime: Tokio
- IPC transport: Unix Domain Socket
- Framing: `tokio-util::codec::LengthDelimitedCodec`
- Serialization: `postcard`
- Browser local boundary: WebExtension Native Messaging JSON
- TUI: Ratatui + crossterm
- CLI: clap
- Logging: tracing + tracing-subscriber
- Storage: SQLite + sqlx
- First target environment: Linux / Arch Linux

## Workspace layout

```text
kunkka/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── AGENTS.md
├── crates/
│   ├── kunkka-ipc/
│   ├── kunkka-core/
│   ├── kunkka-worker-sdk/
│   ├── kunkka-native-host/
│   ├── kunkka-cli/
│   └── kunkka-tui/
├── apps-backend/
├── apps-frontend/
├── schemas/
├── docs/
└── xtask/
```

并非所有目录都已经存在。该 layout 是目标架构。

## 边界

`kunkka-core` 是本地能力平台。它不能包含具体 app business logic。

`kunkka-ipc` 是协议基础设施。它不能知道 browser、app、LLM、database 或 worker business semantics。

`kunkka-native-host` 只负责桥接 Native Messaging JSON 和 Kunkka IPC。

App backend workers 包含 app-specific backend business logic。

Frontend forms 通过 core 和 workers 调用本地能力。

## 当前实现切片

当前 workspace 已实现以下基础切片：

- `kunkka-ipc`：frame protocol、postcard codec、Unix Domain Socket transport。
- `kunkka-protocol`：shared core-control protocol 和 frontend-dispatch protocol。
- `kunkka-core`：XDG path management、runtime socket setup、single-connection runtime loop、in-memory worker registry、core control protocol、XDG app manifest registry、worker startup / active registry / idle cleanup manager、core-internal dispatch API、frontend-dispatch runtime handler、manifest-based frontend dispatch permissions、frontend dispatch permission audit persistence、SQLite/sqlx core database foundation、capability layer with fs operations and path permission checking。
- `kunkka-worker-sdk`：worker registration/dispatch protocol、payload codec、registration and dispatch helpers。
- `kunkka-native-host`：Native Messaging JSON 到 Kunkka IPC core-control/frontend-dispatch 的桥接入口。
- `kunkka-cli`：CLI frontend，支持 `ping`、`status`、`dispatch`，通过 Kunkka IPC 直接连接 core。
- `kunkka-tui`：TUI frontend，基于 Ratatui + crossterm，当前支持 `ping`，通过 Kunkka IPC 直接连接 core。

Core 内部 capability 层：

- `capability/mod.rs`：`kunkka.capability.v1` 协议类型和 codec，capability 请求路由。
- `capability/permissions.rs`：路径白名单校验，支持目录前缀匹配和精确文件匹配，路径规范化。
- `capability/fs.rs`：文件系统操作（`read_file`、`write_file`、`list_dir`）。
- `capability/http.rs`：HTTP capability for external API requests with domain whitelist.
- `capability/sqlite.rs`：SQLite capability for app database management with per-app connection store.
- App manifest `capabilities.fs.paths` 白名单配置。

Core runtime 当前按 `Payload.schema` 分发请求：

- `kunkka.worker.v1` 处理 worker registration。
- `kunkka.core-control.v1` 处理 `Ping/Pong` 和 `Status/StatusResult`。
- `kunkka.frontend-dispatch.v1` 处理 frontend 到 app worker 的 dispatch request。
- `kunkka.capability.v1` 处理 worker capability 请求（当前支持文件系统操作）。

更完整的权限系统仍是后续切片。
