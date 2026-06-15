use kunkka_native_host::bridge::{
    core_message_for_command, frontend_dispatch_request_for_command,
    native_result_for_core_response, native_result_for_frontend_dispatch_response,
};
use kunkka_native_host::native_protocol::{NativeCommand, NativeResult};
use kunkka_native_host::NativeHostError;
use kunkka_protocol::core_control::{
    CoreControlMessage, CorePingRequest, CorePingResponse, CoreStatusRequest, CoreStatusResponse,
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
