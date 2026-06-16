# kunkka

Kunkka 是面向多种 frontend form 的本地能力平台，包括 Browser Extension UI、CLI frontend、TUI frontend 和未来的本地 UI 形态。

## 已实现切片

- `kunkka-ipc`：内部 frame protocol、postcard serialization 和 Unix Domain Socket transport。
- `kunkka-protocol`：共享 typed protocol crate，当前承载 core-control v1、frontend-dispatch v1 message 和 payload codec。
- `kunkka-core`：XDG path resolution、private runtime directory setup、minimal core IPC socket binding、in-memory worker registration、single-connection worker registration runtime loop、core control protocol、XDG app manifest loading、按需 worker startup、core-internal worker dispatch、frontend-dispatch runtime handler，以及 idle worker cleanup。
- `kunkka-worker-sdk`：共享 worker registration/dispatch protocol、typed payload codec、registration client 和 dispatch receive/respond helpers。
- `kunkka-native-host`：WebExtension Native Messaging JSON bridge，支持 `ping`、`status` 和 JSON `dispatch`，并转发到 core IPC。
- `kunkka-cli`：CLI frontend，支持 `ping`、`status`、`dispatch` 命令，通过 Kunkka IPC over Unix Domain Socket 连接 core。

## 文档

- [架构](docs/architecture.md)
- [IPC](docs/ipc.md)
- [存储](docs/storage.md)
- [Workers](docs/worker.md)
- [Browser Extension 边界](docs/browser-extension.md)
- [权限](docs/permissions.md)
- [开发日志](docs/development-log.md)
