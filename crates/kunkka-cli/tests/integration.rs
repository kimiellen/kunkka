use kunkka_cli::cli::{Cli, CliCommand};
use kunkka_cli::run_command_with_socket;
use kunkka_core::prepare_core_runtime;
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
async fn cli_ping_returns_pong() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Ping,
            };
            run_command_with_socket(&cli, &socket_path).await
        }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let result = cli_task.await.unwrap().unwrap();
    assert!(result.is_success());
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::json!({"ok":true,"result":{"type":"pong"}})
    );
}

#[tokio::test]
async fn cli_status_returns_status() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Status,
            };
            run_command_with_socket(&cli, &socket_path).await
        }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let result = cli_task.await.unwrap().unwrap();
    assert!(result.is_success());
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["result"]["type"], "status");
    assert_eq!(value["result"]["worker_count"], 0);
    assert!(value["result"]["socket_path"].as_str().is_some());
    assert_eq!(value["result"]["runtime_ready"], true);
}

#[tokio::test]
async fn cli_core_unavailable_returns_error() {
    let root = tempdir().unwrap();
    let socket_path = root.path().join("nonexistent.sock");

    let cli = Cli {
        command: CliCommand::Ping,
    };
    let result = run_command_with_socket(&cli, &socket_path).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "core_unavailable");
}
