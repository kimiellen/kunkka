use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_core::CoreError;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CoreStatusRequest,
};
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message, FrontendDispatchMessage,
    FrontendDispatchRequest, FrontendDispatchResponse,
};
use kunkka_worker_sdk::{
    AppId, DispatchWorkerResponse, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId,
};
use std::future::Future;
use tempfile::{tempdir, TempDir};
use tokio::time::{timeout, Duration};

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

async fn wait_for<T>(future: impl Future<Output = T>) -> T {
    timeout(TEST_TIMEOUT, future)
        .await
        .expect("test operation timed out")
}

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

fn json_payload(bytes: &[u8]) -> Payload {
    Payload {
        bytes: bytes.to_vec(),
        content_type: Some("application/json".to_string()),
        schema: None,
        metadata: FrameMetadata::new(),
    }
}

fn worker_request() -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new("notes"),
        app_id: AppId::new("notes"),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: None,
        }],
    }
}

async fn register_worker_and_wait_for_dispatch(
    socket_path: std::path::PathBuf,
    response: DispatchWorkerResponse,
) -> kunkka_worker_sdk::RegisterWorkerResponse {
    let mut client = WorkerClient::connect(&socket_path, WorkerId::new("notes"))
        .await
        .unwrap();
    let registration = client.register(worker_request()).await.unwrap();
    let request = wait_for(client.recv_dispatch()).await.unwrap();
    assert_eq!(request.request.app_id.as_str(), "notes");
    assert_eq!(request.request.method, "search");
    client.respond_dispatch(request, response).await.unwrap();
    registration
}

fn dispatch_frame(request_id: u128, app_id: &str, method: &str) -> Frame {
    let payload = encode_frontend_dispatch_message(&FrontendDispatchMessage::Dispatch(
        FrontendDispatchRequest {
            app_id: app_id.to_string(),
            method: method.to_string(),
            payload: json_payload(br#"{"query":"kunkka"}"#),
        },
    ))
    .unwrap();

    Frame::Request {
        request_id: RequestId(request_id),
        session_id: SessionId(1),
        source: EndpointId::new("native-host"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    }
}

#[tokio::test]
async fn frontend_dispatch_calls_warm_worker_and_returns_payload() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let worker_task = tokio::spawn(register_worker_and_wait_for_dispatch(
        paths.socket_path.clone(),
        DispatchWorkerResponse::Ok(json_payload(br#"{"items":[]}"#)),
    ));
    wait_for(runtime.run_once()).await.unwrap();

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection
                .send_frame(&dispatch_frame(10, "notes", "search"))
                .await
                .unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let response_frame = wait_for(frontend_task).await.unwrap();
    let Frame::Response {
        request_id,
        payload,
        ..
    } = response_frame
    else {
        panic!("expected response frame");
    };
    assert_eq!(request_id, RequestId(10));
    assert_eq!(
        decode_frontend_dispatch_message(&payload).unwrap(),
        FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::Ok(json_payload(
            br#"{"items":[]}"#
        )))
    );
    assert!(wait_for(worker_task).await.unwrap().accepted);
}

#[tokio::test]
async fn frontend_dispatch_returns_worker_app_error() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let worker_task = tokio::spawn(register_worker_and_wait_for_dispatch(
        paths.socket_path.clone(),
        DispatchWorkerResponse::Err(kunkka_worker_sdk::WorkerAppError {
            code: "not_found".to_string(),
            message: "note not found".to_string(),
        }),
    ));
    wait_for(runtime.run_once()).await.unwrap();

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection
                .send_frame(&dispatch_frame(11, "notes", "search"))
                .await
                .unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let Frame::Response { payload, .. } = wait_for(frontend_task).await.unwrap() else {
        panic!("expected response frame");
    };
    assert_eq!(
        decode_frontend_dispatch_message(&payload).unwrap(),
        FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::AppError {
            code: "not_found".to_string(),
            message: "note not found".to_string(),
        })
    );
    assert!(wait_for(worker_task).await.unwrap().accepted);
}

