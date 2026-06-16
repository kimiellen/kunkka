use std::time::Duration;

use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_tui::client;

fn test_paths() -> (tempfile::TempDir, KunkkaPaths) {
    let root = tempfile::tempdir().unwrap();
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
async fn tui_ping_returns_pong() {
    let (_tmp, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move { client::ping_core(&socket_path).await }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let result = cli_task.await.unwrap();
    assert!(result.is_ok(), "ping should succeed: {:?}", result.err());
}
