use kunkka_core::capability::shell::{
    PendingApprovalReceipt, ShellRunOutcome, ShellRunParams, ShellRunResult,
};
use kunkka_core::capability::{
    decode_capability_response, encode_capability_request, CapabilityError, CapabilityRequest,
};
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreApproveApprovalRequest, CoreControlMessage,
    CoreListApprovalsRequest, CoreListApprovalsResponse, CoreRejectApprovalRequest,
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

fn write_manifest_with_shell(paths: &KunkkaPaths, allow: &[&str], ask: &[&str]) {
    let apps_dir = paths.config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();

    let allow_json = allow
        .iter()
        .map(|command| format!("\"{command}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let ask_json = ask
        .iter()
        .map(|command| format!("\"{command}\""))
        .collect::<Vec<_>>()
        .join(", ");

    std::fs::write(
        apps_dir.join("notes.json"),
        format!(
            r#"{{
                "app_id": "notes",
                "worker": {{
                    "program": "/usr/bin/notes-worker",
                    "args": ["--serve"]
                }},
                "capabilities": {{
                    "shell": {{
                        "allow": [{allow_json}],
                        "ask": [{ask_json}]
                    }}
                }}
            }}"#,
        ),
    )
    .unwrap();
}

fn capability_frame(request_id: u128, params: &ShellRunParams) -> Frame {
    let payload = encode_capability_request(&CapabilityRequest {
        app_id: "notes".to_string(),
        capability: "shell".to_string(),
        method: "run".to_string(),
        params: postcard::to_stdvec(params).unwrap(),
    })
    .unwrap();

    Frame::Request {
        request_id: RequestId(request_id),
        session_id: SessionId(1),
        source: EndpointId::new("worker:notes"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    }
}

fn control_frame(request_id: u128, message: CoreControlMessage) -> Frame {
    Frame::Request {
        request_id: RequestId(request_id),
        session_id: SessionId(1),
        source: EndpointId::new("frontend:test"),
        target: EndpointId::new("core"),
        payload: encode_control_message(&message).unwrap(),
        metadata: FrameMetadata::new(),
    }
}

fn decode_shell_outcome(frame: Frame) -> ShellRunOutcome {
    let Frame::Response { payload, .. } = frame else {
        panic!("expected response frame");
    };
    let bytes = decode_capability_response(&payload)
        .unwrap()
        .result
        .unwrap();
    postcard::from_bytes(&bytes).unwrap()
}

fn decode_shell_result(frame: Frame) -> Result<Vec<u8>, CapabilityError> {
    let Frame::Response { payload, .. } = frame else {
        panic!("expected response frame");
    };
    decode_capability_response(&payload).unwrap().result
}

fn decode_control_response(frame: Frame) -> CoreControlMessage {
    let Frame::Response { payload, .. } = frame else {
        panic!("expected response frame");
    };
    decode_control_message(&payload).unwrap()
}

#[tokio::test]
async fn shell_allow_executes_simple_pipeline() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &["printf", "wc"], &[]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frame = capability_frame(
        1,
        &ShellRunParams {
            command: "printf foo | wc -c".to_string(),
            approval_id: None,
        },
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response = decode_shell_outcome(wait_for(client_task).await.unwrap());

    let ShellRunOutcome::Completed(ShellRunResult {
        stdout, exit_code, ..
    }) = response
    else {
        panic!("expected completed shell result");
    };
    assert_eq!(exit_code, 0);
    assert_eq!(stdout.trim(), "3");
}

#[tokio::test]
async fn shell_deny_rejects_unlisted_command() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &["printf"], &[]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frame = capability_frame(
        1,
        &ShellRunParams {
            command: "python -c 'print(1)'".to_string(),
            approval_id: None,
        },
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let result = decode_shell_result(wait_for(client_task).await.unwrap());
    assert!(matches!(result, Err(CapabilityError { code, .. }) if code == "permission_denied"));
}

#[tokio::test]
async fn shell_ask_can_be_approved_and_retried() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &[], &["printf"]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let pending_frame = capability_frame(
        1,
        &ShellRunParams {
            command: "printf approved".to_string(),
            approval_id: None,
        },
    );

    let pending_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&pending_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let pending = decode_shell_outcome(wait_for(pending_task).await.unwrap());
    let approval_id = match pending {
        ShellRunOutcome::PendingApproval(PendingApprovalReceipt { approval_id }) => approval_id,
        _ => panic!("expected pending approval receipt"),
    };

    let list_frame = control_frame(
        2,
        CoreControlMessage::ListPendingApprovals(CoreListApprovalsRequest),
    );
    let list_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&list_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let listed = decode_control_response(wait_for(list_task).await.unwrap());
    let CoreControlMessage::PendingApprovalsResult(CoreListApprovalsResponse { approvals }) =
        listed
    else {
        panic!("expected pending approvals result");
    };
    assert!(approvals
        .iter()
        .any(|approval| approval.approval_id == approval_id));

    let approve_frame = control_frame(
        3,
        CoreControlMessage::ApprovePendingApproval(CoreApproveApprovalRequest {
            approval_id: approval_id.clone(),
        }),
    );
    let approve_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&approve_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let approved = decode_control_response(wait_for(approve_task).await.unwrap());
    assert!(matches!(
        approved,
        CoreControlMessage::ApprovalDecisionResult(_)
    ));

    let retry_frame = capability_frame(
        4,
        &ShellRunParams {
            command: "printf approved".to_string(),
            approval_id: Some(approval_id),
        },
    );
    let retry_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&retry_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let retried = decode_shell_outcome(wait_for(retry_task).await.unwrap());
    let ShellRunOutcome::Completed(ShellRunResult {
        stdout, exit_code, ..
    }) = retried
    else {
        panic!("expected completed shell result after approval");
    };
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, "approved");
}

