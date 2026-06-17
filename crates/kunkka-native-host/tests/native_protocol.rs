use kunkka_native_host::native_protocol::{
    decode_request, error_response, success_response, NativeCommand, NativeErrorCode,
    NativePendingApproval, NativeRequest, NativeResult,
};

#[test]
fn decodes_ping_request() {
    let request = decode_request(br#"{"id":"req-1","command":"ping"}"#).unwrap();

    assert_eq!(
        request,
        NativeRequest {
            id: "req-1".to_string(),
            command: NativeCommand::Ping,
        }
    );
}

#[test]
fn serializes_status_success_response() {
    let response = success_response(
        "req-2",
        NativeResult::Status {
            worker_count: 1,
            socket_path: "/run/user/1000/kunkka/core.sock".to_string(),
            runtime_ready: true,
        },
    );

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": "req-2",
            "ok": true,
            "result": {
                "type": "status",
                "worker_count": 1,
                "socket_path": "/run/user/1000/kunkka/core.sock",
                "runtime_ready": true
            }
        })
    );
}

#[test]
fn serializes_invalid_request_without_id_as_null_id() {
    let response = error_response(None, NativeErrorCode::InvalidRequest, "missing request id");

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": null,
            "ok": false,
            "error": {
                "code": "invalid_request",
                "message": "missing request id"
            }
        })
    );
}

#[test]
fn decodes_dispatch_request_with_json_payload() {
    let request = decode_request(
        br#"{"id":"req-3","command":"dispatch","app_id":"notes","method":"search","payload":{"query":"kunkka"}}"#,
    )
    .unwrap();

    assert_eq!(request.id, "req-3");
    assert_eq!(
        request.command,
        NativeCommand::Dispatch {
            app_id: "notes".to_string(),
            method: "search".to_string(),
            payload: serde_json::json!({"query":"kunkka"}),
        }
    );
}

#[test]
fn rejects_dispatch_request_with_empty_app_id() {
    let err = decode_request(
        br#"{"id":"req-4","command":"dispatch","app_id":"","method":"search","payload":{}}"#,
    )
    .unwrap_err();

    assert!(err.to_string().contains("app_id"));
}

#[test]
fn rejects_dispatch_request_with_empty_method() {
    let err = decode_request(
        br#"{"id":"req-8","command":"dispatch","app_id":"notes","method":"","payload":{}}"#,
    )
    .unwrap_err();

    assert!(err.to_string().contains("method"));
}

#[test]
fn serializes_dispatch_success_response() {
    let response = success_response(
        "req-5",
        NativeResult::Dispatch {
            payload: serde_json::json!({"items": []}),
        },
    );

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": "req-5",
            "ok": true,
            "result": {
                "type": "dispatch",
                "payload": { "items": [] }
            }
        })
    );
}

#[test]
fn serializes_dispatch_app_error_response() {
    let response = success_response(
        "req-6",
        NativeResult::DispatchError {
            code: "not_found".to_string(),
            message: "note not found".to_string(),
        },
    );

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": "req-6",
            "ok": true,
            "result": {
                "type": "dispatch_error",
                "code": "not_found",
                "message": "note not found"
            }
        })
    );
}

#[test]
fn serializes_platform_error_code_as_string() {
    let response = error_response(Some("req-7".to_string()), "app_not_found", "missing app");

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": "req-7",
            "ok": false,
            "error": {
                "code": "app_not_found",
                "message": "missing app"
            }
        })
    );
}

#[test]
fn decodes_approvals_list_request() {
    let request = decode_request(br#"{"id":"req-a1","command":"approvals_list"}"#).unwrap();

    assert_eq!(request.id, "req-a1");
    assert_eq!(request.command, NativeCommand::ApprovalsList);
}

#[test]
fn decodes_approval_approve_request() {
    let request =
        decode_request(br#"{"id":"req-a2","command":"approval_approve","approval_id":"appr-1"}"#)
            .unwrap();

    assert_eq!(request.id, "req-a2");
    assert_eq!(
        request.command,
        NativeCommand::ApprovalApprove {
            approval_id: "appr-1".to_string(),
        }
    );
}

#[test]
fn decodes_approval_reject_request() {
    let request =
        decode_request(br#"{"id":"req-a3","command":"approval_reject","approval_id":"appr-2"}"#)
            .unwrap();

    assert_eq!(request.id, "req-a3");
    assert_eq!(
        request.command,
        NativeCommand::ApprovalReject {
            approval_id: "appr-2".to_string(),
        }
    );
}

#[test]
fn rejects_approval_approve_with_empty_approval_id() {
    let err = decode_request(br#"{"id":"req-a4","command":"approval_approve","approval_id":""}"#)
        .unwrap_err();

    assert!(err.to_string().contains("approval_id"));
}

#[test]
fn rejects_approval_reject_with_empty_approval_id() {
    let err = decode_request(br#"{"id":"req-a5","command":"approval_reject","approval_id":""}"#)
        .unwrap_err();

    assert!(err.to_string().contains("approval_id"));
}

#[test]
fn serializes_pending_approvals_response() {
    let response = success_response(
        "req-a6",
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
        },
    );

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": "req-a6",
            "ok": true,
            "result": {
                "type": "pending_approvals",
                "approvals": [
                    {
                        "approval_id": "appr-1",
                        "app_id": "notes",
                        "capability": "notes.search",
                        "summary": "Search notes"
                    },
                    {
                        "approval_id": "appr-2",
                        "app_id": "files",
                        "capability": "files.read",
                        "summary": "Read files"
                    }
                ]
            }
        })
    );
}

#[test]
fn serializes_empty_pending_approvals_response() {
    let response = success_response(
        "req-a7",
        NativeResult::PendingApprovals { approvals: vec![] },
    );

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": "req-a7",
            "ok": true,
            "result": {
                "type": "pending_approvals",
                "approvals": []
            }
        })
    );
}

#[test]
fn serializes_approval_decision_response() {
    let response = success_response("req-a8", NativeResult::ApprovalDecision);

    let value = serde_json::to_value(&response).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "id": "req-a8",
            "ok": true,
            "result": {
                "type": "approval_decision"
            }
        })
    );
}

#[test]
fn decodes_pending_approvals_from_json() {
    let json = serde_json::json!({
        "type": "pending_approvals",
        "approvals": [
            {
                "approval_id": "appr-1",
                "app_id": "notes",
                "capability": "notes.search",
                "summary": "Search notes"
            }
        ]
    });

    let result: NativeResult = serde_json::from_value(json).unwrap();

    assert_eq!(
        result,
        NativeResult::PendingApprovals {
            approvals: vec![NativePendingApproval {
                approval_id: "appr-1".to_string(),
                app_id: "notes".to_string(),
                capability: "notes.search".to_string(),
                summary: "Search notes".to_string(),
            }],
        }
    );
}

#[test]
fn decodes_approval_decision_from_json() {
    let json = serde_json::json!({
        "type": "approval_decision"
    });

    let result: NativeResult = serde_json::from_value(json).unwrap();

    assert_eq!(result, NativeResult::ApprovalDecision);
}
