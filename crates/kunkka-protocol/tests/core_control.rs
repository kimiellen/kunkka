use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingRequest,
    CoreStatusResponse, CORE_CONTROL_CONTENT_TYPE, CORE_CONTROL_SCHEMA,
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
