# Kunkka Storage

## XDG Rule

Kunkka storage must follow XDG Base Directory conventions.

Do not use these paths as default storage:

```text
~/.kunkka
./.kunkka
./data
/tmp/kunkka
```

## Config

Use:

```text
$XDG_CONFIG_HOME/kunkka
```

Fallback:

```text
~/.config/kunkka
```

Used for:

- user-editable config
- providers config
- permissions config
- app permission grants
- native host configuration templates

## Data

Use:

```text
$XDG_DATA_HOME/kunkka
```

Fallback:

```text
~/.local/share/kunkka
```

Default core database:

```text
$XDG_DATA_HOME/kunkka/kunkka.db
```

App database path:

```text
$XDG_DATA_HOME/kunkka/apps/<app-id>/app.db
```

## State

Use:

```text
$XDG_STATE_HOME/kunkka
```

Fallback:

```text
~/.local/state/kunkka
```

Logs belong under:

```text
$XDG_STATE_HOME/kunkka/logs/
```

## Cache

Use:

```text
$XDG_CACHE_HOME/kunkka
```

Fallback:

```text
~/.cache/kunkka
```

Cache content must be safely deletable.

## Runtime

Use:

```text
$XDG_RUNTIME_DIR/kunkka
```

Default core socket:

```text
$XDG_RUNTIME_DIR/kunkka/core.sock
```

If `$XDG_RUNTIME_DIR` is unavailable, Kunkka may use:

```text
/tmp/kunkka-runtime-<uid>
```

The fallback runtime directory must be `0700` and must not store persistent data.

## Core Database

`kunkka-core` owns the core SQLite database at `$XDG_DATA_HOME/kunkka/kunkka.db`.

Current implementation:

- `crates/kunkka-core/src/database.rs`: `CoreDatabase` with `connect()`, `schema_version()`, `ping()`, `pool()`.
- `crates/kunkka-core/migrations/`: embedded SQL migrations run at startup.
- First migration creates `core_metadata` table with `schema_version` = `1`.
- SQLite pragmas: `foreign_keys = ON`, `journal_mode = WAL`.

## Current Implementation

`crates/kunkka-core/src/xdg.rs` provides:

- `KunkkaPaths`
- `PathEnv`
- XDG path resolution
- runtime fallback
- private directory creation
