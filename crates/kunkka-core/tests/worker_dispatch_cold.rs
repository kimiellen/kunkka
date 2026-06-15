use kunkka_core::worker_dispatch::DispatchResult;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_core::{prepare_core_runtime, CoreError};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_worker_sdk::{
    encode_worker_message, AppId, DispatchWorkerResponse, RegisterWorkerRequest, WorkerCapability,
    WorkerClient, WorkerId, WorkerProtocolMessage,
};
use std::fs;
use std::time::Duration;
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

fn payload(bytes: &[u8]) -> Payload {
    Payload {
        bytes: bytes.to_vec(),
        content_type: Some("application/json".to_string()),
        schema: Some("example.notes.v1".to_string()),
        metadata: FrameMetadata::new(),
    }
}

fn write_manifest(paths: &KunkkaPaths, body: &str) {
    let apps_dir = paths.config_dir.join("apps");
    fs::create_dir_all(&apps_dir).unwrap();
    fs::write(apps_dir.join("notes.json"), body).unwrap();
}

fn worker_fixture_manifest(mode: &str, startup_timeout_ms: u64) -> String {
    let current_exe = std::env::current_exe().unwrap();
    format!(
        r#"{{
            "app_id": "notes",
            "worker": {{
                "program": {},
                "args": ["worker_fixture_entrypoint", "--exact", "--nocapture"],
                "env": {{ "KUNKKA_WORKER_FIXTURE": {} }}
            }},
            "idle_timeout_ms": 300000,
            "startup_timeout_ms": {}
        }}"#,
        serde_json::to_string(current_exe.to_str().unwrap()).unwrap(),
        serde_json::to_string(mode).unwrap(),
        startup_timeout_ms
    )
}

#[test]
fn worker_fixture_entrypoint() {
    let Ok(mode) = std::env::var("KUNKKA_WORKER_FIXTURE") else {
        return;
    };

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async move {
        if mode == "never-register" {
            tokio::time::sleep(Duration::from_millis(500)).await;
            return;
        }

        let socket_path = std::env::var("KUNKKA_CORE_SOCKET").unwrap();
        if mode == "wrong-registration" {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("wrong"))
                .await
                .unwrap();
            let _ = client
                .register(RegisterWorkerRequest {
                    worker_id: WorkerId::new("wrong"),
                    app_id: AppId::new("wrong"),
                    capabilities: vec![WorkerCapability {
                        name: "wrong.search".to_string(),
                        description: None,
                    }],
                })
                .await;
            return;
        }

        let app_id = std::env::var("KUNKKA_APP_ID").unwrap();
        let worker_id = std::env::var("KUNKKA_WORKER_ID").unwrap();
        let mut client = WorkerClient::connect(&socket_path, WorkerId::new(worker_id.clone()))
            .await
            .unwrap();
        client
            .register(RegisterWorkerRequest {
                worker_id: WorkerId::new(worker_id),
                app_id: AppId::new(app_id),
                capabilities: vec![WorkerCapability {
                    name: "notes.search".to_string(),
                    description: None,
                }],
            })
            .await
            .unwrap();

        let request = client.recv_dispatch().await.unwrap();
        if mode == "app-error" {
            client
                .respond_dispatch(
                    request,
                    DispatchWorkerResponse::Err(kunkka_worker_sdk::WorkerAppError {
                        code: "not_found".to_string(),
                        message: "missing note".to_string(),
                    }),
                )
                .await
                .unwrap();
        } else {
            client
                .respond_dispatch(
                    request,
                    DispatchWorkerResponse::Ok(payload(br#"{"items":[]}"#)),
                )
                .await
                .unwrap();
        }
    });
}

#[tokio::test]
async fn cold_dispatch_starts_worker_and_returns_payload() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, &worker_fixture_manifest("ok", 5000));
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        runtime.dispatch(
            AppId::new("notes"),
            "search".to_string(),
            payload(br#"{"query":"kunkka"}"#),
        ),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result, DispatchResult::Ok(payload(br#"{"items":[]}"#)));
    assert!(runtime.worker_manager().is_active(&AppId::new("notes")));
}

#[tokio::test]
async fn cold_dispatch_returns_worker_app_error() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, &worker_fixture_manifest("app-error", 5000));
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        runtime.dispatch(AppId::new("notes"), "missing".to_string(), payload(b"{}")),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        result,
        DispatchResult::AppError {
            code: "not_found".to_string(),
            message: "missing note".to_string(),
        }
    );
}

