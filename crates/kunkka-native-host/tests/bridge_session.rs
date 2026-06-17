use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcListener, Payload, RequestId, SessionId};
use kunkka_native_host::bridge::NativeHostSession;
use kunkka_native_host::native_protocol::{NativeCommand, NativeRequest, NativeResult};
use kunkka_protocol::core_control::{encode_control_message, CoreControlMessage, CorePingResponse};
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

fn write_manifest(paths: &KunkkaPaths, body: &str) {
    use std::fs;

    let apps_dir = paths.config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();
    fs::write(apps_dir.join("example-app.json"), body).unwrap();
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

fn ping_request(id: &str) -> NativeRequest {
    NativeRequest {
        id: id.to_string(),
        command: NativeCommand::Ping,
    }
}

fn pong_payload() -> Payload {
    encode_control_message(&CoreControlMessage::Pong(CorePingResponse)).unwrap()
}

async fn run_fake_core_once(listener: IpcListener, response: Frame) {
    let mut connection = listener.accept().await.unwrap();
    connection.recv_frame().await.unwrap().unwrap();
    connection.send_frame(&response).await.unwrap();
}

#[tokio::test]
async fn session_ping_returns_pong() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let mut session = NativeHostSession::new(paths.socket_path.clone());
    let response = wait_for(session.handle_request(ping_request("req-1"))).await;

    assert!(response.ok);
    assert_eq!(response.id.as_deref(), Some("req-1"));
    assert_eq!(response.result, Some(NativeResult::Pong));

    drop(session);
    wait_for(runtime_task).await.unwrap();
}

#[tokio::test]
async fn session_reuses_connection_for_ping_then_status() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let mut session = NativeHostSession::new(paths.socket_path.clone());

    let ping = wait_for(session.handle_request(ping_request("req-1"))).await;
    assert_eq!(ping.result, Some(NativeResult::Pong));

    let status = wait_for(session.handle_request(NativeRequest {
        id: "req-2".to_string(),
        command: NativeCommand::Status,
    }))
    .await;

    let Some(NativeResult::Status {
        worker_count,
        socket_path,
        runtime_ready,
    }) = status.result
    else {
        panic!("expected status result");
    };

    assert_eq!(worker_count, 0);
    assert_eq!(
        socket_path,
        paths.socket_path.to_string_lossy().into_owned()
    );
    assert!(runtime_ready);

    drop(session);
    wait_for(runtime_task).await.unwrap();
}

#[tokio::test]
async fn session_status_reports_registered_worker() {
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

    wait_for(runtime.run_once()).await.unwrap();
    assert!(wait_for(register_task).await.unwrap().accepted);

    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });
    let mut session = NativeHostSession::new(paths.socket_path.clone());
    let response = wait_for(session.handle_request(NativeRequest {
        id: "req-status".to_string(),
        command: NativeCommand::Status,
    }))
    .await;

    let Some(NativeResult::Status { worker_count, .. }) = response.result else {
        panic!("expected status result");
    };
    assert_eq!(worker_count, 1);

    drop(session);
    wait_for(runtime_task).await.unwrap();
}

#[tokio::test]
async fn core_unavailable_returns_error_response() {
    let (_root, paths) = test_paths();
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    let response = wait_for(session.handle_request(ping_request("req-1"))).await;

    assert!(!response.ok);
    assert_eq!(response.id.as_deref(), Some("req-1"));
    assert_eq!(response.error.unwrap().code.to_string(), "core_unavailable");
}

#[tokio::test]
async fn ipc_failure_clears_connection_and_next_request_reconnects() {
    let (_root, paths) = test_paths();
    paths.ensure_dirs().unwrap();

    let listener = IpcListener::bind(&paths.socket_path).await.unwrap();
    let failing_server = tokio::spawn(async move {
        let _connection = listener.accept().await.unwrap();
    });

    let mut session = NativeHostSession::new(paths.socket_path.clone());
    let failed = wait_for(session.handle_request(ping_request("req-fail"))).await;

    assert!(!failed.ok);
    assert_eq!(failed.error.unwrap().code.to_string(), "core_ipc_error");
    wait_for(failing_server).await.unwrap();

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let recovered = wait_for(session.handle_request(ping_request("req-ok"))).await;

    assert!(recovered.ok);
    assert_eq!(recovered.result, Some(NativeResult::Pong));

    drop(session);
    wait_for(runtime_task).await.unwrap();
}

#[tokio::test]
async fn unexpected_response_request_id_clears_connection_and_next_request_reconnects() {
    let (_root, paths) = test_paths();
    paths.ensure_dirs().unwrap();

    let bad_response = Frame::Response {
        request_id: RequestId(999),
        session_id: SessionId(1),
        source: EndpointId::new("core"),
        target: EndpointId::new("native-host"),
        payload: pong_payload(),
        metadata: FrameMetadata::new(),
    };
    let listener = IpcListener::bind(&paths.socket_path).await.unwrap();
    let failing_server = tokio::spawn(run_fake_core_once(listener, bad_response));

    let mut session = NativeHostSession::new(paths.socket_path.clone());
    let failed = wait_for(session.handle_request(ping_request("req-bad-id"))).await;

    assert!(!failed.ok);
    assert_eq!(
        failed.error.unwrap().code.to_string(),
        "unexpected_core_response"
    );
    wait_for(failing_server).await.unwrap();

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let recovered = wait_for(session.handle_request(ping_request("req-ok"))).await;

    assert!(recovered.ok);
    assert_eq!(recovered.result, Some(NativeResult::Pong));

    drop(session);
    wait_for(runtime_task).await.unwrap();
}

