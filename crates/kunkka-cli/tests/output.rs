use kunkka_cli::output::{CliOutput, CliResult};
use kunkka_protocol::core_control::PendingApproval;
use serde_json::json;

#[test]
fn pong_output_serializes_to_json() {
    let output = CliOutput {
        ok: true,
        result: Some(CliResult::Pong),
        error: None,
    };
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(value, json!({"ok":true,"result":{"type":"pong"}}));
}

#[test]
fn status_output_serializes_to_json() {
    let output = CliOutput {
        ok: true,
        result: Some(CliResult::Status {
            worker_count: 2,
            socket_path: "/tmp/core.sock".to_string(),
            runtime_ready: true,
        }),
        error: None,
    };
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(
        value,
        json!({"ok":true,"result":{"type":"status","worker_count":2,"socket_path":"/tmp/core.sock","runtime_ready":true}})
    );
}

#[test]
fn dispatch_output_serializes_to_json() {
    let output = CliOutput {
        ok: true,
        result: Some(CliResult::Dispatch {
            payload: json!({"items": []}),
        }),
        error: None,
    };
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(
        value,
        json!({"ok":true,"result":{"type":"dispatch","payload":{"items":[]}}})
    );
}

#[test]
fn dispatch_error_output_serializes_to_json() {
    let output = CliOutput {
        ok: true,
        result: Some(CliResult::DispatchError {
            code: "not_found".to_string(),
            message: "note not found".to_string(),
        }),
        error: None,
    };
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(
        value,
        json!({"ok":true,"result":{"type":"dispatch_error","code":"not_found","message":"note not found"}})
    );
}

#[test]
fn pending_approvals_output_serializes_to_json() {
    let output = CliOutput {
        ok: true,
        result: Some(CliResult::PendingApprovals {
            approvals: vec![PendingApproval {
                approval_id: "appr_1".to_string(),
                app_id: "notes".to_string(),
                capability: "shell".to_string(),
                summary: "printf approved".to_string(),
            }],
        }),
        error: None,
    };
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(
        value,
        json!({"ok":true,"result":{"type":"pending_approvals","approvals":[{"approval_id":"appr_1","app_id":"notes","capability":"shell","summary":"printf approved"}]}})
    );
}

#[test]
fn approval_decision_output_serializes_to_json() {
    let output = CliOutput {
        ok: true,
        result: Some(CliResult::ApprovalDecision),
        error: None,
    };
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(
        value,
        json!({"ok":true,"result":{"type":"approval_decision"}})
    );
}

#[test]
fn error_output_serializes_to_json() {
    let output: CliOutput = CliOutput::error(
        "permission_denied",
        "frontend dispatch method \"delete\" is not allowed",
    );
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(
        value,
        json!({"ok":false,"error":{"code":"permission_denied","message":"frontend dispatch method \"delete\" is not allowed"}})
    );
}
