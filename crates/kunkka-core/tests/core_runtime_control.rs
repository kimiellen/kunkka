use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_core::CoreError;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingRequest,
    CorePingResponse, CoreStatusRequest,
};
use kunkka_worker_sdk::{AppId, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId};
use std::path::PathBuf;
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

async fn send_frame(socket_path: PathBuf, frame: Frame) {
    let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
    connection.send_frame(&frame).await.unwrap();
}

#[tokio::test]
async fn ping_returns_pong() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            let payload =
                encode_control_message(&CoreControlMessage::Ping(CorePingRequest)).unwrap();
            let frame = Frame::Request {
                request_id: RequestId(11),
                session_id: SessionId(22),
                source: EndpointId::new("cli"),
                target: EndpointId::new("core"),
                payload,
                metadata: FrameMetadata::new(),
            };

            connection.send_frame(&frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    runtime.run_once().await.unwrap();

    let response_frame = client_task.await.unwrap();
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

    assert_eq!(request_id, RequestId(11));
    assert_eq!(session_id, SessionId(22));
    assert_eq!(source.as_str(), "core");
    assert_eq!(target.as_str(), "cli");

    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(decoded, CoreControlMessage::Pong(CorePingResponse));
}

#[tokio::test]
async fn status_returns_runtime_state() {
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
    let registration_response = register_task.await.unwrap();
    assert!(registration_response.accepted);

    let expected_socket_path = paths.socket_path.to_string_lossy().into_owned();
    let status_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            let payload =
                encode_control_message(&CoreControlMessage::Status(CoreStatusRequest)).unwrap();
            let frame = Frame::Request {
                request_id: RequestId(33),
                session_id: SessionId(44),
                source: EndpointId::new("cli"),
                target: EndpointId::new("core"),
                payload,
                metadata: FrameMetadata::new(),
            };

            connection.send_frame(&frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    runtime.run_once().await.unwrap();

    let response_frame = status_task.await.unwrap();
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

    assert_eq!(request_id, RequestId(33));
    assert_eq!(session_id, SessionId(44));
    assert_eq!(source.as_str(), "core");
    assert_eq!(target.as_str(), "cli");

    let decoded = decode_control_message(&payload).unwrap();
    let CoreControlMessage::StatusResult(status) = decoded else {
        panic!("expected status result");
    };

    assert_eq!(status.worker_count, 1);
    assert_eq!(status.socket_path, expected_socket_path);
    assert!(status.runtime_ready);
}

#[tokio::test]
async fn unknown_schema_returns_invalid_core_frame() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let payload = Payload {
        bytes: Vec::new(),
        content_type: None,
        schema: Some("kunkka.unknown.v1".to_string()),
        metadata: FrameMetadata::new(),
    };
    let frame = Frame::Request {
        request_id: RequestId(55),
        session_id: SessionId(66),
        source: EndpointId::new("cli"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    let client_task = tokio::spawn(send_frame(paths.socket_path.clone(), frame));
    let err = runtime.run_once().await.unwrap_err();
    client_task.await.unwrap();

    assert!(matches!(
        err,
        CoreError::InvalidCoreFrame(message) if message.contains("unknown payload schema")
    ));
}

#[tokio::test]
async fn core_control_event_returns_invalid_core_frame() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let payload = encode_control_message(&CoreControlMessage::Ping(CorePingRequest)).unwrap();
    let frame = Frame::Event {
        session_id: SessionId(77),
        source: EndpointId::new("cli"),
        target: EndpointId::new("core"),
        name: "core-control".to_string(),
        payload,
        metadata: FrameMetadata::new(),
    };

    let client_task = tokio::spawn(send_frame(paths.socket_path.clone(), frame));
    let err = runtime.run_once().await.unwrap_err();
    client_task.await.unwrap();

    assert!(matches!(
        err,
        CoreError::InvalidCoreFrame(message) if message.contains("expected request frame")
    ));
}

#[tokio::test]
async fn core_control_response_message_as_request_returns_invalid_core_frame() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let payload = encode_control_message(&CoreControlMessage::Pong(CorePingResponse)).unwrap();
    let frame = Frame::Request {
        request_id: RequestId(88),
        session_id: SessionId(99),
        source: EndpointId::new("cli"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    let client_task = tokio::spawn(send_frame(paths.socket_path.clone(), frame));
    let err = runtime.run_once().await.unwrap_err();
    client_task.await.unwrap();

    assert!(matches!(
        err,
        CoreError::InvalidCoreFrame(message) if message.contains("expected core control request")
    ));
}

#[tokio::test]
async fn one_connection_can_handle_multiple_control_requests() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();

            let ping_payload =
                encode_control_message(&CoreControlMessage::Ping(CorePingRequest)).unwrap();
            let ping_frame = Frame::Request {
                request_id: RequestId(101),
                session_id: SessionId(202),
                source: EndpointId::new("native-host"),
                target: EndpointId::new("core"),
                payload: ping_payload,
                metadata: FrameMetadata::new(),
            };
            connection.send_frame(&ping_frame).await.unwrap();
            let ping_response = connection.recv_frame().await.unwrap().unwrap();

            let status_payload =
                encode_control_message(&CoreControlMessage::Status(CoreStatusRequest)).unwrap();
            let status_frame = Frame::Request {
                request_id: RequestId(102),
                session_id: SessionId(202),
                source: EndpointId::new("native-host"),
                target: EndpointId::new("core"),
                payload: status_payload,
                metadata: FrameMetadata::new(),
            };
            connection.send_frame(&status_frame).await.unwrap();
            let status_response = connection.recv_frame().await.unwrap().unwrap();

            (ping_response, status_response)
        }
    });

    runtime.run_once().await.unwrap();

    let (ping_response, status_response) = client_task.await.unwrap();

    let Frame::Response {
        request_id: ping_request_id,
        payload: ping_payload,
        ..
    } = ping_response
    else {
        panic!("expected ping response frame");
    };
    assert_eq!(ping_request_id, RequestId(101));
    assert_eq!(
        decode_control_message(&ping_payload).unwrap(),
        CoreControlMessage::Pong(CorePingResponse)
    );

    let Frame::Response {
        request_id: status_request_id,
        payload: status_payload,
        ..
    } = status_response
    else {
        panic!("expected status response frame");
    };
    assert_eq!(status_request_id, RequestId(102));
    assert!(matches!(
        decode_control_message(&status_payload).unwrap(),
        CoreControlMessage::StatusResult(_)
    ));
}
