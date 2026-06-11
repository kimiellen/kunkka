# Browser Extension Boundary

## Rule

Browser Extension frontend must not connect directly to Unix Domain Socket.

Browser Extension enters the local Kunkka system only through WebExtension Native Messaging.

## Native Host

`kunkka-native-host` is responsible for:

```text
Native Messaging JSON <-> Kunkka IPC
```

Native host must not implement:

- app business logic
- database logic
- LLM provider logic
- file system business logic
- shell execution business logic
- permission decisions

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
