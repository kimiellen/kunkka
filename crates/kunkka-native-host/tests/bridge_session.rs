use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_ipc::IpcListener;
use kunkka_native_host::bridge::NativeHostSession;
use kunkka_native_host::native_protocol::{NativeCommand, NativeRequest, NativeResult};
use kunkka_worker_sdk::{AppId, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId};
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

fn worker_request() -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new("worker-1"),
        app_id: AppId::new("example-app"),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: Some("Search notes".to_string()),
        }],
    }
}

#[tokio::test]
async fn session_ping_returns_pong() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let mut session = NativeHostSession::new(paths.socket_path.clone());
    let response = session
        .handle_request(NativeRequest {
            id: "req-1".to_string(),
            command: NativeCommand::Ping,
        })
        .await;

    assert!(response.ok);
    assert_eq!(response.id.as_deref(), Some("req-1"));
    assert_eq!(response.result, Some(NativeResult::Pong));

    drop(session);
    runtime_task.await.unwrap();
}

#[tokio::test]
async fn session_reuses_connection_for_ping_then_status() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let mut session = NativeHostSession::new(paths.socket_path.clone());

    let ping = session
        .handle_request(NativeRequest {
            id: "req-1".to_string(),
            command: NativeCommand::Ping,
        })
        .await;
    assert_eq!(ping.result, Some(NativeResult::Pong));

    let status = session
        .handle_request(NativeRequest {
            id: "req-2".to_string(),
            command: NativeCommand::Status,
        })
        .await;

    let Some(NativeResult::Status {
        worker_count,
        socket_path,
        runtime_ready,
    }) = status.result
    else {
        panic!("expected status result");
    };

    assert_eq!(worker_count, 0);
    assert_eq!(
        socket_path,
        paths.socket_path.to_string_lossy().into_owned()
    );
    assert!(runtime_ready);

    drop(session);
    runtime_task.await.unwrap();
}

#[tokio::test]
async fn session_status_reports_registered_worker() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let register_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("worker-1"))
                .await
                .unwrap();
            client.register(worker_request()).await.unwrap()
        }
    });

    runtime.run_once().await.unwrap();
    assert!(register_task.await.unwrap().accepted);

    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });
    let mut session = NativeHostSession::new(paths.socket_path.clone());
    let response = session
        .handle_request(NativeRequest {
            id: "req-status".to_string(),
            command: NativeCommand::Status,
        })
        .await;

    let Some(NativeResult::Status { worker_count, .. }) = response.result else {
        panic!("expected status result");
    };
    assert_eq!(worker_count, 1);

    drop(session);
    runtime_task.await.unwrap();
}

#[tokio::test]
async fn core_unavailable_returns_error_response() {
    let (_root, paths) = test_paths();
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    let response = session
        .handle_request(NativeRequest {
            id: "req-1".to_string(),
            command: NativeCommand::Ping,
        })
        .await;

    assert!(!response.ok);
    assert_eq!(response.id.as_deref(), Some("req-1"));
    assert_eq!(response.error.unwrap().code.to_string(), "core_unavailable");
}

#[tokio::test]
async fn ipc_failure_clears_connection_and_next_request_reconnects() {
    let (_root, paths) = test_paths();
    paths.ensure_dirs().unwrap();

    let listener = IpcListener::bind(&paths.socket_path).await.unwrap();
    let failing_server = tokio::spawn(async move {
        let _connection = listener.accept().await.unwrap();
    });

    let mut session = NativeHostSession::new(paths.socket_path.clone());
    let failed = session
        .handle_request(NativeRequest {
            id: "req-fail".to_string(),
            command: NativeCommand::Ping,
        })
        .await;

    assert!(!failed.ok);
    assert_eq!(failed.error.unwrap().code.to_string(), "core_ipc_error");
    failing_server.await.unwrap();

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let recovered = session
        .handle_request(NativeRequest {
            id: "req-ok".to_string(),
            command: NativeCommand::Ping,
        })
        .await;

    assert!(recovered.ok);
    assert_eq!(recovered.result, Some(NativeResult::Pong));

    drop(session);
    runtime_task.await.unwrap();
}
