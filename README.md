# kunkka

Kunkka 是面向多种 frontend form 的本地能力平台，包括 Browser Extension UI、CLI frontend、TUI frontend 和未来的本地 UI 形态。

## 已实现切片

- `kunkka-ipc`：内部 frame protocol、postcard serialization 和 Unix Domain Socket transport。
- `kunkka-core`：XDG path resolution、private runtime directory setup、minimal core IPC socket binding、in-memory worker registration、single-connection worker registration runtime loop，以及最小 core control protocol。
- `kunkka-worker-sdk`：共享 worker registration protocol、typed payload codec 和 registration client。

## 文档

- [架构](docs/architecture.md)
- [IPC](docs/ipc.md)
- [存储](docs/storage.md)
- [Workers](docs/worker.md)
- [Browser Extension 边界](docs/browser-extension.md)
- [权限](docs/permissions.md)
- [开发日志](docs/development-log.md)
