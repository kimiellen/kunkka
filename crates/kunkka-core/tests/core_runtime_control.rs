use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_core::CoreError;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreApproveApprovalRequest, CoreControlMessage,
    CoreGetThemeRequest, CoreListApprovalsRequest, CoreListApprovalsResponse, CorePingRequest,
    CorePingResponse, CoreRejectApprovalRequest, CoreSetThemeRequest, CoreStatusRequest,
    ThemeChangedEvent, ThemeFlavor,
};
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message, FrontendDispatchMessage,
    FrontendDispatchRequest, FrontendDispatchResponse,
};
use kunkka_worker_sdk::{
    AppId, DispatchWorkerResponse, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId,
};
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

#[tokio::test]
async fn set_theme_switches_flavor_and_persists() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();

            let payload =
                encode_control_message(&CoreControlMessage::SetTheme(CoreSetThemeRequest {
                    flavor: ThemeFlavor::Latte,
                }))
                .unwrap();
            let frame = Frame::Request {
                request_id: RequestId(201),
                session_id: SessionId(1),
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

    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };
    assert_eq!(
        decode_control_message(&payload).unwrap(),
        CoreControlMessage::SetThemeResult(kunkka_protocol::core_control::CoreSetThemeResponse)
    );

    assert_eq!(
        runtime.theme_manager().active_flavor(),
        kunkka_core::theme::ThemeFlavor::Latte
    );

    let json = std::fs::read_to_string(paths.config_dir.join("theme.json")).unwrap();
    assert!(json.contains("\"latte\""));
}

#[tokio::test]
async fn get_theme_returns_current_flavor() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            let payload =
                encode_control_message(&CoreControlMessage::GetTheme(CoreGetThemeRequest)).unwrap();
            let frame = Frame::Request {
                request_id: RequestId(202),
                session_id: SessionId(1),
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

    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(
        decoded,
        CoreControlMessage::GetThemeResult(kunkka_protocol::core_control::CoreGetThemeResponse {
            flavor: ThemeFlavor::Macchiato,
        })
    );
}

#[tokio::test]
async fn set_theme_then_get_theme_reflects_change() {
    let (_root, paths) = test_paths();
    let runtime = prepare_core_runtime(&paths).await.unwrap();

    let runtime_task = tokio::spawn(async move {
        let _ = runtime.run().await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();

            let set_payload =
                encode_control_message(&CoreControlMessage::SetTheme(CoreSetThemeRequest {
                    flavor: ThemeFlavor::Latte,
                }))
                .unwrap();
            let set_frame = Frame::Request {
                request_id: RequestId(301),
                session_id: SessionId(1),
                source: EndpointId::new("cli"),
                target: EndpointId::new("core"),
                payload: set_payload,
                metadata: FrameMetadata::new(),
            };
            connection.send_frame(&set_frame).await.unwrap();
            let set_response = connection.recv_frame().await.unwrap().unwrap();

            let theme_event = connection.recv_frame().await.unwrap().unwrap();

            let get_payload =
                encode_control_message(&CoreControlMessage::GetTheme(CoreGetThemeRequest)).unwrap();
            let get_frame = Frame::Request {
                request_id: RequestId(302),
                session_id: SessionId(1),
                source: EndpointId::new("cli"),
                target: EndpointId::new("core"),
                payload: get_payload,
                metadata: FrameMetadata::new(),
            };
            connection.send_frame(&get_frame).await.unwrap();
            let get_response = connection.recv_frame().await.unwrap().unwrap();

            (set_response, theme_event, get_response)
        }
    });

    let (set_response, theme_event, get_response) = client_task.await.unwrap();

    let Frame::Response {
        payload: set_payload,
        ..
    } = set_response
    else {
        panic!("expected set response frame");
    };
    assert_eq!(
        decode_control_message(&set_payload).unwrap(),
        CoreControlMessage::SetThemeResult(kunkka_protocol::core_control::CoreSetThemeResponse)
    );

    let Frame::Event {
        name,
        payload: event_payload,
        ..
    } = theme_event
    else {
        panic!("expected theme changed event frame");
    };
    assert_eq!(name, "theme_changed");
    assert_eq!(
        decode_control_message(&event_payload).unwrap(),
        CoreControlMessage::ThemeChanged(ThemeChangedEvent {
            flavor: ThemeFlavor::Latte,
        })
    );

    let Frame::Response {
        payload: get_payload,
        ..
    } = get_response
    else {
        panic!("expected get response frame");
    };
    assert_eq!(
        decode_control_message(&get_payload).unwrap(),
        CoreControlMessage::GetThemeResult(kunkka_protocol::core_control::CoreGetThemeResponse {
            flavor: ThemeFlavor::Latte,
        })
    );

    let json = std::fs::read_to_string(paths.config_dir.join("theme.json")).unwrap();
    assert!(json.contains("\"latte\""));

    runtime_task.abort();
}

#[tokio::test]
async fn list_pending_approvals_returns_empty_when_none() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            let payload = encode_control_message(&CoreControlMessage::ListPendingApprovals(
                CoreListApprovalsRequest,
            ))
            .unwrap();
            let frame = Frame::Request {
                request_id: RequestId(401),
                session_id: SessionId(1),
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

    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };
    let decoded = decode_control_message(&payload).unwrap();
    let CoreControlMessage::PendingApprovalsResult(CoreListApprovalsResponse { approvals }) =
        decoded
    else {
        panic!("expected pending approvals result");
    };
    assert!(approvals.is_empty());
}

