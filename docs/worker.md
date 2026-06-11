# Kunkka Workers

## Role

App backend workers are independent Rust processes that provide app-specific backend logic.

Workers connect to `kunkka-core` through Kunkka IPC over Unix Domain Socket.

## Current Worker Registration

`kunkka-worker-sdk` currently defines:

- `WorkerId`
- `AppId`
- `WorkerCapability`
- `RegisterWorkerRequest`
- `RegisterWorkerResponse`
- `WorkerProtocolMessage`
- `WorkerClient`

Worker registration payloads are typed in `kunkka-worker-sdk` and serialized into opaque IPC payloads.

## Current Core Registry

`kunkka-core` currently provides an in-memory `WorkerRegistry`.

Current behavior:

- register worker
- replace duplicate worker ID
- query registered worker by `WorkerId`
- handle one worker registration request frame
- return `RegisterWorkerAccepted`

## Not Implemented Yet

- worker process spawning
- heartbeat loop
- worker lifecycle restart policy
- request dispatch to worker
- worker streams
- worker cancellation
- SQLite persistence
- permission checks
