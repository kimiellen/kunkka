# Kunkka Permissions

## Owner

Permission decisions belong in `kunkka-core`.

Native host, browser extension frontend, CLI frontend, TUI frontend, and app frontend code must not make final local capability permission decisions.

## Permission Subjects

Permission subjects include:

- app frontend
- app backend worker
- CLI frontend
- TUI frontend
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

Permission enforcement is not implemented yet.

## Current Frontend Dispatch Status

Frontend dispatch is currently allowed by an explicit temporary decision inside `kunkka-core`. This keeps the permission decision owner in core and avoids native-host-side authorization logic.

The temporary allow decision must be replaced by the real permission system before Kunkka treats worker invocation as enforceable per subject or per app.

## Implementation Slice: Frontend Dispatch Method Allowlist

`kunkka-core` owns the first concrete permission decision helper for frontend-to-worker dispatch. The helper reads `AppManifest.permissions.frontend_dispatch.allowed_methods` and allows only exact method-name matches.

Decision rules for this slice:

- If the requested method is present in `allowed_methods`, core returns allow.
- If the method is absent or the allowlist is empty, core returns deny with code `permission_denied`.
- Method matching is case-sensitive.
- Method matching does not trim or normalize whitespace.

This slice only introduces the reusable decision API; wiring runtime enforcement happens in a later task.
