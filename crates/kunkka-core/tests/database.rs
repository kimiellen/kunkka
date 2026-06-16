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
