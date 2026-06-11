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
