# Kunkka Agent Instructions

## Communication

- All assistant communication and project documentation should be written in Chinese unless a file format or external tool requires English.
- Keep implementation changes minimal and focused.
- Prefer TDD for behavior changes.

## Iron Law

- Before any implementation, execution, or commit, the relevant design, plan, architectural decision, or project constraint must be documented in the repository first.
- If the required document does not exist, create or update it before changing source code or running the implementation task.
- Do not rely on chat history as the source of truth for project decisions.

## Project Boundaries

Kunkka is a local capability platform for multiple frontend forms.

Kunkka is not a single browser extension, CLI, or TUI. It is a unified local capability platform. Browser Extension UI, CLI frontend, TUI frontend, and future local UI forms access local capabilities through Kunkka IPC and app backend workers.

## Core Principles

1. Shared local capabilities live in `kunkka-core`.
2. App backend business logic lives in independent app backend workers.
3. Frontends are responsible for interaction, display, entrypoints, permission routing, and capability calls.
4. Local IPC uses Kunkka IPC over Unix Domain Socket.
5. Browser Extension enters the local system only through Native Messaging.
6. CLI, TUI, and Browser Extension are frontend forms and must not directly implement core capabilities.
7. App frontend and app backend are linked through app registry.
8. Persistent data, config, state, cache, and runtime files must follow XDG Base Directory rules.

## Current Implementation Slices

- `kunkka-ipc`: frame protocol, postcard codec, and UDS transport.
- `kunkka-core`: XDG path resolution, runtime socket setup, minimal core IPC server, in-memory worker registry.
- `kunkka-worker-sdk`: shared worker registration protocol, payload codec, and registration client.

## Architecture Boundaries

### `crates/kunkka-ipc`

Only protocol, frame, serialization, codec, transport, and IPC errors belong here.

Do not add:

- App business logic
- LLM logic
- Database business logic
- Browser-specific logic
- Permission decisions

### `crates/kunkka-core`

Core owns:

- Capability platform
- Permission system
- Worker manager and worker registry
- App registry
- Database layer
- File system capability
- Shell capability
- LLM provider abstraction
- External API abstraction
- Scheduler
- CLI command registry
- XDG path management

Do not add concrete app business logic to core.

### `crates/kunkka-worker-sdk`

Worker SDK owns:

- Connection to core
- Worker registration protocol
- Worker request handling helpers
- Event / stream helpers
- Cancel / heartbeat helpers

### `crates/kunkka-native-host`

Native host only bridges:

```text
WebExtension Native Messaging JSON <-> Kunkka IPC
```

It must not implement business logic or permission decisions.

## Storage Rules

Do not use these as default storage paths:

```text
~/.kunkka
./.kunkka
./data
/tmp/kunkka
```

Runtime fallback may use:

```text
/tmp/kunkka-runtime-<uid>
```

The fallback runtime directory must be `0700` and must not store long-term data.
