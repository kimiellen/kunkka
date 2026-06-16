use kunkka_core::database::CoreDatabase;
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_core::CoreError;
use sqlx::migrate::Migrator;
use sqlx::Row;
use std::str::FromStr;
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
async fn schema_version_returns_two() {
    let (_root, paths) = test_paths();
    let db = CoreDatabase::connect(&paths).await.unwrap();
    assert_eq!(db.schema_version().await.unwrap(), 2);
}

#[tokio::test]
async fn connect_is_idempotent() {
    let (_root, paths) = test_paths();
    let _db1 = CoreDatabase::connect(&paths).await.unwrap();
    let db2 = CoreDatabase::connect(&paths).await.unwrap();
    assert_eq!(db2.schema_version().await.unwrap(), 2);
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

#[tokio::test]
async fn core_runtime_prepare_initializes_database() {
    let (_root, paths) = test_paths();
    let runtime = prepare_core_runtime(&paths).await.unwrap();
    assert!(paths.database_path.exists());
    assert_eq!(runtime.worker_manager().active_worker_count(), 0);
}

#[tokio::test]
async fn frontend_dispatch_audit_rows_roundtrip() {
    let (_root, paths) = test_paths();
    let db = CoreDatabase::connect(&paths).await.unwrap();

    db.record_frontend_dispatch_audit("notes", "search", "allow", "allowed")
        .await
        .unwrap();

    let rows = sqlx::query(
        "SELECT app_id, method, decision, reason_code FROM frontend_dispatch_audit ORDER BY id ASC",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String, _>("app_id"), "notes");
    assert_eq!(rows[0].get::<String, _>("method"), "search");
    assert_eq!(rows[0].get::<String, _>("decision"), "allow");
    assert_eq!(rows[0].get::<String, _>("reason_code"), "allowed");

    db.record_frontend_dispatch_audit("unknown", "method", "deny", "app_not_found")
        .await
        .unwrap();

    let rows = sqlx::query(
        "SELECT app_id, method, decision, reason_code FROM frontend_dispatch_audit ORDER BY id ASC",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("app_id"), "notes");
    assert_eq!(rows[0].get::<String, _>("decision"), "allow");
    assert_eq!(rows[1].get::<String, _>("app_id"), "unknown");
    assert_eq!(rows[1].get::<String, _>("decision"), "deny");
    assert_eq!(rows[1].get::<String, _>("reason_code"), "app_not_found");
}

#[tokio::test]
async fn record_audit_rejects_invalid_reason_code() {
    let (_root, paths) = test_paths();
    let db = CoreDatabase::connect(&paths).await.unwrap();

    let err = db
        .record_frontend_dispatch_audit("notes", "search", "allow", "invalid_reason")
        .await
        .unwrap_err();

    match err {
        CoreError::Database(msg) => assert!(msg.contains("invalid reason_code")),
        other => panic!("expected CoreError::Database, got {other:?}"),
    }
}

#[tokio::test]
async fn record_audit_rejects_invalid_decision() {
    let (_root, paths) = test_paths();
    let db = CoreDatabase::connect(&paths).await.unwrap();

    let err = db
        .record_frontend_dispatch_audit("notes", "search", "invalid", "allowed")
        .await
        .unwrap_err();

    match err {
        CoreError::Database(msg) => assert!(msg.contains("invalid decision")),
        other => panic!("expected CoreError::Database, got {other:?}"),
    }
}

#[tokio::test]
async fn upgrade_v1_to_v2_creates_audit_table() {
    let root = tempdir().unwrap();
    let db_path = root.path().join("data/kunkka.db");
    let migrations_dir = root.path().join("migrations");
    std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    std::fs::create_dir_all(&migrations_dir).unwrap();
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/0001_core_metadata.sql"),
        migrations_dir.join("0001_core_metadata.sql"),
    )
    .unwrap();

    {
        use sqlx::sqlite::SqliteConnectOptions;
        let options = SqliteConnectOptions::from_str(db_path.to_str().unwrap())
            .unwrap()
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .pragma("foreign_keys", "ON");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();

        Migrator::new(migrations_dir.as_path())
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();

        let applied =
            sqlx::query_scalar::<_, i64>("SELECT version FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(applied, vec![1]);

        let schema_version = sqlx::query_scalar::<_, String>(
            "SELECT value FROM core_metadata WHERE key = 'schema_version'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(schema_version, "1");

        pool.close().await;
    }

    let paths = KunkkaPaths {
        config_dir: root.path().join("config"),
        data_dir: root.path().join("data"),
        state_dir: root.path().join("state"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
        database_path: db_path,
        log_dir: root.path().join("state/logs"),
        socket_path: root.path().join("runtime/core.sock"),
    };
    let db = CoreDatabase::connect(&paths).await.unwrap();
    assert_eq!(db.schema_version().await.unwrap(), 2);

    db.record_frontend_dispatch_audit("notes", "search", "allow", "allowed")
        .await
        .unwrap();
    let rows = sqlx::query(
        "SELECT app_id, method, decision, reason_code FROM frontend_dispatch_audit ORDER BY id ASC",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String, _>("decision"), "allow");
}
