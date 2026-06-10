use kunkka_ipc::{
    EndpointId, Frame, FrameMetadata, IpcConnection, IpcListener, Payload, RequestId, SessionId,
};
use tempfile::tempdir;

#[tokio::test]
async fn client_and_server_exchange_request_response() {
    let dir = tempdir().unwrap();
    let socket_path = dir.path().join("core.sock");

    let listener = IpcListener::bind(&socket_path).await.unwrap();

    let server = tokio::spawn(async move {
        let mut conn = listener.accept().await.unwrap();
        let received = conn.recv_frame().await.unwrap().unwrap();

        match received {
            Frame::Request {
                request_id,
                session_id,
                ..
            } => {
                let response = Frame::Response {
                    request_id,
                    session_id,
                    source: EndpointId::new("core"),
                    target: EndpointId::new("client"),
                    payload: Payload::from_bytes(b"pong".to_vec()),
                    metadata: FrameMetadata::new(),
                };

                conn.send_frame(&response).await.unwrap();
            }
            other => panic!("expected request frame, got {other:?}"),
        }
    });

    let mut client = IpcConnection::connect(&socket_path).await.unwrap();

    let request = Frame::Request {
        request_id: RequestId(42),
        session_id: SessionId(7),
        source: EndpointId::new("client"),
        target: EndpointId::new("core"),
        payload: Payload::from_bytes(b"ping".to_vec()),
        metadata: FrameMetadata::new(),
    };

    client.send_frame(&request).await.unwrap();

    let response = client.recv_frame().await.unwrap().unwrap();

    match response {
        Frame::Response {
            request_id,
            payload,
            ..
        } => {
            assert_eq!(request_id, RequestId(42));
            assert_eq!(payload.bytes, b"pong");
        }
        other => panic!("expected response frame, got {other:?}"),
    }

    server.await.unwrap();
}
