# Kunkka Permissions

## Owner

Permission decisions belong in `kunkka-core`.

Native host, browser extension frontend, app frontend code, and kunkka 底座管理工具（kunkka-cli、kunkka-tui）must not make final local capability permission decisions.

## Permission Subjects

Permission subjects include:

- app frontend（上层应用前端）
- app backend worker
- kunkka 底座管理工具（kunkka-cli、kunkka-tui）
- Browser Extension shell
- registered CLI command

## Controlled Capabilities

Controlled capabilities include:

- LLM provider access
- external API request
- SQLite database access
- file query
- file edit
- shell command execution
- worker invocation
- task scheduling
- CLI command registration

## Security Principles

1. Frontends do not hold local credentials.
2. Frontends do not directly access the local filesystem.
3. Frontends do not directly execute shell commands.
4. Native host only bridges messages.
5. Workers do not bypass core for controlled capabilities.
6. Core audits controlled capability access.
7. File query and file edit permissions are distinct.
8. External API and LLM credentials must not be exposed to frontend code.

## Current Status

Permission enforcement is partially implemented.

Frontend dispatch is now checked against manifest-declared `permissions.frontend_dispatch.allowed_methods`. If the method is not in the allowed list, core returns `permission_denied`. If no manifest exists for the app, core returns `app_not_found`. Both allow and deny decisions are audited by `kunkka-core` into the core SQLite database.

Worker invocation, database access, file access, shell execution, and other controlled capabilities are not yet enforced.

## Current Frontend Dispatch Status

Frontend dispatch permission is checked in `kunkka-core` against the app manifest:

- `permissions.frontend_dispatch.allowed_methods` declares which methods are allowed.
- Missing `permissions`, missing `frontend_dispatch`, or empty `allowed_methods` means deny all.
- Permission decision is in `crates/kunkka-core/src/permissions.rs`.
- Permission decision outcome is persisted in the `frontend_dispatch_audit` table owned by `kunkka-core`.
- `native-host` does not make permission decisions.

## Implementation Slice: Frontend Dispatch Method Allowlist

`kunkka-core` owns the first concrete permission decision helper for frontend-to-worker dispatch. The helper reads `AppManifest.permissions.frontend_dispatch.allowed_methods` and allows only exact method-name matches.

Decision rules for this slice:

- If the requested method is present in `allowed_methods`, core returns allow.
- If the method is absent or the allowlist is empty, core returns deny with code `permission_denied`.
- Method matching is case-sensitive.
- Method matching does not trim or normalize whitespace.

The decision API is wired into the frontend dispatch runtime handler: `runtime.rs` checks permissions via `decide_frontend_dispatch` before calling `dispatch_with_start`.
