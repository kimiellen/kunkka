use kunkka_cli::cli::{Cli, CliCommand};
use kunkka_cli::run_command_with_socket;
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_ipc::{FrameMetadata, Payload};
use kunkka_worker_sdk::{
    AppId, DispatchWorkerResponse, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId,
};
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

fn write_manifest(config_dir: &std::path::Path, body: &str) {
    let apps_dir = config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();
    std::fs::write(apps_dir.join("notes.json"), body).unwrap();
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

#[tokio::test]
async fn cli_dispatch_returns_worker_payload() {
    let root = tempdir().unwrap();
    let socket_path = root.path().join("core.sock");
    let config_dir = root.path().join("config");

    write_manifest(
        &config_dir,
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": ["--serve"]
            },
            "permissions": {
                "frontend_dispatch": {
                    "allowed_methods": ["search"]
                }
            }
        }"#,
    );

    let paths = KunkkaPaths {
        config_dir,
        data_dir: root.path().join("data"),
        state_dir: root.path().join("state"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
        database_path: root.path().join("data/kunkka.db"),
        log_dir: root.path().join("state/logs"),
        socket_path: socket_path.clone(),
    };
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("notes"))
                .await
                .unwrap();
            let registration = client
                .register(RegisterWorkerRequest {
                    worker_id: WorkerId::new("notes"),
                    app_id: AppId::new("notes"),
                    capabilities: vec![WorkerCapability {
                        name: "notes.search".to_string(),
                        description: None,
                    }],
                })
                .await
                .unwrap();
            let request =
                tokio::time::timeout(std::time::Duration::from_secs(5), client.recv_dispatch())
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(request.request.app_id.as_str(), "notes");
            assert_eq!(request.request.method, "search");
            client
                .respond_dispatch(
                    request,
                    DispatchWorkerResponse::Ok(Payload {
                        bytes: br#"{"items":["a","b"]}"#.to_vec(),
                        content_type: Some("application/json".to_string()),
                        schema: None,
                        metadata: FrameMetadata::new(),
                    }),
                )
                .await
                .unwrap();
            registration
        }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let cli_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Dispatch {
                    app_id: "notes".to_string(),
                    method: "search".to_string(),
                    payload: serde_json::json!({"query": "kunkka"}),
                },
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
    assert_eq!(value["result"]["type"], "dispatch");
    assert_eq!(
        value["result"]["payload"]["items"],
        serde_json::json!(["a", "b"])
    );

    let registration = tokio::time::timeout(std::time::Duration::from_secs(5), worker_task)
        .await
        .unwrap()
        .unwrap();
    assert!(registration.accepted);
}