#[tokio::test]
async fn unexpected_non_response_frame_clears_connection_and_next_request_reconnects() {
    let (_root, paths) = test_paths();
    paths.ensure_dirs().unwrap();

    let bad_response = Frame::Heartbeat {
        session_id: SessionId(1),
        source: EndpointId::new("core"),
        target: EndpointId::new("native-host"),
        metadata: FrameMetadata::new(),
    };
    let listener = IpcListener::bind(&paths.socket_path).await.unwrap();
    let failing_server = tokio::spawn(run_fake_core_once(listener, bad_response));

    let mut session = NativeHostSession::new(paths.socket_path.clone());
    let failed = wait_for(session.handle_request(ping_request("req-bad-frame"))).await;

    assert!(!failed.ok);
    assert_eq!(
        failed.error.unwrap().code.to_string(),
        "unexpected_core_response"
    );
    wait_for(failing_server).await.unwrap();

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let recovered = wait_for(session.handle_request(ping_request("req-ok"))).await;

    assert!(recovered.ok);
    assert_eq!(recovered.result, Some(NativeResult::Pong));

    drop(session);
    wait_for(runtime_task).await.unwrap();
}

fn dispatch_request(id: &str) -> NativeRequest {
    NativeRequest {
        id: id.to_string(),
        command: NativeCommand::Dispatch {
            app_id: "example-app".to_string(),
            method: "search".to_string(),
            payload: serde_json::json!({"query":"kunkka"}),
        },
    }
}

#[tokio::test]
async fn session_reuses_connection_for_status_then_dispatch() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        r#"{
        "app_id": "example-app",
        "worker": {
            "program": "/usr/bin/example-worker",
            "args": []
        },
        "permissions": {
            "frontend_dispatch": {
                "allowed_methods": ["search"]
            }
        }
    }"#,
    );
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let worker_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("worker-1"))
                .await
                .unwrap();
            let registration = client.register(worker_request()).await.unwrap();
            let dispatch = wait_for(client.recv_dispatch()).await.unwrap();
            assert_eq!(dispatch.request.method, "search");
            assert_eq!(
                dispatch.request.payload.content_type.as_deref(),
                Some("application/json")
            );
            assert_eq!(dispatch.request.payload.bytes, br#"{"query":"kunkka"}"#);
            client
                .respond_dispatch(
                    dispatch,
                    DispatchWorkerResponse::Ok(Payload {
                        bytes: br#"{"items":[]}"#.to_vec(),
                        content_type: Some("application/json".to_string()),
                        schema: None,
                        metadata: FrameMetadata::new(),
                    }),
                )
                .await
                .unwrap();
            registration
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    let status = wait_for(session.handle_request(NativeRequest {
        id: "req-status".to_string(),
        command: NativeCommand::Status,
    }))
    .await;
    assert!(matches!(status.result, Some(NativeResult::Status { .. })));

    let dispatch = wait_for(session.handle_request(dispatch_request("req-dispatch"))).await;
    assert_eq!(dispatch.id.as_deref(), Some("req-dispatch"));
    assert_eq!(
        dispatch.result,
        Some(NativeResult::Dispatch {
            payload: serde_json::json!({"items": []}),
        })
    );

    assert!(wait_for(worker_task).await.unwrap().accepted);
    drop(session);
    wait_for(runtime_task).await.unwrap();
}

fn approvals_list_request(id: &str) -> NativeRequest {
    NativeRequest {
        id: id.to_string(),
        command: NativeCommand::ApprovalsList,
    }
}

#[tokio::test]
async fn session_approvals_list_returns_empty_pending_approvals() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let mut session = NativeHostSession::new(paths.socket_path.clone());
    let response = wait_for(session.handle_request(approvals_list_request("req-al-1"))).await;

    assert!(response.ok);
    assert_eq!(response.id.as_deref(), Some("req-al-1"));
    assert_eq!(
        response.result,
        Some(NativeResult::PendingApprovals { approvals: vec![] })
    );

    drop(session);
    wait_for(runtime_task).await.unwrap();
}

#[tokio::test]
async fn session_reuses_connection_for_ping_then_approvals_list() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let mut session = NativeHostSession::new(paths.socket_path.clone());

    let ping = wait_for(session.handle_request(ping_request("req-ping"))).await;
    assert_eq!(ping.result, Some(NativeResult::Pong));

    let approvals = wait_for(session.handle_request(approvals_list_request("req-al-2"))).await;
    assert_eq!(
        approvals.result,
        Some(NativeResult::PendingApprovals { approvals: vec![] })
    );

    drop(session);
    wait_for(runtime_task).await.unwrap();
}

#[tokio::test]
async fn session_approve_returns_core_unavailable_when_no_core() {
    let (_root, paths) = test_paths();
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    let response = wait_for(session.handle_request(NativeRequest {
        id: "req-approve".to_string(),
        command: NativeCommand::ApprovalApprove {
            approval_id: "appr-1".to_string(),
        },
    }))
    .await;

    assert!(!response.ok);
    assert_eq!(response.id.as_deref(), Some("req-approve"));
    assert_eq!(response.error.unwrap().code.to_string(), "core_unavailable");
}

#[tokio::test]
async fn session_reject_returns_core_unavailable_when_no_core() {
    let (_root, paths) = test_paths();
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    let response = wait_for(session.handle_request(NativeRequest {
        id: "req-reject".to_string(),
        command: NativeCommand::ApprovalReject {
            approval_id: "appr-2".to_string(),
        },
    }))
    .await;

    assert!(!response.ok);
    assert_eq!(response.id.as_deref(), Some("req-reject"));
    assert_eq!(response.error.unwrap().code.to_string(), "core_unavailable");
}
