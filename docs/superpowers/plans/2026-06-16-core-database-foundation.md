# Core SQLite Database Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add SQLite/sqlx core database foundation to `kunkka-core` with metadata table and schema version query.

**Architecture:** `CoreDatabase` module in `kunkka-core` opens a SQLite connection pool, sets pragmas, runs embedded migrations, and provides `schema_version()` / `ping()` APIs. Integrated into `CoreRuntime::prepare()` lifecycle.

**Tech Stack:** Rust, sqlx (runtime-tokio, sqlite), tokio, tempfile (dev)

---

### Task 1: Add sqlx dependencies

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/kunkka-core/Cargo.toml`

- [ ] **Step 1: Add sqlx to workspace dependencies**

Add to `Cargo.toml` `[workspace.dependencies]` section:

```toml
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
```

- [ ] **Step 2: Add sqlx to kunkka-core dependencies**

Add to `crates/kunkka-core/Cargo.toml` `[dependencies]` section:

```toml
sqlx.workspace = true
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p kunkka-core`
Expected: PASS (may take time to download and compile sqlx)

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/kunkka-core/Cargo.toml
git commit -m "feat: add sqlx dependency to kunkka-core"
```

---

### Task 2: Add CoreError::Database variant

**Files:**
- Modify: `crates/kunkka-core/src/error.rs`

- [ ] **Step 1: Add Database error variant**

Add to `crates/kunkka-core/src/error.rs` before the closing `}`:

```rust
    #[error("database error: {0}")]
    Database(String),
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p kunkka-core`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-core/src/error.rs
git commit -m "feat: add CoreError::Database variant"
```

---

### Task 3: Create migration file

**Files:**
- Create: `crates/kunkka-core/migrations/0001_core_metadata.sql`

- [ ] **Step 1: Create migrations directory and file**

Create `crates/kunkka-core/migrations/0001_core_metadata.sql`:

```sql
CREATE TABLE IF NOT EXISTS core_metadata (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

INSERT INTO core_metadata (key, value)
VALUES ('schema_version', '1')
ON CONFLICT(key) DO UPDATE SET value = excluded.value;
```

- [ ] **Step 2: Commit**

```bash
git add crates/kunkka-core/migrations/
git commit -m "feat: add core_metadata migration"
```

---

### Task 4: Implement CoreDatabase module

**Files:**
- Create: `crates/kunkka-core/src/database.rs`
- Modify: `crates/kunkka-core/src/lib.rs`
- Test: `crates/kunkka-core/tests/database.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/kunkka-core/tests/database.rs`:

```rust
use kunkka_core::database::CoreDatabase;
use kunkka_core::xdg::KunkkaPaths;
use tempfile::tempdir;

fn test_paths() -> (tempfile::TempDir, KunkkaPaths) {
    let root = tempdir().unwrap();
    let paths = KunkkaPaths {
        config_dir: root.path().join("config"),
        data_dir: root.path().join("data"),
        state_dir: root.path().join("state"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
        database_path: root.path().join("data/kunkka.db"),
        log_dir: root.path().join("state/logs"),
        socket_path: root.path().join("runtime/core.sock"),
    };
    (root, paths)
}

#[tokio::test]
async fn connect_creates_database_file() {
    let (_root, paths) = test_paths();
    let _db = CoreDatabase::connect(&paths).await.unwrap();
    assert!(paths.database_path.exists());
}

#[tokio::test]
async fn schema_version_returns_one() {
    let (_root, paths) = test_paths();
    let db = CoreDatabase::connect(&paths).await.unwrap();
    assert_eq!(db.schema_version().await.unwrap(), 1);
}

#[tokio::test]
async fn connect_is_idempotent() {
    let (_root, paths) = test_paths();
    let _db1 = CoreDatabase::connect(&paths).await.unwrap();
    let db2 = CoreDatabase::connect(&paths).await.unwrap();
    assert_eq!(db2.schema_version().await.unwrap(), 1);
}

#[tokio::test]
async fn ping_succeeds() {
    let (_root, paths) = test_paths();
    let db = CoreDatabase::connect(&paths).await.unwrap();
    db.ping().await.unwrap();
}

#[tokio::test]
async fn connect_creates_parent_directories() {
    let root = tempdir().unwrap();
    let deep_path = root.path().join("a/b/c/kunkka.db");
    let paths = KunkkaPaths {
        config_dir: root.path().join("config"),
        data_dir: root.path().join("data"),
        state_dir: root.path().join("state"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
        database_path: deep_path.clone(),
        log_dir: root.path().join("state/logs"),
        socket_path: root.path().join("runtime/core.sock"),
    };
    let _db = CoreDatabase::connect(&paths).await.unwrap();
    assert!(deep_path.exists());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kunkka-core --test database`
Expected: FAIL with "module `database` does not exist"

- [ ] **Step 3: Implement database.rs**

Create `crates/kunkka-core/src/database.rs`:

```rust
use crate::xdg::KunkkaPaths;
use crate::{CoreError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Connection, Executor, Row, SqlitePool};
use std::str::FromStr;

pub struct CoreDatabase {
    pool: SqlitePool,
}

impl CoreDatabase {
    pub async fn connect(paths: &KunkkaPaths) -> Result<Self> {
        if let Some(parent) = paths.database_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options =
            SqliteConnectOptions::from_str(&paths.database_path.to_string_lossy())
                .map_err(|err| CoreError::Database(format!("invalid database path: {err}")))?
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .pragma("foreign_keys", "ON");

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|err| CoreError::Database(format!("failed to connect: {err}")))?;

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    pub async fn schema_version(&self) -> Result<i64> {
        let row = sqlx::query("SELECT value FROM core_metadata WHERE key = 'schema_version'")
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| CoreError::Database(format!("failed to query schema_version: {err}")))?
            .ok_or_else(|| CoreError::Database("schema_version not found".to_string()))?;

        let value: String = row
            .try_get("value")
            .map_err(|err| CoreError::Database(format!("failed to read schema_version: {err}")))?;

        value
            .parse::<i64>()
            .map_err(|_| CoreError::Database(format!("invalid schema_version: {value}")))
    }

    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|err| CoreError::Database(format!("ping failed: {err}")))?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("migrations")
            .run(&self.pool)
            .await
            .map_err(|err| CoreError::Database(format!("migration failed: {err}")))?;
        Ok(())
    }
}
```

- [ ] **Step 4: Register module in lib.rs**

Add to `crates/kunkka-core/src/lib.rs`:

```rust
pub mod database;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p kunkka-core --test database`
Expected: All 5 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/kunkka-core/src/database.rs crates/kunkka-core/src/lib.rs crates/kunkka-core/tests/database.rs
git commit -m "feat: add CoreDatabase module"
```

---

### Task 5: Integrate CoreDatabase into CoreRuntime

**Files:**
- Modify: `crates/kunkka-core/src/runtime.rs`
- Test: `crates/kunkka-core/tests/database.rs` (add test)

- [ ] **Step 1: Add database field to CoreRuntime**

In `crates/kunkka-core/src/runtime.rs`, change the struct definition:

```rust
use crate::database::CoreDatabase;

pub struct CoreRuntime {
    server: CoreIpcServer,
    worker_manager: WorkerManager,
    _database: CoreDatabase,
}
```

- [ ] **Step 2: Update CoreRuntime::prepare()**

Change the `prepare` method:

```rust
    pub async fn prepare(paths: &KunkkaPaths) -> Result<Self> {
        paths.ensure_dirs()?;
        let database = CoreDatabase::connect(paths).await?;
        let server = CoreIpcServer::bind(paths).await?;
        let app_registry = AppRegistry::load(paths)?;

        Ok(Self {
            server,
            worker_manager: WorkerManager::with_app_registry(
                app_registry,
                paths.socket_path.clone(),
            ),
            _database: database,
        })
    }
```

- [ ] **Step 3: Add runtime integration test**

Add to `crates/kunkka-core/tests/database.rs`:

```rust
use kunkka_core::prepare_core_runtime;

#[tokio::test]
async fn core_runtime_prepare_initializes_database() {
    let (_root, paths) = test_paths();
    let runtime = prepare_core_runtime(&paths).await.unwrap();
    // Database is initialized as part of runtime lifecycle
    // Verify runtime prepared successfully by checking worker_manager
    assert_eq!(runtime.worker_manager().active_worker_count(), 0);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p kunkka-core --test database`
Expected: All 6 tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-core/src/runtime.rs crates/kunkka-core/tests/database.rs
git commit -m "feat: integrate CoreDatabase into CoreRuntime"
```

---

### Task 6: Verify existing tests still pass

**Files:**
- (none)

- [ ] **Step 1: Run all kunkka-core tests**

Run: `cargo test -p kunkka-core`
Expected: All tests PASS (existing + new)

- [ ] **Step 2: Run workspace tests**

Run: `cargo test --workspace`
Expected: All tests PASS

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 4: Run fmt check**

Run: `cargo fmt --all --check`
Expected: PASS

---

### Task 7: Update documentation

**Files:**
- Modify: `docs/storage.md`
- Modify: `docs/architecture.md`
- Modify: `docs/development-log.md`

- [ ] **Step 1: Update storage.md**

Add to `docs/storage.md` before the "Current Implementation" section:

```markdown
## Core Database

`kunkka-core` owns the core SQLite database at `$XDG_DATA_HOME/kunkka/kunkka.db`.

Current implementation:

- `crates/kunkka-core/src/database.rs`: `CoreDatabase` with `connect()`, `schema_version()`, `ping()`, `pool()`.
- `crates/kunkka-core/migrations/`: embedded SQL migrations run at startup.
- First migration creates `core_metadata` table with `schema_version` = `1`.
- SQLite pragmas: `foreign_keys = ON`, `journal_mode = WAL`.
```

- [ ] **Step 2: Update architecture.md**

Add to the `kunkka-core` bullet in "当前实现切片":

```markdown
- `kunkka-core`：...、manifest-based frontend dispatch permissions、SQLite/sqlx core database foundation。
```

- [ ] **Step 3: Update development-log.md**

Add to the top of `docs/development-log.md`:

```markdown
### Core Database Foundation

Implemented:

- `crates/kunkka-core/src/database.rs` with `CoreDatabase` struct.
- SQLite connection pool via sqlx with `runtime-tokio` and `sqlite` features.
- Embedded migrations via `sqlx::migrate!()`.
- First migration creates `core_metadata` table with `schema_version` = `1`.
- SQLite pragmas: `foreign_keys = ON`, `journal_mode = WAL`.
- Integrated into `CoreRuntime::prepare()` lifecycle.

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
```

- [ ] **Step 4: Run workspace verification**

Run:
```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add docs/storage.md docs/architecture.md docs/development-log.md
git commit -m "docs: add core database foundation to architecture and development log"
```
