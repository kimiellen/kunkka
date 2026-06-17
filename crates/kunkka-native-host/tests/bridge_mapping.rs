use kunkka_native_host::bridge::{
    core_message_for_command, frontend_dispatch_request_for_command,
    native_result_for_core_response, native_result_for_frontend_dispatch_response,
};
use kunkka_native_host::native_protocol::{NativeCommand, NativePendingApproval, NativeResult};
use kunkka_native_host::NativeHostError;
use kunkka_protocol::core_control::{
    CoreApprovalDecisionResponse, CoreApproveApprovalRequest, CoreControlMessage,
    CoreListApprovalsRequest, CoreListApprovalsResponse, CorePingRequest, CorePingResponse,
    CoreRejectApprovalRequest, CoreStatusRequest, CoreStatusResponse, PendingApproval,
};
use kunkka_protocol::frontend_dispatch::FrontendDispatchResponse;

#[test]
fn maps_ping_command_to_core_ping() {
    let message = core_message_for_command(&NativeCommand::Ping).unwrap();

    assert_eq!(message, CoreControlMessage::Ping(CorePingRequest));
}

#[test]
fn maps_status_command_to_core_status() {
    let message = core_message_for_command(&NativeCommand::Status).unwrap();

    assert_eq!(message, CoreControlMessage::Status(CoreStatusRequest));
}

#[test]
fn maps_status_result_to_native_status_result() {
    let result = native_result_for_core_response(
        &NativeCommand::Status,
        CoreControlMessage::StatusResult(CoreStatusResponse {
            worker_count: 3,
            socket_path: "/run/user/1000/kunkka/core.sock".to_string(),
            runtime_ready: true,
        }),
    )
    .unwrap();

    assert_eq!(
        result,
        NativeResult::Status {
            worker_count: 3,
            socket_path: "/run/user/1000/kunkka/core.sock".to_string(),
            runtime_ready: true,
        }
    );
}

#[test]
fn rejects_unexpected_core_response_for_ping() {
    let err = native_result_for_core_response(
        &NativeCommand::Ping,
        CoreControlMessage::StatusResult(CoreStatusResponse {
            worker_count: 0,
            socket_path: "/run/user/1000/kunkka/core.sock".to_string(),
            runtime_ready: true,
        }),
    )
    .unwrap_err();

    assert!(matches!(err, NativeHostError::UnexpectedCoreResponse(_)));
}

#[test]
fn rejects_unexpected_core_response_for_status() {
    let err = native_result_for_core_response(
        &NativeCommand::Status,
        CoreControlMessage::Pong(CorePingResponse),
    )
    .unwrap_err();

    assert!(matches!(err, NativeHostError::UnexpectedCoreResponse(_)));
}

#[test]
fn maps_pong_to_native_pong() {
    let result = native_result_for_core_response(
        &NativeCommand::Ping,
        CoreControlMessage::Pong(CorePingResponse),
    )
    .unwrap();

    assert_eq!(result, NativeResult::Pong);
}

