# kunkka

Kunkka 是本地能力平台，为上层应用提供统一的能力基础。底座包含 core 能力平台、IPC 协议栈、worker SDK、底座管理工具（kunkka-cli、kunkka-tui）和 Browser Extension 桥接（kunkka-native-host）。上层应用（Browser Extension、TUI 应用、CLI 工具）通过 `apps-frontend/` 和 `apps-backend/` 目录开发。

## 已实现切片

- `kunkka-ipc`：内部 frame protocol、postcard serialization 和 Unix Domain Socket transport。
- `kunkka-protocol`：共享 typed protocol crate，当前承载 core-control v1、frontend-dispatch v1 message 和 payload codec。
- `kunkka-core`：XDG path resolution、private runtime directory setup、minimal core IPC socket binding、in-memory worker registration、single-connection worker registration runtime loop、core control protocol、XDG app manifest loading、按需 worker startup、core-internal worker dispatch、frontend-dispatch runtime handler，以及 idle worker cleanup。
- `kunkka-worker-sdk`：共享 worker registration/dispatch protocol、typed payload codec、registration client 和 dispatch receive/respond helpers。
- `kunkka-native-host`：WebExtension Native Messaging JSON bridge，支持 `ping`、`status` 和 JSON `dispatch`，并转发到 core IPC。
- `kunkka-cli`：底座管理命令行入口（`kunkka` 命令），支持 `ping`、`status`、`dispatch`、`shell`、`approvals`、`llm` 管理命令，通过 Kunkka IPC over Unix Domain Socket 连接 core。
- `kunkka-tui`：底座管理 TUI 平台，基于 Ratatui + crossterm，支持 `ping` 和 `approvals` 管理界面，通过 Kunkka IPC over Unix Domain Socket 连接 core。

## 文档

- [架构](docs/architecture.md)
- [IPC](docs/ipc.md)
- [存储](docs/storage.md)
- [Workers](docs/worker.md)
- [Browser Extension 边界](docs/browser-extension.md)
- [权限](docs/permissions.md)
- [开发日志](docs/development-log.md)
