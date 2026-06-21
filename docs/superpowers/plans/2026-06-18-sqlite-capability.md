# SQLite Capability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement SQLite capability that allows workers to create and manage their own SQLite database files with open/query/execute/close operations.

**Architecture:** Reuse `kunkka.capability.v1` schema with `capability = "sqlite"`. Use sqlx (already in dependencies) for SQLite operations. Short connection model, WAL mode, synchronous=FULL.

**Tech Stack:** Rust, sqlx (SQLite), postcard, serde

---

### Task 1: Implement SQLite capability types and handler

**Covers:** S2, S3, S4

**Files:**
- Create: `crates/kunkka-core/src/capability/sqlite.rs`
- Modify: `crates/kunkka-core/src/capability/mod.rs`

- [ ] **Step 1: Create sqlite.rs with types and handler**

Create `crates/kunkka-core/src/capability/sqlite.rs`:

```rust
use crate::app_manifest::AppManifest;
use crate::capability::CapabilityError;
use crate::xdg::KunkkaPaths;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SqliteRequest {
    Open,
    Query { sql: String, params: Vec<Vec<u8>> },
    Execute { sql: String, params: Vec<Vec<u8>> },
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SqliteResponse {
    Opened { path: String },
    Queried { columns: Vec<String>, rows: Vec<Vec<Option<Vec<u8>>>> },
    Executed { rows_affected: u64 },
    Closed,
}

pub struct SqliteConnection {
    pool: Option<SqlitePool>,
    app_id: String,
}

impl SqliteConnection {
    pub fn new(app_id: String) -> Self {
        Self { pool: None, app_id }
    }

    async fn open(&mut self, data_dir: &std::path::Path) -> Result<String, CapabilityError> {
        let db_dir = data_dir.join("app-data").join(&self.app_id);
        std::fs::create_dir_all(&db_dir).map_err(|e| CapabilityError {
            code: "io_error".to_string(),
            message: format!("failed to create database directory: {e}"),
        })?;

        let db_path = db_dir.join("app.db");
        let path_str = db_path.to_string_lossy().to_string();

        let options = SqliteConnectOptions::from_str(&path_str)
            .map_err(|e| CapabilityError {
                code: "io_error".to_string(),
                message: format!("invalid database path: {e}"),
            })?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .pragma("synchronous", "FULL")
            .pragma("foreign_keys", "ON");

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|e| CapabilityError {
                code: "database_error".to_string(),
                message: format!("failed to connect: {e}"),
            })?;

        self.pool = Some(pool);
        Ok(path_str)
    }

    async fn query(&self, sql: &str, params: &[Vec<u8>]) -> Result<SqliteResponse, CapabilityError> {
        let pool = self.pool.as_ref().ok_or_else(|| CapabilityError {
            code: "not_open".to_string(),
            message: "database not open".to_string(),
        })?;

        let mut query = sqlx::query(sql);
        for param in params {
            // Decode postcard-encoded parameter
            let value: serde_json::Value = postcard::from_bytes(param).map_err(|e| CapabilityError {
                code: "invalid_params".to_string(),
                message: format!("failed to decode param: {e}"),
            })?;
            query = bind_json_value(query, value);
        }

        let rows = query.fetch_all(pool).await.map_err(|e| CapabilityError {
            code: "database_error".to_string(),
            message: format!("query failed: {e}"),
        })?;

        let columns: Vec<String> = if let Some(first) = rows.first() {
            (0..first.columns().len())
                .map(|i| first.columns()[i].name().to_string())
                .collect()
        } else {
            vec![]
        };

        let mut result_rows = Vec::new();
        for row in &rows {
            let mut row_data = Vec::new();
            for i in 0..row.columns().len() {
                let value = get_column_value(row, i)?;
                row_data.push(value);
            }
            result_rows.push(row_data);
        }

        Ok(SqliteResponse::Queried {
            columns,
            rows: result_rows,
        })
    }

    async fn execute(&self, sql: &str, params: &[Vec<u8>]) -> Result<SqliteResponse, CapabilityError> {
        let pool = self.pool.as_ref().ok_or_else(|| CapabilityError {
            code: "not_open".to_string(),
            message: "database not open".to_string(),
        })?;

        let mut query = sqlx::query(sql);
        for param in params {
            let value: serde_json::Value = postcard::from_bytes(param).map_err(|e| CapabilityError {
                code: "invalid_params".to_string(),
                message: format!("failed to decode param: {e}"),
            })?;
            query = bind_json_value(query, value);
        }

        let result = query.execute(pool).await.map_err(|e| CapabilityError {
            code: "database_error".to_string(),
            message: format!("execute failed: {e}"),
        })?;

        Ok(SqliteResponse::Executed {
            rows_affected: result.rows_affected(),
        })
    }

    async fn close(&mut self) -> Result<SqliteResponse, CapabilityError> {
        if let Some(pool) = self.pool.take() {
            pool.close().await;
        }
        Ok(SqliteResponse::Closed)
    }
}

fn bind_json_value<'a>(query: sqlx::query::Query<'a, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'a>>, value: serde_json::Value) -> sqlx::query::Query<'a, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'a>> {
    match value {
        serde_json::Value::Null => query.bind(None::<i64>),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                query.bind(i)
            } else if let Some(f) = n.as_f64() {
                query.bind(f)
            } else {
                query.bind(None::<i64>)
            }
        }
        serde_json::Value::String(s) => query.bind(s),
        serde_json::Value::Bool(b) => query.bind(b),
        _ => query.bind(value.to_string()),
    }
}

fn get_column_value(row: &sqlx::sqlite::SqliteRow, index: usize) -> Result<Option<Vec<u8>>, CapabilityError> {
    let column = &row.columns()[index];
    match column.type_info().name() {
        "NULL" => Ok(None),
        "INTEGER" => {
            let value: Option<i64> = row.try_get(index).map_err(|e| CapabilityError {
                code: "database_error".to_string(),
                message: format!("failed to read column: {e}"),
            })?;
            Ok(value.map(|v| postcard::to_stdvec(&v).unwrap()))
        }
        "REAL" => {
            let value: Option<f64> = row.try_get(index).map_err(|e| CapabilityError {
                code: "database_error".to_string(),
                message: format!("failed to read column: {e}"),
            })?;
            Ok(value.map(|v| postcard::to_stdvec(&v).unwrap()))
        }
        "TEXT" => {
            let value: Option<String> = row.try_get(index).map_err(|e| CapabilityError {
                code: "database_error".to_string(),
                message: format!("failed to read column: {e}"),
            })?;
            Ok(value.map(|v| postcard::to_stdvec(&v).unwrap()))
        }
        "BLOB" => {
            let value: Option<Vec<u8>> = row.try_get(index).map_err(|e| CapabilityError {
                code: "database_error".to_string(),
                message: format!("failed to read column: {e}"),
            })?;
            Ok(value)
        }
        _ => Ok(None),
    }
}

pub async fn handle_sqlite_request(
    _manifest: &AppManifest,
    method: &str,
    params: &[u8],
    connection: &mut SqliteConnection,
    data_dir: &std::path::Path,
) -> Result<Vec<u8>, CapabilityError> {
    let response = match method {
        "open" => connection.open(data_dir).await.map(|path| SqliteResponse::Opened { path })?,
        "query" => {
            let params: SqliteQueryParams = postcard::from_bytes(params).map_err(|e| CapabilityError {
                code: "invalid_params".to_string(),
                message: format!("invalid params: {e}"),
            })?;
            connection.query(&params.sql, &params.params).await?
        }
        "execute" => {
            let params: SqliteExecuteParams = postcard::from_bytes(params).map_err(|e| CapabilityError {
                code: "invalid_params".to_string(),
                message: format!("invalid params: {e}"),
            })?;
            connection.execute(&params.sql, &params.params).await?
        }
        "close" => connection.close().await?,
        _ => {
            return Err(CapabilityError {
                code: "unknown_method".to_string(),
                message: format!("unknown sqlite method: {method}"),
            });
        }
    };

    postcard::to_stdvec(&response).map_err(|e| CapabilityError {
        code: "io_error".to_string(),
        message: format!("encode result: {e}"),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SqliteQueryParams {
    sql: String,
    params: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SqliteExecuteParams {
    sql: String,
    params: Vec<Vec<u8>>,
}
```