#[tokio::test]
async fn manifest_env_cannot_override_core_worker_env() {
    let (_root, paths) = test_paths();
    let current_exe = std::env::current_exe().unwrap();
    write_manifest(
        &paths,
        &format!(
            r#"{{
                "app_id": "notes",
                "worker": {{
                    "program": {},
                    "args": ["worker_fixture_entrypoint", "--exact", "--nocapture"],
                    "env": {{
                        "KUNKKA_WORKER_FIXTURE": "ok",
                        "KUNKKA_CORE_SOCKET": "/path/that/does/not/exist/core.sock",
                        "KUNKKA_APP_ID": "wrong-app",
                        "KUNKKA_WORKER_ID": "wrong-worker"
                    }}
                }},
                "idle_timeout_ms": 300000,
                "startup_timeout_ms": 5000
            }}"#,
            serde_json::to_string(current_exe.to_str().unwrap()).unwrap(),
        ),
    );
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        runtime.dispatch(
            AppId::new("notes"),
            "search".to_string(),
            payload(br#"{"query":"kunkka"}"#),
        ),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result, DispatchResult::Ok(payload(br#"{"items":[]}"#)));
    assert!(runtime.worker_manager().is_active(&AppId::new("notes")));
    assert_eq!(
        runtime
            .registry()
            .get_by_app_id(&AppId::new("notes"))
            .unwrap()
            .worker_id
            .as_str(),
        "notes"
    );
}

#[tokio::test]
async fn cold_dispatch_rejects_mismatched_worker_registration() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, &worker_fixture_manifest("wrong-registration", 50));
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let err = tokio::time::timeout(
        Duration::from_secs(10),
        runtime.dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}")),
    )
    .await
    .unwrap()
    .unwrap_err();

    assert!(matches!(
        err,
        CoreError::WorkerStartTimeout(message) if message.contains("notes")
    ));
    assert!(!runtime.worker_manager().is_active(&AppId::new("notes")));
    assert!(!runtime.worker_manager().is_active(&AppId::new("wrong")));
    assert!(runtime
        .registry()
        .get_by_app_id(&AppId::new("wrong"))
        .is_none());
}

#[tokio::test]
async fn cold_dispatch_ignores_unrelated_registration_while_waiting_for_worker() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, &worker_fixture_manifest("ok", 5000));
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let stray_socket_path = paths.socket_path.clone();
    let stray_task = tokio::spawn(async move {
        let mut connection = IpcConnection::connect(&stray_socket_path).await.unwrap();
        let payload = encode_worker_message(&WorkerProtocolMessage::RegisterWorker(
            RegisterWorkerRequest {
                worker_id: WorkerId::new("stray"),
                app_id: AppId::new("stray"),
                capabilities: vec![WorkerCapability {
                    name: "stray.search".to_string(),
                    description: None,
                }],
            },
        ))
        .unwrap();
        let frame = Frame::Request {
            request_id: RequestId(1),
            session_id: SessionId(1),
            source: EndpointId::new("worker:stray"),
            target: EndpointId::new("core"),
            payload,
            metadata: FrameMetadata::new(),
        };
        connection.send_frame(&frame).await.unwrap();
    });
    stray_task.await.unwrap();

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        runtime.dispatch(
            AppId::new("notes"),
            "search".to_string(),
            payload(br#"{"query":"kunkka"}"#),
        ),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result, DispatchResult::Ok(payload(br#"{"items":[]}"#)));
    assert!(runtime.worker_manager().is_active(&AppId::new("notes")));
    assert!(!runtime.worker_manager().is_active(&AppId::new("stray")));
    assert!(runtime
        .registry()
        .get_by_app_id(&AppId::new("stray"))
        .is_none());
}

#[tokio::test]
async fn cold_dispatch_ignores_silent_connection_while_waiting_for_worker() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, &worker_fixture_manifest("ok", 5000));
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let stray = IpcConnection::connect(&paths.socket_path).await.unwrap();

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        runtime.dispatch(
            AppId::new("notes"),
            "search".to_string(),
            payload(br#"{"query":"kunkka"}"#),
        ),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result, DispatchResult::Ok(payload(br#"{"items":[]}"#)));
    assert!(runtime.worker_manager().is_active(&AppId::new("notes")));
    drop(stray);
}

#[tokio::test]
async fn missing_manifest_returns_app_not_found() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let err = runtime
        .dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}"))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CoreError::AppNotFound(message) if message.contains("notes")
    ));
}

#[tokio::test]
async fn invalid_worker_executable_returns_start_failed() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/path/that/does/not/exist/kunkka-worker",
                "args": []
            },
            "idle_timeout_ms": 300000,
            "startup_timeout_ms": 5000
        }"#,
    );
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let err = runtime
        .dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}"))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CoreError::WorkerStartFailed(message) if message.contains("notes")
    ));
    assert!(!runtime.worker_manager().is_active(&AppId::new("notes")));
}

#[tokio::test]
async fn worker_that_never_registers_returns_start_timeout() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, &worker_fixture_manifest("never-register", 50));
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let err = tokio::time::timeout(
        Duration::from_secs(2),
        runtime.dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}")),
    )
    .await
    .unwrap()
    .unwrap_err();

    assert!(matches!(
        err,
        CoreError::WorkerStartTimeout(message) if message.contains("notes")
    ));
    assert!(!runtime.worker_manager().is_active(&AppId::new("notes")));
}
