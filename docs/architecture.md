# Kunkka 架构

## 项目定位

Kunkka 是本地能力平台和多形态应用 runtime。

Kunkka 底座分为两层：

**基础设施层**（`crates/`）：
- `kunkka-core` — 本地能力平台
- `kunkka-ipc` — IPC 协议基础设施
- `kunkka-protocol` — 共享 typed protocol
- `kunkka-worker-sdk` — Worker 开发 SDK
- `kunkka-native-host` — Browser Extension 桥接
- `kunkka-cli` — 底座管理命令行入口（`kunkka` 命令）
- `kunkka-tui` — 底座管理 TUI 平台

**上层应用层**（`apps-frontend/` + `apps-backend/`）：
- Browser Extension app UI（通过 `kunkka-native-host` 桥接）
- 上层 TUI 应用（通过 `kunkka-worker-sdk` + `kunkka-ipc` 连接 core）
- 上层 CLI 工具（通过 `kunkka-worker-sdk` + `kunkka-ipc` 连接 core）
- App backend workers（通过 `kunkka-worker-sdk` 连接 core）

`kunkka-cli` 和 `kunkka-tui` 是底座自身的管理工具，不是上层应用。上层应用通过 `apps-frontend/` 和 `apps-backend/` 目录开发，复用 core 的能力平台和 worker 机制。

Kunkka 为上层应用的 frontend 和 app backend worker 提供统一的本地能力基础。

## 高层架构

```text
上层应用前端                     底座管理工具
(Browser Extension / TUI App / CLI Tool)    (kunkka-cli / kunkka-tui)
        |                                           |
        | frontend request / management command      |
        v                                           v
kunkka-native-host (桥接)                     kunkka-cli / kunkka-tui
        |                                           |
        | Kunkka IPC over Unix Domain Socket          |
        +-------------------------------------------+
        |
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
        |
        v
app backend workers (apps-backend/)
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

`kunkka-cli` 是底座管理命令行入口，提供 `ping`、`status`、`dispatch`、`shell`、`approvals`、`llm`、`theme` 等管理命令。它不是上层应用 CLI。

`kunkka-tui` 是底座管理 TUI 平台，提供 ping 和 approvals 管理界面。它不是上层应用 TUI。

App backend workers 包含 app-specific backend business logic。

上层应用 frontend 通过 core 和 workers 调用本地能力。

## 当前实现切片

当前 workspace 已实现以下基础切片：

- `kunkka-ipc`：frame protocol、postcard codec、Unix Domain Socket transport。
- `kunkka-protocol`：shared core-control protocol 和 frontend-dispatch protocol。
- `kunkka-core`：XDG path management、runtime socket setup、single-connection runtime loop、in-memory worker registry、core control protocol、XDG app manifest registry、worker startup / active registry / idle cleanup manager、core-internal dispatch API、frontend-dispatch runtime handler、manifest-based frontend dispatch permissions、frontend dispatch permission audit persistence、SQLite/sqlx core database foundation、capability layer with fs operations and path permission checking。
- `kunkka-worker-sdk`：worker registration/dispatch protocol、payload codec、registration and dispatch helpers。
- `kunkka-native-host`：Native Messaging JSON 到 Kunkka IPC core-control/frontend-dispatch 的桥接入口。
- `kunkka-cli`：底座管理命令行入口，支持 `ping`、`status`、`dispatch`、`shell`、`approvals`、`llm` 管理命令，通过 Kunkka IPC 直接连接 core。
- `kunkka-tui`：底座管理 TUI 平台，基于 Ratatui + crossterm，支持 `ping` 和 `approvals` 管理界面，通过 Kunkka IPC 直接连接 core。

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
- `kunkka.frontend-dispatch.v1` 处理上层应用 frontend 到 app worker 的 dispatch request。
- `kunkka.capability.v1` 处理 worker 和底座管理工具的 capability 请求（当前支持文件系统、HTTP、SQLite、Shell、LLM 操作）。

更完整的权限系统仍是后续切片。
