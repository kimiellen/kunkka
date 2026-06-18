use crate::app_manifest::AppManifest;
use crate::capability::CapabilityError;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::{Column, Row, TypeInfo};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// SQLite connection pool per app_id.
#[derive(Debug)]
pub struct SqliteConnectionStore {
    connections: HashMap<String, SqlitePool>,
}

impl SqliteConnectionStore {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    fn get(&self, app_id: &str) -> Option<&SqlitePool> {
        self.connections.get(app_id)
    }

    fn insert(&mut self, app_id: String, pool: SqlitePool) {
        self.connections.insert(app_id, pool);
    }

    fn remove(&mut self, app_id: &str) -> Option<SqlitePool> {
        self.connections.remove(app_id)
    }
}

impl Default for SqliteConnectionStore {
    fn default() -> Self {
        Self::new()
    }
}

/// A postcard-compatible SQLite value type for parameter binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SqliteValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

// Protocol types for sqlite capability

#[derive(Debug, Serialize, Deserialize)]
pub struct SqliteOpenParams {
    /// Optional database path relative to app-data/<app_id>/; defaults to "app.db"
    pub path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SqliteQueryParams {
    pub sql: String,
    /// Each param is a postcard-encoded SqliteValue
    #[serde(default)]
    pub params: Vec<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SqliteExecuteParams {
    pub sql: String,
    /// Each param is a postcard-encoded SqliteValue
    #[serde(default)]
    pub params: Vec<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SqliteCloseParams {
    // no fields
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SqliteResponse {
    Opened {
        path: String,
    },
    Queried {
        columns: Vec<String>,
        rows: Vec<Vec<Option<Vec<u8>>>>,
    },
    Executed {
        rows_affected: u64,
    },
    Closed,
}

/// Decode postcard-encoded SqliteValue params for SQL binding.
fn decode_param_values(raw_params: &[Vec<u8>]) -> Result<Vec<SqliteValue>, CapabilityError> {
    let mut values = Vec::with_capacity(raw_params.len());
    for (i, bytes) in raw_params.iter().enumerate() {
        let val: SqliteValue = postcard::from_bytes(bytes).map_err(|e| CapabilityError {
            code: "invalid_params".to_string(),
            message: format!("invalid param at index {i}: {e}"),
        })?;
        values.push(val);
    }
    Ok(values)
}

/// Encode a single SQLite column value into postcard bytes.
fn encode_cell_value(raw_value: &sqlx::sqlite::SqliteRow, col_idx: usize) -> Option<Vec<u8>> {
    // Try to get the value as different types based on the column type info
    let col = &raw_value.columns()[col_idx];
    let type_name = col.type_info().name();

    match type_name {
        "NULL" => None,
        "INTEGER" => {
            let val: Option<i64> = raw_value.get(col_idx);
            val.map(|v| postcard::to_stdvec(&v).unwrap_or_default())
        }
        "REAL" => {
            let val: Option<f64> = raw_value.get(col_idx);
            val.map(|v| postcard::to_stdvec(&v).unwrap_or_default())
        }
        "TEXT" => {
            let val: Option<String> = raw_value.get(col_idx);
            val.map(|v| postcard::to_stdvec(&v).unwrap_or_default())
        }
        "BLOB" => {
            let val: Option<Vec<u8>> = raw_value.get(col_idx);
            val
        }
        _ => {
            // Fallback: try as string
            let val: Option<String> = raw_value.get(col_idx);
            val.map(|v| postcard::to_stdvec(&v).unwrap_or_default())
        }
    }
}

/// Build the database path for an app.
fn app_db_path(data_dir: &Path, app_id: &str, db_name: &str) -> PathBuf {
    data_dir.join("app-data").join(app_id).join(db_name)
}

/// Ensure the parent directory for the database exists.
fn ensure_db_dir(db_path: &Path) -> Result<(), CapabilityError> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CapabilityError {
            code: "io_error".to_string(),
            message: format!("failed to create database directory: {e}"),
        })?;
    }
    Ok(())
}

/// Open a new SQLite connection with WAL mode and foreign keys enabled.
async fn open_connection(db_path: &Path) -> Result<SqlitePool, CapabilityError> {
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .map_err(|e| CapabilityError {
            code: "connection_error".to_string(),
            message: format!("failed to open database: {e}"),
        })?;

    // Set pragmas
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await
        .map_err(|e| CapabilityError {
            code: "connection_error".to_string(),
            message: format!("failed to set WAL mode: {e}"),
        })?;

    sqlx::query("PRAGMA synchronous=FULL")
        .execute(&pool)
        .await
        .map_err(|e| CapabilityError {
            code: "connection_error".to_string(),
            message: format!("failed to set synchronous mode: {e}"),
        })?;

    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&pool)
        .await
        .map_err(|e| CapabilityError {
            code: "connection_error".to_string(),
            message: format!("failed to enable foreign keys: {e}"),
        })?;

