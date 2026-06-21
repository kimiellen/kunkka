use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreApprovalDecisionResponse,
    CoreApproveApprovalRequest, CoreControlMessage, CoreListApprovalsRequest,
    CoreListApprovalsResponse, CorePingRequest, CoreRejectApprovalRequest, CoreStatusResponse,
    PendingApproval, CORE_CONTROL_CONTENT_TYPE, CORE_CONTROL_SCHEMA,
};

fn assert_worker_count_is_u64(_: u64) {}

#[test]
fn ping_payload_roundtrips_with_control_metadata() {
    let message = CoreControlMessage::Ping(CorePingRequest);

    let payload = encode_control_message(&message).unwrap();

    assert_eq!(
        payload.content_type.as_deref(),
        Some(CORE_CONTROL_CONTENT_TYPE)
    );
    assert_eq!(payload.schema.as_deref(), Some(CORE_CONTROL_SCHEMA));
    assert!(payload.metadata.is_empty());

    let decoded = decode_control_message(&payload).unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn status_result_payload_roundtrips_with_runtime_state() {
    let message = CoreControlMessage::StatusResult(CoreStatusResponse {
        worker_count: 2,
        socket_path: "/run/user/1000/kunkka/core.sock".to_string(),
        runtime_ready: true,
    });

    let payload = encode_control_message(&message).unwrap();
    let decoded = decode_control_message(&payload).unwrap();

    if let CoreControlMessage::StatusResult(response) = &decoded {
        assert_worker_count_is_u64(response.worker_count);
    }

    assert_eq!(decoded, message);
}

#[test]
fn pending_approvals_result_roundtrips() {
    let message = CoreControlMessage::PendingApprovalsResult(CoreListApprovalsResponse {
        approvals: vec![PendingApproval {
            approval_id: "appr_1".to_string(),
            app_id: "notes".to_string(),
            capability: "shell".to_string(),
            summary: "curl https://example.com".to_string(),
        }],
    });

    let payload = encode_control_message(&message).unwrap();
    let decoded = decode_control_message(&payload).unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn list_approvals_request_roundtrips() {
    let message = CoreControlMessage::ListPendingApprovals(CoreListApprovalsRequest);
    let payload = encode_control_message(&message).unwrap();
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(decoded, message);
}

#[test]
fn list_approvals_response_empty_roundtrips() {
    let message =
        CoreControlMessage::PendingApprovalsResult(CoreListApprovalsResponse { approvals: vec![] });
    let payload = encode_control_message(&message).unwrap();
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(decoded, message);
}

#[test]
fn list_approvals_response_multiple_roundtrips() {
    let message = CoreControlMessage::PendingApprovalsResult(CoreListApprovalsResponse {
        approvals: vec![
            PendingApproval {
                approval_id: "approval-1".to_string(),
                app_id: "notes".to_string(),
                capability: "shell".to_string(),
                summary: "rg pattern".to_string(),
            },
            PendingApproval {
                approval_id: "approval-2".to_string(),
                app_id: "todo".to_string(),
                capability: "shell".to_string(),
                summary: "curl https://api.example.com".to_string(),
            },
        ],
    });
    let payload = encode_control_message(&message).unwrap();
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(decoded, message);
}

#[test]
fn approve_request_roundtrips() {
    let message = CoreControlMessage::ApprovePendingApproval(CoreApproveApprovalRequest {
        approval_id: "approval-42".to_string(),
    });
    let payload = encode_control_message(&message).unwrap();
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(decoded, message);
}

#[test]
fn reject_request_roundtrips() {
    let message = CoreControlMessage::RejectPendingApproval(CoreRejectApprovalRequest {
        approval_id: "approval-99".to_string(),
    });
    let payload = encode_control_message(&message).unwrap();
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(decoded, message);
}

#[test]
fn approval_decision_response_roundtrips() {
    let message = CoreControlMessage::ApprovalDecisionResult(CoreApprovalDecisionResponse);
    let payload = encode_control_message(&message).unwrap();
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(decoded, message);
}

#[test]
fn approve_empty_approval_id_roundtrips() {
    let message = CoreControlMessage::ApprovePendingApproval(CoreApproveApprovalRequest {
        approval_id: "".to_string(),
    });
    let payload = encode_control_message(&message).unwrap();
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(decoded, message);
}

#[test]
fn pending_approval_fields_preserved_through_roundtrip() {
    let original = PendingApproval {
        approval_id: "appr-special-!@#$%".to_string(),
        app_id: "my-app_v2".to_string(),
        capability: "shell".to_string(),
        summary: "printf 'hello world' | wc -c".to_string(),
    };
    let message = CoreControlMessage::PendingApprovalsResult(CoreListApprovalsResponse {
        approvals: vec![original.clone()],
    });
    let payload = encode_control_message(&message).unwrap();
    let decoded = decode_control_message(&payload).unwrap();

    if let CoreControlMessage::PendingApprovalsResult(resp) = decoded {
        assert_eq!(resp.approvals.len(), 1);
        assert_eq!(resp.approvals[0], original);
    } else {
        panic!("expected PendingApprovalsResult");
    }
}

#[test]
fn approval_payload_has_correct_metadata() {
    let message = CoreControlMessage::ApprovePendingApproval(CoreApproveApprovalRequest {
        approval_id: "test".to_string(),
    });
    let payload = encode_control_message(&message).unwrap();
    assert_eq!(
        payload.content_type.as_deref(),
        Some(CORE_CONTROL_CONTENT_TYPE)
    );
    assert_eq!(payload.schema.as_deref(), Some(CORE_CONTROL_SCHEMA));
    assert!(payload.metadata.is_empty());
}
