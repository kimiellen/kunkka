use kunkka_core::prepare_core_server;
use kunkka_core::xdg::KunkkaPaths;
use tempfile::{tempdir, TempDir};

fn test_paths() -> (TempDir, KunkkaPaths) {
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
async fn prepare_core_server_creates_dirs_and_binds_socket() {
    let (_root, paths) = test_paths();

    let server = prepare_core_server(&paths).await.unwrap();

    assert!(paths.config_dir.exists());
    assert!(paths.data_dir.exists());
    assert!(paths.state_dir.exists());
    assert!(paths.cache_dir.exists());
    assert!(paths.runtime_dir.exists());
    assert!(paths.log_dir.exists());
    assert!(paths.socket_path.exists());
    assert_eq!(server.socket_path(), paths.socket_path.as_path());
}