#[tokio::test]
async fn shell_reject_blocks_retry() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &[], &["printf"]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let pending_frame = capability_frame(
        1,
        &ShellRunParams {
            command: "printf rejected".to_string(),
            approval_id: None,
        },
    );
    let pending_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&pending_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let pending = decode_shell_outcome(wait_for(pending_task).await.unwrap());
    let approval_id = match pending {
        ShellRunOutcome::PendingApproval(PendingApprovalReceipt { approval_id }) => approval_id,
        _ => panic!("expected pending approval receipt"),
    };

    let reject_frame = control_frame(
        2,
        CoreControlMessage::RejectPendingApproval(CoreRejectApprovalRequest {
            approval_id: approval_id.clone(),
        }),
    );
    let reject_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&reject_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let rejected = decode_control_response(wait_for(reject_task).await.unwrap());
    assert!(matches!(
        rejected,
        CoreControlMessage::ApprovalDecisionResult(_)
    ));

    let retry_frame = capability_frame(
        3,
        &ShellRunParams {
            command: "printf rejected".to_string(),
            approval_id: Some(approval_id),
        },
    );
    let retry_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&retry_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let result = decode_shell_result(wait_for(retry_task).await.unwrap());
    assert!(matches!(result, Err(CapabilityError { code, .. }) if code == "approval_denied"));
}

#[tokio::test]
async fn expired_approval_is_not_returned_by_pending_approvals_query() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &[], &["printf"]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let pending_frame = capability_frame(
        1,
        &ShellRunParams {
            command: "printf expiring".to_string(),
            approval_id: None,
        },
    );
    let pending_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&pending_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let pending = decode_shell_outcome(wait_for(pending_task).await.unwrap());
    let approval_id = match pending {
        ShellRunOutcome::PendingApproval(PendingApprovalReceipt { approval_id }) => approval_id,
        _ => panic!("expected pending approval receipt"),
    };

    runtime.expire_pending_approval_for_test(&approval_id);

    let list_frame = control_frame(
        2,
        CoreControlMessage::ListPendingApprovals(CoreListApprovalsRequest),
    );
    let list_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&list_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let listed = decode_control_response(wait_for(list_task).await.unwrap());
    let CoreControlMessage::PendingApprovalsResult(CoreListApprovalsResponse { approvals }) =
        listed
    else {
        panic!("expected pending approvals result");
    };

    assert!(approvals
        .iter()
        .all(|approval| approval.approval_id != approval_id));
}

#[tokio::test]
async fn shell_timeout_returns_timeout_error() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &["sleep"], &[]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let shell_frame = capability_frame(
        1,
        &ShellRunParams {
            command: "sleep 60".to_string(),
            approval_id: None,
        },
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&shell_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    let result = tokio::time::timeout(
        Duration::from_secs(35),
        runtime.run_once(),
    )
    .await;

    let response_frame = tokio::time::timeout(Duration::from_secs(5), client_task)
        .await
        .expect("client task timed out")
        .unwrap();

    let err = decode_shell_result(response_frame).unwrap_err();
    assert_eq!(err.code, "timeout");
    assert!(err.message.contains("timed out"));

    let _ = result;
}
