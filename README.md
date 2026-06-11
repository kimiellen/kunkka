# kunkka

Kunkka is a local capability platform for multiple frontend forms, including browser extension UI, CLI frontend, TUI frontend, and future local UI surfaces.

Implemented slices:

- `kunkka-ipc`: internal frame protocol, postcard serialization, and Unix Domain Socket transport.
- `kunkka-core`: XDG path resolution, private runtime directory setup, and minimal core IPC socket binding.