#[tokio::test]
async fn frontend_dispatch_maps_missing_manifest_to_platform_error() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection
                .send_frame(&dispatch_frame(12, "missing", "search"))
                .await
                .unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let Frame::Response { payload, .. } = wait_for(frontend_task).await.unwrap() else {
        panic!("expected response frame");
    };
    let FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::PlatformError {
        code,
        message,
    }) = decode_frontend_dispatch_message(&payload).unwrap()
    else {
        panic!("expected platform error");
    };
    assert_eq!(code, "app_not_found");
    assert!(message.contains("missing"));
}

#[tokio::test]
async fn frontend_dispatch_rejects_empty_method_as_platform_error() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection
                .send_frame(&dispatch_frame(15, "notes", ""))
                .await
                .unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let Frame::Response { payload, .. } = wait_for(frontend_task).await.unwrap() else {
        panic!("expected response frame");
    };
    let FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::PlatformError {
        code,
        message,
    }) = decode_frontend_dispatch_message(&payload).unwrap()
    else {
        panic!("expected platform error");
    };
    assert_eq!(code, "invalid_request");
    assert!(message.contains("method"));
}

#[tokio::test]
async fn frontend_dispatch_rejects_empty_app_id_as_platform_error() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection
                .send_frame(&dispatch_frame(13, "", "search"))
                .await
                .unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let Frame::Response { payload, .. } = wait_for(frontend_task).await.unwrap() else {
        panic!("expected response frame");
    };
    let FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::PlatformError {
        code,
        message,
    }) = decode_frontend_dispatch_message(&payload).unwrap()
    else {
        panic!("expected platform error");
    };
    assert_eq!(code, "invalid_request");
    assert!(message.contains("app_id"));
}

#[tokio::test]
async fn frontend_dispatch_event_returns_invalid_core_frame() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let Frame::Request { payload, .. } = dispatch_frame(14, "notes", "search") else {
        panic!("expected request frame");
    };
    let event = Frame::Event {
        session_id: SessionId(1),
        source: EndpointId::new("native-host"),
        target: EndpointId::new("core"),
        name: "frontend-dispatch".to_string(),
        payload,
        metadata: FrameMetadata::new(),
    };

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&event).await.unwrap();
        }
    });

    let err = wait_for(runtime.run_once()).await.unwrap_err();
    wait_for(frontend_task).await.unwrap();
    assert!(matches!(
        err,
        CoreError::InvalidCoreFrame(message) if message.contains("expected request frame")
    ));
}

#[tokio::test]
async fn one_frontend_connection_can_handle_status_then_dispatch() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let worker_task = tokio::spawn(register_worker_and_wait_for_dispatch(
        paths.socket_path.clone(),
        DispatchWorkerResponse::Ok(json_payload(br#"{"items":["a"]}"#)),
    ));
    wait_for(runtime.run_once()).await.unwrap();

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            let status_payload =
                encode_control_message(&CoreControlMessage::Status(CoreStatusRequest)).unwrap();
            let status_frame = Frame::Request {
                request_id: RequestId(20),
                session_id: SessionId(1),
                source: EndpointId::new("native-host"),
                target: EndpointId::new("core"),
                payload: status_payload,
                metadata: FrameMetadata::new(),
            };
            connection.send_frame(&status_frame).await.unwrap();
            let status_response = connection.recv_frame().await.unwrap().unwrap();

            connection
                .send_frame(&dispatch_frame(21, "notes", "search"))
                .await
                .unwrap();
            let dispatch_response = connection.recv_frame().await.unwrap().unwrap();

            (status_response, dispatch_response)
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let (status_response, dispatch_response) = wait_for(frontend_task).await.unwrap();
    let Frame::Response {
        payload: status_payload,
        ..
    } = status_response
    else {
        panic!("expected status response");
    };
    assert!(matches!(
        decode_control_message(&status_payload).unwrap(),
        CoreControlMessage::StatusResult(_)
    ));

    let Frame::Response {
        payload: dispatch_payload,
        ..
    } = dispatch_response
    else {
        panic!("expected dispatch response");
    };
    assert_eq!(
        decode_frontend_dispatch_message(&dispatch_payload).unwrap(),
        FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::Ok(json_payload(
            br#"{"items":["a"]}"#
        )))
    );
    assert!(wait_for(worker_task).await.unwrap().accepted);
}