#[tokio::test]
async fn approve_nonexistent_approval_id_is_noop() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            let payload = encode_control_message(&CoreControlMessage::ApprovePendingApproval(
                CoreApproveApprovalRequest {
                    approval_id: "nonexistent-id".to_string(),
                },
            ))
            .unwrap();
            let frame = Frame::Request {
                request_id: RequestId(402),
                session_id: SessionId(1),
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

    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };
    assert_eq!(
        decode_control_message(&payload).unwrap(),
        CoreControlMessage::ApprovalDecisionResult(
            kunkka_protocol::core_control::CoreApprovalDecisionResponse
        )
    );
}

#[tokio::test]
async fn reject_nonexistent_approval_id_is_noop() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            let payload = encode_control_message(&CoreControlMessage::RejectPendingApproval(
                CoreRejectApprovalRequest {
                    approval_id: "nonexistent-id".to_string(),
                },
            ))
            .unwrap();
            let frame = Frame::Request {
                request_id: RequestId(403),
                session_id: SessionId(1),
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

    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };
    assert_eq!(
        decode_control_message(&payload).unwrap(),
        CoreControlMessage::ApprovalDecisionResult(
            kunkka_protocol::core_control::CoreApprovalDecisionResponse
        )
    );
}

#[tokio::test]
async fn frontend_connection_handles_multiple_dispatch_requests() {
    let (_root, paths) = test_paths();
    std::fs::create_dir_all(paths.config_dir.join("apps")).unwrap();
    std::fs::write(
        paths.config_dir.join("apps/notes.json"),
        r#"{
            "app_id": "notes",
            "worker": { "program": "/usr/bin/notes-worker", "args": [] },
            "permissions": { "frontend_dispatch": { "allowed_methods": ["search"] } }
        }"#,
    )
    .unwrap();

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let register_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("notes"))
                .await
                .unwrap();
            let reg = client
                .register(RegisterWorkerRequest {
                    worker_id: WorkerId::new("notes"),
                    app_id: AppId::new("notes"),
                    capabilities: vec![],
                })
                .await
                .unwrap();
            assert!(reg.accepted);

            let ctx = client.recv_dispatch().await.unwrap();
            client
                .respond_dispatch(
                    ctx,
                    DispatchWorkerResponse::Ok(Payload {
                        bytes: b"result1".to_vec(),
                        content_type: Some("application/json".to_string()),
                        schema: None,
                        metadata: FrameMetadata::new(),
                    }),
                )
                .await
                .unwrap();

            let ctx2 = client.recv_dispatch().await.unwrap();
            client
                .respond_dispatch(
                    ctx2,
                    DispatchWorkerResponse::Ok(Payload {
                        bytes: b"result2".to_vec(),
                        content_type: Some("application/json".to_string()),
                        schema: None,
                        metadata: FrameMetadata::new(),
                    }),
                )
                .await
                .unwrap();
        }
    });

    runtime.run_once().await.unwrap();

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();

            let make_dispatch = |req_id: u128, method: &str| {
                let payload = encode_frontend_dispatch_message(&FrontendDispatchMessage::Dispatch(
                    FrontendDispatchRequest {
                        app_id: "notes".to_string(),
                        method: method.to_string(),
                        payload: Payload {
                            bytes: b"{}".to_vec(),
                            content_type: Some("application/json".to_string()),
                            schema: None,
                            metadata: FrameMetadata::new(),
                        },
                    },
                ))
                .unwrap();
                Frame::Request {
                    request_id: RequestId(req_id),
                    session_id: SessionId(1),
                    source: EndpointId::new("cli"),
                    target: EndpointId::new("core"),
                    payload,
                    metadata: FrameMetadata::new(),
                }
            };

            connection
                .send_frame(&make_dispatch(501, "search"))
                .await
                .unwrap();
            let resp1 = connection.recv_frame().await.unwrap().unwrap();

            connection
                .send_frame(&make_dispatch(502, "search"))
                .await
                .unwrap();
            let resp2 = connection.recv_frame().await.unwrap().unwrap();

            (resp1, resp2)
        }
    });

    runtime.run_once().await.unwrap();

    let (resp1, resp2) = client_task.await.unwrap();
    let _ = register_task.await;

    let Frame::Response {
        request_id: rid1,
        payload: p1,
        ..
    } = resp1
    else {
        panic!("expected response 1");
    };
    assert_eq!(rid1, RequestId(501));
    let msg1 = decode_frontend_dispatch_message(&p1).unwrap();
    assert!(matches!(
        msg1,
        FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::Ok(_))
    ));

    let Frame::Response {
        request_id: rid2,
        payload: p2,
        ..
    } = resp2
    else {
        panic!("expected response 2");
    };
    assert_eq!(rid2, RequestId(502));
    let msg2 = decode_frontend_dispatch_message(&p2).unwrap();
    assert!(matches!(
        msg2,
        FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::Ok(_))
    ));
}
