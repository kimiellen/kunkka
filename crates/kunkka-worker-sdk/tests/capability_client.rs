use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcListener, RequestId, SessionId};
use kunkka_worker_sdk::capability::{
    call_capability, decode_capability_request, encode_capability_response, CapabilityError,
    CapabilityResponse,
};
use kunkka_worker_sdk::AppId;
use tempfile::{tempdir, TempDir};

fn socket_path() -> (TempDir, std::path::PathBuf) {
    let root = tempdir().unwrap();
    let path = root.path().join("capability.sock");
    (root, path)
}

#[tokio::test]
async fn call_capability_sends_request_and_decodes_success_response() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let server_task = tokio::spawn(async move {
        let mut connection = listener.accept().await.unwrap();
        let frame = connection.recv_frame().await.unwrap().unwrap();
        let Frame::Request {
            request_id,
            payload,
            ..
        } = frame
        else {
            panic!("expected request frame");
        };

        let request = decode_capability_request(&payload).unwrap();
        assert_eq!(request.app_id, "notes");
        assert_eq!(request.capability, "fs");
        assert_eq!(request.method, "read_file");
        assert_eq!(request.params, b"encoded-params");

        let response = Frame::Response {
            request_id,
            session_id: SessionId(1),
            source: EndpointId::new("core"),
            target: EndpointId::new("worker-sdk"),
            payload: encode_capability_response(&CapabilityResponse {
                result: Ok(b"encoded-result".to_vec()),
            })
            .unwrap(),
            metadata: FrameMetadata::new(),
        };
        connection.send_frame(&response).await.unwrap();
    });

    let response = call_capability(
        &socket_path,
        &AppId::new("notes"),
        "fs",
        "read_file",
        b"encoded-params".to_vec(),
    )
    .await
    .unwrap();

    assert_eq!(response.result.unwrap(), b"encoded-result");
    server_task.await.unwrap();
}

#[tokio::test]
async fn call_capability_decodes_app_error_response() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let server_task = tokio::spawn(async move {
        let mut connection = listener.accept().await.unwrap();
        let frame = connection.recv_frame().await.unwrap().unwrap();
        let Frame::Request { request_id, .. } = frame else {
            panic!("expected request frame");
        };

        let response = Frame::Response {
            request_id,
            session_id: SessionId(1),
            source: EndpointId::new("core"),
            target: EndpointId::new("worker-sdk"),
            payload: encode_capability_response(&CapabilityResponse {
                result: Err(CapabilityError {
                    code: "permission_denied".to_string(),
                    message: "outside whitelist".to_string(),
                }),
            })
            .unwrap(),
            metadata: FrameMetadata::new(),
        };
        connection.send_frame(&response).await.unwrap();
    });

    let response = call_capability(
        &socket_path,
        &AppId::new("notes"),
        "fs",
        "read_file",
        Vec::new(),
    )
    .await
    .unwrap();

    assert_eq!(
        response.result.unwrap_err(),
        CapabilityError {
            code: "permission_denied".to_string(),
            message: "outside whitelist".to_string(),
        }
    );
    server_task.await.unwrap();
}

#[tokio::test]
async fn call_capability_rejects_non_response_frame() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let server_task = tokio::spawn(async move {
        let mut connection = listener.accept().await.unwrap();
        let _ = connection.recv_frame().await.unwrap().unwrap();
        let frame = Frame::Request {
            request_id: RequestId(99),
            session_id: SessionId(1),
            source: EndpointId::new("core"),
            target: EndpointId::new("worker-sdk"),
            payload: encode_capability_response(&CapabilityResponse { result: Ok(vec![]) })
                .unwrap(),
            metadata: FrameMetadata::new(),
        };
        connection.send_frame(&frame).await.unwrap();
    });

    let err = call_capability(
        &socket_path,
        &AppId::new("notes"),
        "fs",
        "read_file",
        Vec::new(),
    )
    .await
    .unwrap_err();

    assert!(err
        .to_string()
        .contains("expected capability response frame"));
    server_task.await.unwrap();
}
