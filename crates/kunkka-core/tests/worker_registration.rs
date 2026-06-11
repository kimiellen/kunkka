use kunkka_core::ipc_server::CoreIpcServer;
use kunkka_core::worker_registry::{handle_worker_registration_frame, WorkerRegistry};
use kunkka_core::xdg::KunkkaPaths;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, Payload, RequestId, SessionId};
use kunkka_worker_sdk::{
    decode_worker_message, encode_worker_message, AppId, RegisterWorkerRequest, WorkerCapability,
    WorkerClient, WorkerId, WorkerProtocolMessage,
};
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

fn request() -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new("worker-1"),
        app_id: AppId::new("example-app"),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: Some("Search notes".to_string()),
        }],
    }
}

fn registration_frame() -> Frame {
    let payload = encode_worker_message(&WorkerProtocolMessage::RegisterWorker(request())).unwrap();

    Frame::Request {
        request_id: RequestId(7),
        session_id: SessionId(9),
        source: EndpointId::new("worker:worker-1"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    }
}

#[test]
fn handles_worker_registration_request_frame() {
    let mut registry = WorkerRegistry::new();

    let response_frame =
        handle_worker_registration_frame(&mut registry, registration_frame()).unwrap();

    let Frame::Response {
        request_id,
        session_id,
        source,
        target,
        payload,
        ..
    } = response_frame
    else {
        panic!("expected response frame");
    };

    assert_eq!(request_id, RequestId(7));
    assert_eq!(session_id, SessionId(9));
    assert_eq!(source.as_str(), "core");
    assert_eq!(target.as_str(), "worker:worker-1");

    let decoded = decode_worker_message(&payload).unwrap();

    match decoded {
        WorkerProtocolMessage::RegisterWorkerAccepted(response) => {
            assert!(response.accepted);
            assert_eq!(response.worker_id.as_str(), "worker-1");
        }
        other => panic!("expected RegisterWorkerAccepted, got {other:?}"),
    }

    let registered = registry.get(&WorkerId::new("worker-1")).unwrap();
    assert_eq!(registered.app_id.as_str(), "example-app");
}

#[test]
fn rejects_non_request_registration_frame() {
    let mut registry = WorkerRegistry::new();

    let err = handle_worker_registration_frame(
        &mut registry,
        Frame::Event {
            session_id: SessionId(1),
            source: EndpointId::new("worker:worker-1"),
            target: EndpointId::new("core"),
            name: "not-registration".to_string(),
            payload: Payload::from_bytes(Vec::new()),
            metadata: FrameMetadata::new(),
        },
    )
    .unwrap_err();

    assert!(err.to_string().contains("expected request frame"));
}

#[tokio::test]
async fn worker_client_registers_with_core_over_ipc() {
    let (_root, paths) = test_paths();
    paths.ensure_dirs().unwrap();

    let server = CoreIpcServer::bind(&paths).await.unwrap();

    let server_task = tokio::spawn(async move {
        let mut registry = WorkerRegistry::new();
        let mut connection = server.accept_one().await.unwrap();
        let frame = connection.recv_frame().await.unwrap().unwrap();

        let response = handle_worker_registration_frame(&mut registry, frame).unwrap();

        connection.send_frame(&response).await.unwrap();

        registry
    });

    let mut client = WorkerClient::connect(&paths.socket_path, WorkerId::new("worker-1"))
        .await
        .unwrap();

    let response = client.register(request()).await.unwrap();

    assert!(response.accepted);
    assert_eq!(response.worker_id.as_str(), "worker-1");

    let registry = server_task.await.unwrap();
    assert!(registry.get(&WorkerId::new("worker-1")).is_some());
}
