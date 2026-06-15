use kunkka_core::prepare_core_runtime;
use kunkka_core::worker_dispatch::{DispatchResult, WorkerManager};
use kunkka_core::xdg::KunkkaPaths;
use kunkka_ipc::{
    EndpointId, Frame, FrameMetadata, IpcConnection, IpcListener, Payload, RequestId, SessionId,
};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingRequest,
    CorePingResponse, CoreStatusRequest,
};
use kunkka_worker_sdk::{
    AppId, DispatchWorkerResponse, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId,
};
use std::fs;
use std::process::Command;
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

fn socket_path() -> (TempDir, std::path::PathBuf) {
    let root = tempdir().unwrap();
    let socket_path = root.path().join("worker.sock");
    (root, socket_path)
}

fn registration() -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new("notes"),
        app_id: AppId::new("notes"),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: None,
        }],
    }
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

fn worker_idle_fixture_manifest() -> String {
    let current_exe = std::env::current_exe().unwrap();
    format!(
        r#"{{
            "app_id": "notes",
            "worker": {{
                "program": {},
                "args": ["worker_idle_fixture_entrypoint", "--exact", "--nocapture"],
                "env": {{ "KUNKKA_WORKER_IDLE_FIXTURE": "ok" }}
            }},
            "idle_timeout_ms": 1,
            "startup_timeout_ms": 5000
        }}"#,
        serde_json::to_string(current_exe.to_str().unwrap()).unwrap(),
    )
}

fn process_exists(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

async fn wait_until_process_exits(pid: u32) -> bool {
    for _ in 0..10 {
        if !process_exists(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    false
}

async fn core_status_worker_count(socket_path: &std::path::Path) -> u64 {
    let mut connection = IpcConnection::connect(socket_path).await.unwrap();
    let payload = encode_control_message(&CoreControlMessage::Status(CoreStatusRequest)).unwrap();
    let frame = Frame::Request {
        request_id: RequestId(99),
        session_id: SessionId(100),
        source: EndpointId::new("test"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    connection.send_frame(&frame).await.unwrap();
    let response = connection.recv_frame().await.unwrap().unwrap();
    let Frame::Response { payload, .. } = response else {
        panic!("expected response frame");
    };
    let CoreControlMessage::StatusResult(status) = decode_control_message(&payload).unwrap() else {
        panic!("expected status result");
    };
    status.worker_count
}

async fn send_control_request(
    connection: &mut IpcConnection,
    request_id: u128,
    message: CoreControlMessage,
) -> CoreControlMessage {
    let payload = encode_control_message(&message).unwrap();
    let frame = Frame::Request {
        request_id: RequestId(request_id),
        session_id: SessionId(100),
        source: EndpointId::new("test"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    connection.send_frame(&frame).await.unwrap();
    let response = connection.recv_frame().await.unwrap().unwrap();
    let Frame::Response { payload, .. } = response else {
        panic!("expected response frame");
    };

    decode_control_message(&payload).unwrap()
}

#[test]
fn worker_idle_fixture_entrypoint() {
    if std::env::var("KUNKKA_WORKER_IDLE_FIXTURE").is_err() {
        return;
    }

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let socket_path = std::env::var("KUNKKA_CORE_SOCKET").unwrap();
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
        client
            .respond_dispatch(
                request,
                DispatchWorkerResponse::Ok(payload(br#"{"items":[]}"#)),
            )
            .await
            .unwrap();
    });
}

#[tokio::test]
async fn reap_idle_workers_removes_expired_active_worker() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let _connection = IpcConnection::connect(&socket_path).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });
    let core_connection = listener.accept().await.unwrap();

    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 1);
    assert!(manager.is_active(&AppId::new("notes")));

    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    manager.reap_idle_workers();

    assert!(!manager.is_active(&AppId::new("notes")));
    assert_eq!(manager.active_worker_count(), 0);
    assert!(manager
        .registry()
        .get_by_app_id(&AppId::new("notes"))
        .is_none());
    worker_task.await.unwrap();
}

#[tokio::test]
async fn reap_idle_workers_keeps_recent_active_worker() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let _connection = IpcConnection::connect(&socket_path).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    });
    let core_connection = listener.accept().await.unwrap();

    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 60_000);
    manager.reap_idle_workers();

    assert!(manager.is_active(&AppId::new("notes")));
    worker_task.await.unwrap();
}

#[tokio::test]
async fn dropping_worker_manager_terminates_active_child() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let _connection = IpcConnection::connect(&socket_path).await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    let core_connection = listener.accept().await.unwrap();

    let mut manager = WorkerManager::new_empty();
    let child = Command::new("sleep").arg("30").spawn().unwrap();
    let pid = child.id();
    manager.insert_active_worker(registration(), core_connection, Some(child), 60_000);

    drop(manager);

    let exited = wait_until_process_exits(pid).await;
    if !exited {
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }
    assert!(exited, "active child process should be terminated on drop");
    worker_task.await.unwrap();
}

#[tokio::test]
async fn runtime_run_reaps_idle_workers_while_waiting_for_connections() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, &worker_idle_fixture_manifest());
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        runtime.dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}")),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result, DispatchResult::Ok(payload(br#"{"items":[]}"#)));
    assert!(runtime.worker_manager().is_active(&AppId::new("notes")));
    assert_eq!(runtime.worker_manager().active_worker_count(), 1);

    let socket_path = paths.socket_path.clone();
    let run_task = tokio::spawn(runtime.run());
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(core_status_worker_count(&socket_path).await, 0);
    run_task.abort();
}

#[tokio::test]
async fn runtime_run_reaps_idle_workers_during_open_control_connection() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, &worker_idle_fixture_manifest());
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        runtime.dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}")),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result, DispatchResult::Ok(payload(br#"{"items":[]}"#)));
    assert!(runtime.worker_manager().is_active(&AppId::new("notes")));
    assert_eq!(runtime.worker_manager().active_worker_count(), 1);

    let socket_path = paths.socket_path.clone();
    let run_task = tokio::spawn(runtime.run());
    let mut connection = IpcConnection::connect(&socket_path).await.unwrap();

    let pong = send_control_request(
        &mut connection,
        101,
        CoreControlMessage::Ping(CorePingRequest),
    )
    .await;
    assert_eq!(pong, CoreControlMessage::Pong(CorePingResponse));

    tokio::time::sleep(Duration::from_millis(200)).await;

    let status = send_control_request(
        &mut connection,
        102,
        CoreControlMessage::Status(CoreStatusRequest),
    )
    .await;
    let CoreControlMessage::StatusResult(status) = status else {
        panic!("expected status result");
    };
    assert_eq!(status.worker_count, 0);

    run_task.abort();
}
