# kunkka

Kunkka is a local capability platform for multiple frontend forms, including browser extension UI, CLI frontend, TUI frontend, and future local UI surfaces.

## Implemented Slices

- `kunkka-ipc`: internal frame protocol, postcard serialization, and Unix Domain Socket transport.
- `kunkka-core`: XDG path resolution, private runtime directory setup, minimal core IPC socket binding, and in-memory worker registration.
- `kunkka-worker-sdk`: shared worker registration protocol, typed payload codec, and registration client.

## Documentation

- [Architecture](docs/architecture.md)
- [IPC](docs/ipc.md)
- [Storage](docs/storage.md)
- [Workers](docs/worker.md)
- [Browser Extension Boundary](docs/browser-extension.md)
- [Permissions](docs/permissions.md)
- [Development Log](docs/development-log.md)