    Ok(pool)
}

/// Bind `SqliteValue` parameters to a SQL query.
async fn bind_and_query(
    pool: &SqlitePool,
    sql: &str,
    params: Vec<SqliteValue>,
) -> Result<(Vec<String>, Vec<Vec<Option<Vec<u8>>>>), CapabilityError> {
    let mut query = sqlx::query(sql);
    for val in params {
        match val {
            SqliteValue::Null => {
                query = query.bind(None::<i64>);
            }
            SqliteValue::Integer(i) => {
                query = query.bind(i);
            }
            SqliteValue::Real(f) => {
                query = query.bind(f);
            }
            SqliteValue::Text(s) => {
                query = query.bind(s);
            }
            SqliteValue::Blob(b) => {
                query = query.bind(b);
            }
        }
    }

    let rows = query.fetch_all(pool).await.map_err(|e| CapabilityError {
        code: "query_error".to_string(),
        message: format!("query failed: {e}"),
    })?;

    if rows.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let columns: Vec<String> = rows[0]
        .columns()
        .iter()
        .map(|c| c.name().to_string())
        .collect();

    let mut result_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut row_values = Vec::with_capacity(columns.len());
        for col_idx in 0..columns.len() {
            row_values.push(encode_cell_value(row, col_idx));
        }
        result_rows.push(row_values);
    }

    Ok((columns, result_rows))
}

/// Bind parameters and execute a statement (non-query).
async fn bind_and_execute(
    pool: &SqlitePool,
    sql: &str,
    params: Vec<SqliteValue>,
) -> Result<u64, CapabilityError> {
    let mut query = sqlx::query(sql);
    for val in params {
        match val {
            SqliteValue::Null => {
                query = query.bind(None::<i64>);
            }
            SqliteValue::Integer(i) => {
                query = query.bind(i);
            }
            SqliteValue::Real(f) => {
                query = query.bind(f);
            }
            SqliteValue::Text(s) => {
                query = query.bind(s);
            }
            SqliteValue::Blob(b) => {
                query = query.bind(b);
            }
        }
    }

    let result = query.execute(pool).await.map_err(|e| CapabilityError {
        code: "execute_error".to_string(),
        message: format!("execute failed: {e}"),
    })?;

    Ok(result.rows_affected())
}

