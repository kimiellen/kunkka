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
- `kunkka-core`：XDG path management、runtime socket setup、single-connection runtime loop、in-memory worker registry、core control protocol。
- `kunkka-worker-sdk`：worker registration protocol、payload codec、registration client。

Core runtime 当前按 `Payload.schema` 分发请求：

- `kunkka.worker.v1` 处理 worker registration。
- `kunkka.core-control.v1` 处理 `Ping/Pong` 和 `Status/StatusResult`。

CLI、TUI、native-host bridge、权限系统、worker request dispatch、数据库持久化仍是后续切片。
