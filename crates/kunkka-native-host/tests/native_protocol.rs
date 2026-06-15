use kunkka_native_host::native_protocol::{
    decode_request, error_response, success_response, NativeCommand, NativeErrorCode,
    NativeRequest, NativeResult,
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
