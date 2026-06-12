use kunkka_ipc::{
    EndpointId, Frame, FrameMetadata, IpcConnection, IpcListener, Payload, RequestId, SessionId,
};
use kunkka_worker_sdk::{
    decode_worker_message, encode_worker_message, AppId, DispatchWorkerRequest,
    DispatchWorkerResponse, WorkerAppError, WorkerClient, WorkerId, WorkerProtocolMessage,
};
use tempfile::{tempdir, TempDir};

fn socket_path() -> (TempDir, std::path::PathBuf) {
    let root = tempdir().unwrap();
    let path = root.path().join("worker.sock");
    (root, path)
}

fn payload(bytes: &[u8]) -> Payload {
    Payload {
        bytes: bytes.to_vec(),
        content_type: Some("application/json".to_string()),
        schema: Some("example.notes.v1".to_string()),
        metadata: FrameMetadata::new(),
    }
}

#[tokio::test]
async fn worker_client_receives_dispatch_request_and_sends_success_response() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let server_task = tokio::spawn(async move {
        let mut connection = listener.accept().await.unwrap();
        let request = DispatchWorkerRequest {
            app_id: AppId::new("notes"),
            method: "search".to_string(),
            payload: payload(br#"{\"query\":\"kunkka\"}"#),
        };
        let frame = Frame::Request {
            request_id: RequestId(10),
            session_id: SessionId(20),
            source: EndpointId::new("core"),
            target: EndpointId::new("worker:notes"),
            payload: encode_worker_message(&WorkerProtocolMessage::DispatchWorker(request))
                .unwrap(),
            metadata: FrameMetadata::new(),
        };

        connection.send_frame(&frame).await.unwrap();
        connection.recv_frame().await.unwrap().unwrap()
    });

    let connection = IpcConnection::connect(&socket_path).await.unwrap();
    let mut client =
        WorkerClient::from_connection(connection, WorkerId::new("notes"), SessionId(20));
    let request = client.recv_dispatch().await.unwrap();

    assert_eq!(request.request.app_id.as_str(), "notes");
    assert_eq!(request.request.method, "search");

    client
        .respond_dispatch(
            request,
            DispatchWorkerResponse::Ok(payload(br#"{\"items\":[]}"#)),
        )
        .await
        .unwrap();

    let response_frame = server_task.await.unwrap();
    let Frame::Response {
        request_id,
        payload: response_payload,
        ..
    } = response_frame
    else {
        panic!("expected response frame");
    };

    assert_eq!(request_id, RequestId(10));
    assert_eq!(
        decode_worker_message(&response_payload).unwrap(),
        WorkerProtocolMessage::DispatchWorkerResult(DispatchWorkerResponse::Ok(payload(
            br#"{\"items\":[]}"#
        )))
    );
}

#[tokio::test]
async fn worker_client_sends_app_error_response() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let server_task = tokio::spawn(async move {
        let mut connection = listener.accept().await.unwrap();
        let request = DispatchWorkerRequest {
            app_id: AppId::new("notes"),
            method: "missing".to_string(),
            payload: payload(b"{}"),
        };
        let frame = Frame::Request {
            request_id: RequestId(11),
            session_id: SessionId(21),
            source: EndpointId::new("core"),
            target: EndpointId::new("worker:notes"),
            payload: encode_worker_message(&WorkerProtocolMessage::DispatchWorker(request))
                .unwrap(),
            metadata: FrameMetadata::new(),
        };

        connection.send_frame(&frame).await.unwrap();
        connection.recv_frame().await.unwrap().unwrap()
    });

    let connection = IpcConnection::connect(&socket_path).await.unwrap();
    let mut client =
        WorkerClient::from_connection(connection, WorkerId::new("notes"), SessionId(21));
    let request = client.recv_dispatch().await.unwrap();

    client
        .respond_dispatch(
            request,
            DispatchWorkerResponse::Err(WorkerAppError {
                code: "not_found".to_string(),
                message: "missing note".to_string(),
            }),
        )
        .await
        .unwrap();

    let response_frame = server_task.await.unwrap();
    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };

    assert_eq!(
        decode_worker_message(&payload).unwrap(),
        WorkerProtocolMessage::DispatchWorkerResult(DispatchWorkerResponse::Err(WorkerAppError {
            code: "not_found".to_string(),
            message: "missing note".to_string(),
        }))
    );
}