- [ ] **Step 2: Add sqlite module to mod.rs**

In `crates/kunkka-core/src/capability/mod.rs`, add:
```rust
pub mod sqlite;
```

- [ ] **Step 3: Add sqlite routing in handle_capability_inner**

In `crates/kunkka-core/src/capability/mod.rs`, modify the `handle_capability_inner` function to handle sqlite capability. Note: sqlite requires a mutable connection reference, so the routing needs to be adjusted.

The sqlite handler needs access to:
1. The app's data directory path
2. A mutable connection reference (stored per-app)

This requires modifying the capability handler signature or using a different approach.

- [ ] **Step 4: Verify build**

Run: `cargo check -p kunkka-core`
Expected: PASS

- [ ] **Step 5: Run all existing tests**

Run: `cargo test --workspace`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/kunkka-core/src/capability/sqlite.rs crates/kunkka-core/src/capability/mod.rs
git commit -m "feat: add sqlite capability handler"
```

---

### Task 2: Add SQLite capability tests

**Covers:** S2, S3, S4

**Files:**
- Create: `crates/kunkka-core/tests/sqlite_capability.rs`

- [ ] **Step 1: Create test file**

Create `crates/kunkka-core/tests/sqlite_capability.rs` with tests for:
1. `test_sqlite_open` — open creates database file
2. `test_sqlite_execute` — execute creates table and inserts data
3. `test_sqlite_query` — query returns correct data
4. `test_sqlite_close` — close releases connection
5. `test_sqlite_not_open` — query before open returns "not_open"

- [ ] **Step 2: Run tests**

Run: `cargo test -p kunkka-core --test sqlite_capability`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-core/tests/sqlite_capability.rs
git commit -m "test: add sqlite capability tests"
```

---

### Task 3: Run full verification and update docs

**Covers:** All

- [ ] **Step 1: Run formatting check**

Run: `cargo fmt --all --check`
Expected: PASS

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace`
Expected: PASS

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 4: Update development log**

Append to `docs/development-log.md`:
```markdown
## 2026-06-18

### SQLite Capability

Implemented:

- `SqliteRequest`/`SqliteResponse` protocol types with postcard codec in `capability/sqlite.rs`.
- App data directory `$XDG_DATA_HOME/kunkka/app-data/<app_id>/app.db` with auto-creation.
- SQLite pragmas: WAL mode, synchronous=FULL, foreign_keys=ON.
- SQL operations: open, query, execute, close.
- Positional parameter binding (?1, ?2, ...).
- Query results: column names + row data with postcard-encoded values.
- Error codes: invalid_params, database_error, io_error, not_open.
- Tests: open, execute, query, close, not_open scenarios.

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
```

- [ ] **Step 5: Update architecture doc**

In `docs/architecture.md`, add to the capability layer section:
```markdown
- `capability/sqlite.rs`：SQLite capability for app database management.
```

- [ ] **Step 6: Final commit**

```bash
git add docs/development-log.md docs/architecture.md
git commit -m "docs: update development log and architecture for sqlite capability"
```
