use kunkka_native_host::bridge::{core_message_for_command, native_result_for_core_response};
use kunkka_native_host::native_protocol::{NativeCommand, NativeResult};
use kunkka_native_host::NativeHostError;
use kunkka_protocol::core_control::{
    CoreControlMessage, CorePingRequest, CorePingResponse, CoreStatusResponse,
};

#[test]
fn maps_ping_command_to_core_ping() {
    let message = core_message_for_command(&NativeCommand::Ping);

    assert_eq!(message, CoreControlMessage::Ping(CorePingRequest));
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
fn maps_pong_to_native_pong() {
    let result = native_result_for_core_response(
        &NativeCommand::Ping,
        CoreControlMessage::Pong(CorePingResponse),
    )
    .unwrap();

    assert_eq!(result, NativeResult::Pong);
}
