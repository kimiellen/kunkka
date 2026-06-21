use kunkka_core::worker_dispatch::{DispatchResult, WorkerManager};
use kunkka_core::Result as CoreResult;
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
    registration_for("notes", "notes")
}

fn registration_for(worker_id: &str, app_id: &str) -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new(worker_id),
        app_id: AppId::new(app_id),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: None,
        }],
    }
}

async fn dispatch_with_timeout(
    manager: &mut WorkerManager,
    app_id: AppId,
    method: &str,
    payload: Payload,
) -> CoreResult<DispatchResult> {
    timeout(
        Duration::from_secs(2),
        manager.dispatch(app_id, method.to_string(), payload),
    )
    .await
    .unwrap()
}

async fn idle_core_connection() -> (TempDir, tokio::task::JoinHandle<()>, IpcConnection) {
    let (root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let _connection = IpcConnection::connect(&socket_path).await.unwrap();
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
    let core_connection = listener.accept().await.unwrap();

    (root, worker_task, core_connection)
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

    let result = dispatch_with_timeout(
        &mut manager,
        AppId::new("notes"),
        "search",
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

    let result =
        dispatch_with_timeout(&mut manager, AppId::new("notes"), "missing", payload(b"{}"))
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

    let err = dispatch_with_timeout(&mut manager, AppId::new("notes"), "search", payload(b"{}"))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("dispatch ipc error"));
    assert!(!manager.is_active(&AppId::new("notes")));
    assert!(manager
        .registry()
        .get_by_app_id(&AppId::new("notes"))
        .is_none());
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

    let err = dispatch_with_timeout(&mut manager, AppId::new("notes"), "search", payload(b"{}"))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("response request_id mismatch"));
    assert!(!manager.is_active(&AppId::new("notes")));
    assert!(manager
        .registry()
        .get_by_app_id(&AppId::new("notes"))
        .is_none());
    timeout(Duration::from_secs(2), worker_task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn duplicate_worker_id_moving_app_removes_old_active_worker() {
    let (_first_root, first_worker_task, first_core_connection) = idle_core_connection().await;
    let (_second_root, second_worker_task, second_core_connection) = idle_core_connection().await;
    let mut manager = WorkerManager::new_empty();

    manager.register_active_for_test(
        registration_for("notes", "notes"),
        first_core_connection,
        300_000,
    );
    manager.register_active_for_test(
        registration_for("notes", "tasks"),
        second_core_connection,
        300_000,
    );

    assert!(!manager.is_active(&AppId::new("notes")));
    assert!(manager.is_active(&AppId::new("tasks")));
    assert_eq!(manager.active_worker_count(), 1);
    assert!(manager
        .registry()
        .get_by_app_id(&AppId::new("notes"))
        .is_none());
    assert_eq!(
        manager
            .registry()
            .get_by_app_id(&AppId::new("tasks"))
            .unwrap()
            .worker_id,
        WorkerId::new("notes")
    );

    first_worker_task.abort();
    second_worker_task.abort();
}

#[tokio::test]
async fn dispatch_decode_failure_removes_active_worker_and_returns_unexpected_response() {
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
                request_id,
                session_id,
                source,
                target,
                ..
            } = frame
            else {
                panic!("expected dispatch request frame");
            };
            let response = Frame::Response {
                request_id,
                session_id,
                source: target,
                target: source,
                payload: payload(b"not a worker protocol message"),
                metadata: FrameMetadata::new(),
            };
            connection.send_frame(&response).await.unwrap();
        }
    });

    let core_connection = listener.accept().await.unwrap();
    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 300_000);

    let err = dispatch_with_timeout(&mut manager, AppId::new("notes"), "search", payload(b"{}"))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("unexpected worker response"));
    assert!(!manager.is_active(&AppId::new("notes")));
    assert!(manager
        .registry()
        .get_by_app_id(&AppId::new("notes"))
        .is_none());
    timeout(Duration::from_secs(2), worker_task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn dispatch_to_different_app_ids_routes_to_correct_workers() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();

    let notes_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let connection = IpcConnection::connect(&socket_path).await.unwrap();
            let mut worker = WorkerClient::from_connection(
                connection,
                WorkerId::new("notes-worker"),
                kunkka_ipc::SessionId(1),
            );
            let ctx = timeout(Duration::from_secs(2), worker.recv_dispatch())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(ctx.request.app_id.as_str(), "notes");
            worker
                .respond_dispatch(ctx, DispatchWorkerResponse::Ok(payload(b"notes-result")))
                .await
                .unwrap();
        }
    });

    let notes_conn = listener.accept().await.unwrap();
    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(
        registration_for("notes-worker", "notes"),
        notes_conn,
        300_000,
    );

    let todo_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let connection = IpcConnection::connect(&socket_path).await.unwrap();
            let mut worker = WorkerClient::from_connection(
                connection,
                WorkerId::new("todo-worker"),
                kunkka_ipc::SessionId(1),
            );
            let ctx = timeout(Duration::from_secs(2), worker.recv_dispatch())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(ctx.request.app_id.as_str(), "todo");
            worker
                .respond_dispatch(ctx, DispatchWorkerResponse::Ok(payload(b"todo-result")))
                .await
                .unwrap();
        }
    });

    let todo_conn = listener.accept().await.unwrap();
    manager.register_active_for_test(registration_for("todo-worker", "todo"), todo_conn, 300_000);

    assert!(manager.is_active(&AppId::new("notes")));
    assert!(manager.is_active(&AppId::new("todo")));

    let notes_result =
        dispatch_with_timeout(&mut manager, AppId::new("notes"), "search", payload(b"q")).await;
    let DispatchResult::Ok(p) = notes_result.unwrap() else {
        panic!("expected notes ok");
    };
    assert_eq!(p.bytes, b"notes-result");

    let todo_result =
        dispatch_with_timeout(&mut manager, AppId::new("todo"), "list", payload(b"")).await;
    let DispatchResult::Ok(p) = todo_result.unwrap() else {
        panic!("expected todo ok");
    };
    assert_eq!(p.bytes, b"todo-result");

    notes_task.await.unwrap();
    todo_task.await.unwrap();
}
