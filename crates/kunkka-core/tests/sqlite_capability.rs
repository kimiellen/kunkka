use kunkka_core::app_manifest::{AppManifest, CapabilitiesConfig, WorkerCommand};
use kunkka_core::capability::sqlite::{
    handle_sqlite_request, SqliteCloseParams, SqliteConnectionStore, SqliteExecuteParams,
    SqliteOpenParams, SqliteQueryParams, SqliteResponse, SqliteValue,
};
use std::collections::BTreeMap;
use tempfile::tempdir;

fn make_manifest(app_id: &str) -> AppManifest {
    AppManifest {
        app_id: kunkka_worker_sdk::AppId::new(app_id),
        worker: WorkerCommand {
            program: "/usr/bin/test".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
        },
        permissions: Default::default(),
        capabilities: CapabilitiesConfig::default(),
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

fn encode_open_params(path: Option<&str>) -> Vec<u8> {
    postcard::to_stdvec(&SqliteOpenParams {
        path: path.map(|s| s.to_string()),
    })
    .unwrap()
}

fn encode_execute_params(sql: &str, params: Vec<SqliteValue>) -> Vec<u8> {
    let encoded_params: Vec<Vec<u8>> = params
        .iter()
        .map(|v| postcard::to_stdvec(v).unwrap())
        .collect();
    postcard::to_stdvec(&SqliteExecuteParams {
        sql: sql.to_string(),
        params: encoded_params,
    })
    .unwrap()
}

fn encode_query_params(sql: &str, params: Vec<SqliteValue>) -> Vec<u8> {
    let encoded_params: Vec<Vec<u8>> = params
        .iter()
        .map(|v| postcard::to_stdvec(v).unwrap())
        .collect();
    postcard::to_stdvec(&SqliteQueryParams {
        sql: sql.to_string(),
        params: encoded_params,
    })
    .unwrap()
}

fn encode_close_params() -> Vec<u8> {
    postcard::to_stdvec(&SqliteCloseParams {}).unwrap()
}

/// Decode the response bytes into `SqliteResponse`.
fn decode_response(bytes: &[u8]) -> SqliteResponse {
    postcard::from_bytes(bytes).expect("failed to decode SqliteResponse")
}

#[tokio::test]
async fn test_sqlite_open() {
    let tmp = tempdir().unwrap();
    let manifest = make_manifest("test_app");
    let mut store = SqliteConnectionStore::new();

    let params = encode_open_params(None);
    let result = handle_sqlite_request(&manifest, "open", &params, &mut store, tmp.path()).await;
    assert!(result.is_ok(), "open should succeed: {:?}", result.err());

    let resp = decode_response(&result.unwrap());
    match resp {
        SqliteResponse::Opened { path } => {
            // The default db name is "app.db"
            assert!(
                path.ends_with("app.db"),
                "path should end with app.db: {path}"
            );
            // The database file should have been created
            assert!(
                std::path::Path::new(&path).exists(),
                "database file should exist on disk"
            );
        }
        other => panic!("expected Opened response, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_sqlite_execute() {
    let tmp = tempdir().unwrap();
    let manifest = make_manifest("test_app");
    let mut store = SqliteConnectionStore::new();

    // Open a connection first
    let open_params = encode_open_params(Some("test.db"));
    let result =
        handle_sqlite_request(&manifest, "open", &open_params, &mut store, tmp.path()).await;
    assert!(result.is_ok(), "open should succeed: {:?}", result.err());

    // Create a table
    let exec_params = encode_execute_params(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        vec![],
    );
    let result =
        handle_sqlite_request(&manifest, "execute", &exec_params, &mut store, tmp.path()).await;
    assert!(
        result.is_ok(),
        "execute create table should succeed: {:?}",
        result.err()
    );

    let resp = decode_response(&result.unwrap());
    match resp {
        SqliteResponse::Executed { rows_affected } => {
            assert_eq!(rows_affected, 0, "CREATE TABLE should affect 0 rows");
        }
        other => panic!("expected Executed response, got: {other:?}"),
    }

    // Insert data with bound parameters
    let insert_params = encode_execute_params(
        "INSERT INTO items (id, name) VALUES (?1, ?2)",
        vec![
            SqliteValue::Integer(1),
            SqliteValue::Text("hello".to_string()),
        ],
    );
    let result =
        handle_sqlite_request(&manifest, "execute", &insert_params, &mut store, tmp.path()).await;
    assert!(
        result.is_ok(),
        "execute insert should succeed: {:?}",
        result.err()
    );

    let resp = decode_response(&result.unwrap());
    match resp {
        SqliteResponse::Executed { rows_affected } => {
            assert_eq!(rows_affected, 1, "INSERT should affect 1 row");
        }
        other => panic!("expected Executed response, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_sqlite_query() {
    let tmp = tempdir().unwrap();
    let manifest = make_manifest("test_app");
    let mut store = SqliteConnectionStore::new();

    // Open
    let open_params = encode_open_params(None);
    let result =
        handle_sqlite_request(&manifest, "open", &open_params, &mut store, tmp.path()).await;
    assert!(result.is_ok());

    // Create table and insert data
    let exec_params = encode_execute_params(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        vec![],
    );
    let result =
        handle_sqlite_request(&manifest, "execute", &exec_params, &mut store, tmp.path()).await;
    assert!(result.is_ok());

    let insert_params = encode_execute_params(
        "INSERT INTO users (id, name) VALUES (?1, ?2)",
        vec![
            SqliteValue::Integer(1),
            SqliteValue::Text("Alice".to_string()),
        ],
    );
    let result =
        handle_sqlite_request(&manifest, "execute", &insert_params, &mut store, tmp.path()).await;
    assert!(result.is_ok());

    let insert_params = encode_execute_params(
        "INSERT INTO users (id, name) VALUES (?1, ?2)",
        vec![
            SqliteValue::Integer(2),
            SqliteValue::Text("Bob".to_string()),
        ],
    );
    let result =
        handle_sqlite_request(&manifest, "execute", &insert_params, &mut store, tmp.path()).await;
    assert!(result.is_ok());

    // Query all rows
    let query_params = encode_query_params("SELECT id, name FROM users ORDER BY id", vec![]);
    let result =
        handle_sqlite_request(&manifest, "query", &query_params, &mut store, tmp.path()).await;
    assert!(result.is_ok(), "query should succeed: {:?}", result.err());

    let resp = decode_response(&result.unwrap());
    match resp {
        SqliteResponse::Queried { columns, rows } => {
            assert_eq!(columns, vec!["id", "name"]);
            assert_eq!(rows.len(), 2, "should have 2 rows");

            // Decode the first row: id=1, name="Alice"
            let id_bytes = rows[0][0].as_ref().expect("id should not be null");
            let id: i64 = postcard::from_bytes(id_bytes).unwrap();
            assert_eq!(id, 1);

            let name_bytes = rows[0][1].as_ref().expect("name should not be null");
            let name: String = postcard::from_bytes(name_bytes).unwrap();
            assert_eq!(name, "Alice");

            // Decode the second row: id=2, name="Bob"
            let id_bytes = rows[1][0].as_ref().expect("id should not be null");
            let id: i64 = postcard::from_bytes(id_bytes).unwrap();
            assert_eq!(id, 2);

            let name_bytes = rows[1][1].as_ref().expect("name should not be null");
            let name: String = postcard::from_bytes(name_bytes).unwrap();
            assert_eq!(name, "Bob");
        }
        other => panic!("expected Queried response, got: {other:?}"),
    }

    // Query with bound parameters
    let query_params = encode_query_params(
        "SELECT id, name FROM users WHERE id = ?1",
        vec![SqliteValue::Integer(2)],
    );
    let result =
        handle_sqlite_request(&manifest, "query", &query_params, &mut store, tmp.path()).await;
    assert!(result.is_ok());

    let resp = decode_response(&result.unwrap());
    match resp {
        SqliteResponse::Queried { columns, rows } => {
            assert_eq!(columns, vec!["id", "name"]);
            assert_eq!(rows.len(), 1, "should have 1 row");
            let name_bytes = rows[0][1].as_ref().expect("name should not be null");
            let name: String = postcard::from_bytes(name_bytes).unwrap();
            assert_eq!(name, "Bob");
        }
        other => panic!("expected Queried response, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_sqlite_close() {
    let tmp = tempdir().unwrap();
    let manifest = make_manifest("test_app");
    let mut store = SqliteConnectionStore::new();

    // Open
    let open_params = encode_open_params(Some("close_test.db"));
    let result =
        handle_sqlite_request(&manifest, "open", &open_params, &mut store, tmp.path()).await;
    assert!(result.is_ok());

    // Close
    let close_params = encode_close_params();
    let result =
        handle_sqlite_request(&manifest, "close", &close_params, &mut store, tmp.path()).await;
    assert!(result.is_ok(), "close should succeed: {:?}", result.err());

    let resp = decode_response(&result.unwrap());
    assert!(
        matches!(resp, SqliteResponse::Closed),
        "expected Closed response, got: {resp:?}"
    );

    // After close, query should fail with "not_open"
    let query_params = encode_query_params("SELECT 1", vec![]);
    let result =
        handle_sqlite_request(&manifest, "query", &query_params, &mut store, tmp.path()).await;
    assert!(result.is_err(), "query after close should fail");
    let err = result.unwrap_err();
    assert_eq!(err.code, "not_open");
}

#[tokio::test]
async fn test_sqlite_not_open() {
    let tmp = tempdir().unwrap();
    let manifest = make_manifest("test_app");
    let mut store = SqliteConnectionStore::new();

    // Try to query without opening first
    let query_params = encode_query_params("SELECT 1", vec![]);
    let result =
        handle_sqlite_request(&manifest, "query", &query_params, &mut store, tmp.path()).await;
    assert!(result.is_err(), "query before open should fail");
    let err = result.unwrap_err();
    assert_eq!(err.code, "not_open");

    // Try to execute without opening first
    let exec_params = encode_execute_params("CREATE TABLE t (id INTEGER)", vec![]);
    let result =
        handle_sqlite_request(&manifest, "execute", &exec_params, &mut store, tmp.path()).await;
    assert!(result.is_err(), "execute before open should fail");
    let err = result.unwrap_err();
    assert_eq!(err.code, "not_open");
}
