use kunkka_core::worker_dispatch::{DispatchResult, WorkerManager};
use kunkka_ipc::{Frame, FrameMetadata, IpcConnection, IpcListener, Payload, RequestId};
use kunkka_worker_sdk::{
    encode_worker_message, AppId, DispatchWorkerResponse, RegisterWorkerRequest, WorkerAppError,
    WorkerCapability, WorkerClient, WorkerId, WorkerProtocolMessage,
};
use tempfile::{tempdir, TempDir};
use tokio::time::{timeout, Duration};

fn socket_path() -> (TempDir, std::path::PathBuf) {
    let root = tempdir().unwrap();
    let socket_path = root.path().join("worker.sock");
    (root, socket_path)
}

fn payload(bytes: &[u8]) -> Payload {
    Payload {
        bytes: bytes.to_vec(),
        content_type: Some("application/json".to_string()),
        schema: Some("example.notes.v1".to_string()),
        metadata: FrameMetadata::new(),
    }
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

#[tokio::test]
async fn dispatch_sends_request_to_active_worker_and_returns_payload() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let connection = IpcConnection::connect(&socket_path).await.unwrap();
            let mut worker = WorkerClient::from_connection(
                connection,
                WorkerId::new("notes"),
                kunkka_ipc::SessionId(1),
            );
            let request = timeout(Duration::from_secs(2), worker.recv_dispatch())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(request.request.app_id.as_str(), "notes");
            assert_eq!(request.request.method, "search");
            worker
                .respond_dispatch(
                    request,
                    DispatchWorkerResponse::Ok(payload(br#"{\"items\":[]}"#)),
                )
                .await
                .unwrap();
        }
    });

    let core_connection = listener.accept().await.unwrap();
    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 300_000);

    let result = manager
        .dispatch(
            AppId::new("notes"),
            "search".to_string(),
            payload(br#"{\"query\":\"kunkka\"}"#),
        )
        .await
        .unwrap();

    assert_eq!(result, DispatchResult::Ok(payload(br#"{\"items\":[]}"#)));
    timeout(Duration::from_secs(2), worker_task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn dispatch_returns_worker_app_error_without_removing_worker() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let connection = IpcConnection::connect(&socket_path).await.unwrap();
            let mut worker = WorkerClient::from_connection(
                connection,
                WorkerId::new("notes"),
                kunkka_ipc::SessionId(1),
            );
            let request = timeout(Duration::from_secs(2), worker.recv_dispatch())
                .await
                .unwrap()
                .unwrap();
            worker
                .respond_dispatch(
                    request,
                    DispatchWorkerResponse::Err(WorkerAppError {
                        code: "not_found".to_string(),
                        message: "missing note".to_string(),
                    }),
                )
                .await
                .unwrap();
        }
    });

    let core_connection = listener.accept().await.unwrap();
    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 300_000);

    let result = manager
        .dispatch(AppId::new("notes"), "missing".to_string(), payload(b"{}"))
        .await
        .unwrap();

    assert_eq!(
        result,
        DispatchResult::AppError {
            code: "not_found".to_string(),
            message: "missing note".to_string(),
        }
    );
    assert!(manager.is_active(&AppId::new("notes")));
    timeout(Duration::from_secs(2), worker_task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn dispatch_ipc_failure_removes_active_worker() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let _connection = IpcConnection::connect(&socket_path).await.unwrap();
        }
    });

    let core_connection = listener.accept().await.unwrap();
    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 300_000);
    timeout(Duration::from_secs(2), worker_task)
        .await
        .unwrap()
        .unwrap();

    let err = manager
        .dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}"))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("dispatch ipc error"));
    assert!(!manager.is_active(&AppId::new("notes")));
}

#[tokio::test]
async fn dispatch_request_id_mismatch_removes_active_worker() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            let frame = timeout(Duration::from_secs(2), connection.recv_frame())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let Frame::Request {
                session_id,
                source,
                target,
                ..
            } = frame
            else {
                panic!("expected dispatch request frame");
            };
            let response = Frame::Response {
                request_id: RequestId(999),
                session_id,
                source: target,
                target: source,
                payload: encode_worker_message(&WorkerProtocolMessage::DispatchWorkerResult(
                    DispatchWorkerResponse::Ok(payload(br#"{\"items\":[]}"#)),
                ))
                .unwrap(),
                metadata: FrameMetadata::new(),
            };
            connection.send_frame(&response).await.unwrap();
        }
    });

    let core_connection = listener.accept().await.unwrap();
    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 300_000);

    let err = manager
        .dispatch(AppId::new("notes"), "search".to_string(), payload(b"{}"))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("response request_id mismatch"));
    assert!(!manager.is_active(&AppId::new("notes")));
    timeout(Duration::from_secs(2), worker_task)
        .await
        .unwrap()
        .unwrap();
}
