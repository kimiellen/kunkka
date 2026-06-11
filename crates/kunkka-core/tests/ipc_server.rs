use kunkka_core::ipc_server::CoreIpcServer;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, SessionId};
use std::fs;
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

#[tokio::test]
async fn binds_socket_and_accepts_client_connection() {
    let (_root, paths) = test_paths();
    paths.ensure_dirs().unwrap();

    let server = CoreIpcServer::bind(&paths).await.unwrap();
    assert_eq!(server.socket_path(), paths.socket_path.as_path());
    assert!(paths.socket_path.exists());

    let server_task = tokio::spawn(async move {
        let mut connection = server.accept_one().await.unwrap();
        let frame = connection.recv_frame().await.unwrap().unwrap();

        match frame {
            Frame::Heartbeat {
                session_id,
                source,
                target,
                ..
            } => {
                assert_eq!(session_id, SessionId(99));
                assert_eq!(source.as_str(), "client");
                assert_eq!(target.as_str(), "core");

                let response = Frame::Event {
                    session_id,
                    source: EndpointId::new("core"),
                    target: EndpointId::new("client"),
                    name: "accepted".to_string(),
                    payload: Payload::from_bytes(Vec::new()),
                    metadata: FrameMetadata::new(),
                };

                connection.send_frame(&response).await.unwrap();
            }
            other => panic!("expected heartbeat frame, got {other:?}"),
        }
    });

    let mut client = IpcConnection::connect(&paths.socket_path).await.unwrap();

    let heartbeat = Frame::Heartbeat {
        session_id: SessionId(99),
        source: EndpointId::new("client"),
        target: EndpointId::new("core"),
        metadata: FrameMetadata::new(),
    };

    client.send_frame(&heartbeat).await.unwrap();

    let response = client.recv_frame().await.unwrap().unwrap();

    match response {
        Frame::Event { name, .. } => assert_eq!(name, "accepted"),
        other => panic!("expected accepted event, got {other:?}"),
    }

    server_task.await.unwrap();
}

#[tokio::test]
async fn bind_replaces_existing_socket_file() {
    let (_root, paths) = test_paths();
    paths.ensure_dirs().unwrap();

    fs::write(&paths.socket_path, b"stale socket").unwrap();

    let server = CoreIpcServer::bind(&paths).await.unwrap();

    assert_eq!(server.socket_path(), paths.socket_path.as_path());
    assert!(paths.socket_path.exists());
}