/// Main handler for sqlite capability requests.
pub async fn handle_sqlite_request(
    manifest: &AppManifest,
    method: &str,
    params: &[u8],
    connection_store: &mut SqliteConnectionStore,
    data_dir: &Path,
) -> Result<Vec<u8>, CapabilityError> {
    match method {
        "open" => {
            let p: SqliteOpenParams =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("invalid params: {e}"),
                })?;

            let db_name = p.path.as_deref().unwrap_or("app.db");
            let db_path = app_db_path(data_dir, manifest.app_id.as_str(), db_name);

            // Security: prevent path traversal
            let app_data_dir = data_dir.join("app-data").join(manifest.app_id.as_str());
            if !db_path.starts_with(&app_data_dir) {
                return Err(CapabilityError {
                    code: "permission_denied".to_string(),
                    message: "database path must be within app-data directory".to_string(),
                });
            }

            ensure_db_dir(&db_path)?;
            let pool = open_connection(&db_path).await?;
            let path_str = db_path.to_string_lossy().into_owned();
            connection_store.insert(manifest.app_id.as_str().to_string(), pool);

            postcard::to_stdvec(&SqliteResponse::Opened { path: path_str }).map_err(|e| {
                CapabilityError {
                    code: "io_error".to_string(),
                    message: format!("encode result: {e}"),
                }
            })
        }
        "query" => {
            let p: SqliteQueryParams =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("invalid params: {e}"),
                })?;

            let pool = connection_store
                .get(manifest.app_id.as_str())
                .ok_or_else(|| CapabilityError {
                    code: "not_open".to_string(),
                    message: "database is not open; call 'open' first".to_string(),
                })?;

            let values = decode_param_values(&p.params)?;
            let (columns, rows) = bind_and_query(pool, &p.sql, values).await?;

            postcard::to_stdvec(&SqliteResponse::Queried { columns, rows }).map_err(|e| {
                CapabilityError {
                    code: "io_error".to_string(),
                    message: format!("encode result: {e}"),
                }
            })
        }
        "execute" => {
            let p: SqliteExecuteParams =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("invalid params: {e}"),
                })?;

            let pool = connection_store
                .get(manifest.app_id.as_str())
                .ok_or_else(|| CapabilityError {
                    code: "not_open".to_string(),
                    message: "database is not open; call 'open' first".to_string(),
                })?;

            let values = decode_param_values(&p.params)?;
            let rows_affected = bind_and_execute(pool, &p.sql, values).await?;

            postcard::to_stdvec(&SqliteResponse::Executed { rows_affected }).map_err(|e| {
                CapabilityError {
                    code: "io_error".to_string(),
                    message: format!("encode result: {e}"),
                }
            })
        }
        "close" => {
            let _p: SqliteCloseParams =
                postcard::from_bytes(params).map_err(|e| CapabilityError {
                    code: "invalid_params".to_string(),
                    message: format!("invalid params: {e}"),
                })?;

            if let Some(pool) = connection_store.remove(manifest.app_id.as_str()) {
                pool.close().await;
            }

            postcard::to_stdvec(&SqliteResponse::Closed).map_err(|e| CapabilityError {
                code: "io_error".to_string(),
                message: format!("encode result: {e}"),
            })
        }
        _ => Err(CapabilityError {
            code: "unknown_method".to_string(),
            message: format!("unknown sqlite method: {method}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_response_opened_encode_decode() {
        let resp = SqliteResponse::Opened {
            path: "/tmp/test.db".to_string(),
        };
        let bytes = postcard::to_stdvec(&resp).unwrap();
        let decoded: SqliteResponse = postcard::from_bytes(&bytes).unwrap();
        match decoded {
            SqliteResponse::Opened { path } => assert_eq!(path, "/tmp/test.db"),
            _ => panic!("expected Opened"),
        }
    }

    #[test]
    fn test_sqlite_response_queried_encode_decode() {
        let resp = SqliteResponse::Queried {
            columns: vec!["id".to_string(), "name".to_string()],
            rows: vec![
                vec![
                    Some(postcard::to_stdvec(&1i64).unwrap()),
                    Some(postcard::to_stdvec(&"hello".to_string()).unwrap()),
                ],
                vec![Some(postcard::to_stdvec(&2i64).unwrap()), None],
            ],
        };
        let bytes = postcard::to_stdvec(&resp).unwrap();
        let decoded: SqliteResponse = postcard::from_bytes(&bytes).unwrap();
        match decoded {
            SqliteResponse::Queried { columns, rows } => {
                assert_eq!(columns, vec!["id", "name"]);
                assert_eq!(rows.len(), 2);
                assert!(rows[1][1].is_none());
            }
            _ => panic!("expected Queried"),
        }
    }

    #[test]
    fn test_sqlite_response_executed_encode_decode() {
        let resp = SqliteResponse::Executed { rows_affected: 42 };
        let bytes = postcard::to_stdvec(&resp).unwrap();
        let decoded: SqliteResponse = postcard::from_bytes(&bytes).unwrap();
        match decoded {
            SqliteResponse::Executed { rows_affected } => assert_eq!(rows_affected, 42),
            _ => panic!("expected Executed"),
        }
    }

    #[test]
    fn test_sqlite_response_closed_encode_decode() {
        let resp = SqliteResponse::Closed;
        let bytes = postcard::to_stdvec(&resp).unwrap();
        let decoded: SqliteResponse = postcard::from_bytes(&bytes).unwrap();
        assert!(matches!(decoded, SqliteResponse::Closed));
    }

    #[test]
    fn test_sqlite_query_params_encode_decode() {
        let p = SqliteQueryParams {
            sql: "SELECT * FROM users WHERE id = ?1".to_string(),
            params: vec![postcard::to_stdvec(&SqliteValue::Integer(1)).unwrap()],
        };
        let bytes = postcard::to_stdvec(&p).unwrap();
        let decoded: SqliteQueryParams = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.sql, "SELECT * FROM users WHERE id = ?1");
        assert_eq!(decoded.params.len(), 1);
    }

    #[test]
    fn test_sqlite_execute_params_encode_decode() {
        let p = SqliteExecuteParams {
            sql: "INSERT INTO users (name) VALUES (?1)".to_string(),
            params: vec![postcard::to_stdvec(&SqliteValue::Text("Alice".to_string())).unwrap()],
        };
        let bytes = postcard::to_stdvec(&p).unwrap();
        let decoded: SqliteExecuteParams = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.sql, "INSERT INTO users (name) VALUES (?1)");
        assert_eq!(decoded.params.len(), 1);
    }

    #[test]
    fn test_decode_param_values() {
        let params = vec![
            postcard::to_stdvec(&SqliteValue::Integer(42)).unwrap(),
            postcard::to_stdvec(&SqliteValue::Text("hello".to_string())).unwrap(),
            postcard::to_stdvec(&SqliteValue::Real(std::f64::consts::PI)).unwrap(),
            postcard::to_stdvec(&SqliteValue::Null).unwrap(),
            postcard::to_stdvec(&SqliteValue::Blob(vec![1, 2, 3])).unwrap(),
        ];
        let values = decode_param_values(&params).unwrap();
        assert_eq!(values.len(), 5);
        assert!(matches!(&values[0], SqliteValue::Integer(42)));
        assert!(matches!(&values[1], SqliteValue::Text(s) if s == "hello"));
        assert!(
            matches!(&values[2], SqliteValue::Real(f) if (*f - std::f64::consts::PI).abs() < f64::EPSILON)
        );
        assert!(matches!(&values[3], SqliteValue::Null));
        assert!(matches!(&values[4], SqliteValue::Blob(b) if b == &[1, 2, 3]));
    }

    #[test]
    fn test_app_db_path() {
        let data_dir = PathBuf::from("/home/user/.local/share/kunkka");
        let path = app_db_path(&data_dir, "notes", "app.db");
        assert_eq!(
            path,
            PathBuf::from("/home/user/.local/share/kunkka/app-data/notes/app.db")
        );
    }

    #[test]
    fn test_connection_store() {
        let mut store = SqliteConnectionStore::new();
        assert!(store.get("nonexistent").is_none());
        assert!(store.remove("nonexistent").is_none());
    }

    #[test]
    fn test_sqlite_value_encode_decode() {
        let values = vec![
            SqliteValue::Null,
            SqliteValue::Integer(42),
            SqliteValue::Real(std::f64::consts::PI),
            SqliteValue::Text("hello".to_string()),
            SqliteValue::Blob(vec![1, 2, 3]),
        ];
        for val in values {
            let bytes = postcard::to_stdvec(&val).unwrap();
            let decoded: SqliteValue = postcard::from_bytes(&bytes).unwrap();
            match (&val, &decoded) {
                (SqliteValue::Null, SqliteValue::Null) => {}
                (SqliteValue::Integer(a), SqliteValue::Integer(b)) => assert_eq!(a, b),
                (SqliteValue::Real(a), SqliteValue::Real(b)) => {
                    assert!((a - b).abs() < f64::EPSILON)
                }
                (SqliteValue::Text(a), SqliteValue::Text(b)) => assert_eq!(a, b),
                (SqliteValue::Blob(a), SqliteValue::Blob(b)) => assert_eq!(a, b),
                _ => panic!("type mismatch"),
            }
        }
    }
}
