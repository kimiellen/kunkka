# Browser Extension Boundary

## Rule

Browser Extension frontend must not connect directly to Unix Domain Socket.

Browser Extension enters the local Kunkka system only through WebExtension Native Messaging.

## Native Host

`kunkka-native-host` is responsible for:

```text
Native Messaging JSON <-> Kunkka IPC core-control/frontend-dispatch
```

Native host must not implement:

- app business logic
- database logic
- LLM provider logic
- file system business logic
- shell execution business logic
- permission decisions

第一版 native-host JSON API：

- request `{ "id": "req-1", "command": "ping" }` -> response `{ "id": "req-1", "ok": true, "result": { "type": "pong" } }`
- request `{ "id": "req-2", "command": "status" }` -> response `{ "id": "req-2", "ok": true, "result": { "type": "status", "worker_count": 0, "socket_path": "/run/user/1000/kunkka/core.sock", "runtime_ready": true } }`
- core 不可用时返回 `core_unavailable`。
- native-host 不自动启动 core。

## Native Messaging Dispatch

Browser Extension may request app dispatch through `kunkka-native-host` with high-level JSON:

```json
{ "id": "req-1", "command": "dispatch", "app_id": "notes", "method": "search", "payload": { "query": "hello" } }
```

The extension does not see Kunkka IPC frames, Unix sockets, postcard payloads, or worker protocol messages. `kunkka-native-host` forwards the request to `kunkka-core` using `kunkka.frontend-dispatch.v1`.

## Extension Shell

Future extension shell path:

```text
apps-frontend/extension/shell/
```

Responsibilities:

- manifest
- background service worker
- native messaging client
- app registry frontend
- launcher
- permissions routing
- shortcut routing
- popup routing
- side panel routing
- full page routing
- new tab routing

## Extension Apps

Future app UI path:

```text
apps-frontend/extension/apps/<app-id>/
```

Each app owns its own surfaces:

```text
popup/
side_panel/
full_page/
new_tab/
```

Do not mix all app pages into a global `src/pages` directory.