#[test]
fn maps_dispatch_command_to_frontend_dispatch_request() {
    let request = frontend_dispatch_request_for_command(&NativeCommand::Dispatch {
        app_id: "notes".to_string(),
        method: "search".to_string(),
        payload: serde_json::json!({"query":"kunkka"}),
    })
    .unwrap();

    assert_eq!(request.app_id, "notes");
    assert_eq!(request.method, "search");
    assert_eq!(
        request.payload.content_type.as_deref(),
        Some("application/json")
    );
    assert_eq!(request.payload.schema, None);
    assert_eq!(request.payload.bytes, br#"{"query":"kunkka"}"#);
}

#[test]
fn maps_frontend_dispatch_success_to_native_dispatch_result() {
    let result = native_result_for_frontend_dispatch_response(FrontendDispatchResponse::Ok(
        kunkka_ipc::Payload {
            bytes: br#"{"items":[]}"#.to_vec(),
            content_type: Some("application/json".to_string()),
            schema: None,
            metadata: kunkka_ipc::FrameMetadata::new(),
        },
    ))
    .unwrap();

    assert_eq!(
        result,
        NativeResult::Dispatch {
            payload: serde_json::json!({"items": []}),
        }
    );
}

#[test]
fn maps_frontend_dispatch_app_error_to_native_dispatch_error() {
    let result = native_result_for_frontend_dispatch_response(FrontendDispatchResponse::AppError {
        code: "not_found".to_string(),
        message: "note not found".to_string(),
    })
    .unwrap();

    assert_eq!(
        result,
        NativeResult::DispatchError {
            code: "not_found".to_string(),
            message: "note not found".to_string(),
        }
    );
}

#[test]
fn rejects_frontend_dispatch_success_with_non_json_content_type() {
    let err = native_result_for_frontend_dispatch_response(FrontendDispatchResponse::Ok(
        kunkka_ipc::Payload {
            bytes: b"raw".to_vec(),
            content_type: Some("application/octet-stream".to_string()),
            schema: None,
            metadata: kunkka_ipc::FrameMetadata::new(),
        },
    ))
    .unwrap_err();

    assert!(matches!(err, NativeHostError::UnexpectedCoreResponse(_)));
}

#[test]
fn maps_approvals_list_command_to_list_pending_approvals() {
    let message = core_message_for_command(&NativeCommand::ApprovalsList).unwrap();

    assert_eq!(
        message,
        CoreControlMessage::ListPendingApprovals(CoreListApprovalsRequest)
    );
}

#[test]
fn maps_approval_approve_command_to_approve_pending_approval() {
    let message = core_message_for_command(&NativeCommand::ApprovalApprove {
        approval_id: "appr-1".to_string(),
    })
    .unwrap();

    assert_eq!(
        message,
        CoreControlMessage::ApprovePendingApproval(CoreApproveApprovalRequest {
            approval_id: "appr-1".to_string(),
        })
    );
}

#[test]
fn maps_approval_reject_command_to_reject_pending_approval() {
    let message = core_message_for_command(&NativeCommand::ApprovalReject {
        approval_id: "appr-2".to_string(),
    })
    .unwrap();

    assert_eq!(
        message,
        CoreControlMessage::RejectPendingApproval(CoreRejectApprovalRequest {
            approval_id: "appr-2".to_string(),
        })
    );
}

#[test]
fn maps_pending_approvals_result_to_native_pending_approvals() {
    let result = native_result_for_core_response(
        &NativeCommand::ApprovalsList,
        CoreControlMessage::PendingApprovalsResult(CoreListApprovalsResponse {
            approvals: vec![
                PendingApproval {
                    approval_id: "appr-1".to_string(),
                    app_id: "notes".to_string(),
                    capability: "notes.search".to_string(),
                    summary: "Search notes".to_string(),
                },
                PendingApproval {
                    approval_id: "appr-2".to_string(),
                    app_id: "files".to_string(),
                    capability: "files.read".to_string(),
                    summary: "Read files".to_string(),
                },
            ],
        }),
    )
    .unwrap();

    assert_eq!(
        result,
        NativeResult::PendingApprovals {
            approvals: vec![
                NativePendingApproval {
                    approval_id: "appr-1".to_string(),
                    app_id: "notes".to_string(),
                    capability: "notes.search".to_string(),
                    summary: "Search notes".to_string(),
                },
                NativePendingApproval {
                    approval_id: "appr-2".to_string(),
                    app_id: "files".to_string(),
                    capability: "files.read".to_string(),
                    summary: "Read files".to_string(),
                },
            ],
        }
    );
}

#[test]
fn maps_empty_pending_approvals_result_to_empty_list() {
    let result = native_result_for_core_response(
        &NativeCommand::ApprovalsList,
        CoreControlMessage::PendingApprovalsResult(CoreListApprovalsResponse { approvals: vec![] }),
    )
    .unwrap();

    assert_eq!(result, NativeResult::PendingApprovals { approvals: vec![] });
}

#[test]
fn maps_approval_approve_decision_result_to_approval_decision() {
    let result = native_result_for_core_response(
        &NativeCommand::ApprovalApprove {
            approval_id: "appr-1".to_string(),
        },
        CoreControlMessage::ApprovalDecisionResult(CoreApprovalDecisionResponse),
    )
    .unwrap();

    assert_eq!(result, NativeResult::ApprovalDecision);
}

#[test]
fn maps_approval_reject_decision_result_to_approval_decision() {
    let result = native_result_for_core_response(
        &NativeCommand::ApprovalReject {
            approval_id: "appr-1".to_string(),
        },
        CoreControlMessage::ApprovalDecisionResult(CoreApprovalDecisionResponse),
    )
    .unwrap();

    assert_eq!(result, NativeResult::ApprovalDecision);
}

#[test]
fn rejects_unexpected_core_response_for_approvals_list() {
    let err = native_result_for_core_response(
        &NativeCommand::ApprovalsList,
        CoreControlMessage::Pong(CorePingResponse),
    )
    .unwrap_err();

    assert!(matches!(err, NativeHostError::UnexpectedCoreResponse(_)));
}

#[test]
fn rejects_unexpected_core_response_for_approval_approve() {
    let err = native_result_for_core_response(
        &NativeCommand::ApprovalApprove {
            approval_id: "appr-1".to_string(),
        },
        CoreControlMessage::Pong(CorePingResponse),
    )
    .unwrap_err();

    assert!(matches!(err, NativeHostError::UnexpectedCoreResponse(_)));
}

#[test]
fn rejects_unexpected_core_response_for_approval_reject() {
    let err = native_result_for_core_response(
        &NativeCommand::ApprovalReject {
            approval_id: "appr-1".to_string(),
        },
        CoreControlMessage::Pong(CorePingResponse),
    )
    .unwrap_err();

    assert!(matches!(err, NativeHostError::UnexpectedCoreResponse(_)));
}

#[test]
fn rejects_approve_decision_result_for_wrong_command() {
    let err = native_result_for_core_response(
        &NativeCommand::Ping,
        CoreControlMessage::ApprovalDecisionResult(CoreApprovalDecisionResponse),
    )
    .unwrap_err();

    assert!(matches!(err, NativeHostError::UnexpectedCoreResponse(_)));
}
