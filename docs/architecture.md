# Kunkka Architecture

## Project Positioning

Kunkka is a local capability platform and multi-form application runtime.

It supports multiple frontend forms:

- Browser Extension UI
- CLI frontend
- TUI frontend
- Future desktop or webview frontends

Kunkka provides one local capability foundation for app frontends and app backend workers.

## High-Level Architecture

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

## Confirmed Tech Stack

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

## Workspace Layout

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

Not all directories exist yet. This layout is the intended architecture target.

## Boundaries

`kunkka-core` is the local capability platform. It must not contain concrete app business logic.

`kunkka-ipc` is protocol infrastructure. It must not know browser, app, LLM, database, or worker business semantics.

`kunkka-native-host` is only a bridge between Native Messaging JSON and Kunkka IPC.

App backend workers contain app-specific backend business logic.

Frontend forms call local capabilities through core and workers.
