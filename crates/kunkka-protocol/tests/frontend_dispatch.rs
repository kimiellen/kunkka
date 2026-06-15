use kunkka_ipc::{FrameMetadata, Payload};
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message, FrontendDispatchMessage,
    FrontendDispatchRequest, FrontendDispatchResponse, FRONTEND_DISPATCH_CONTENT_TYPE,
    FRONTEND_DISPATCH_SCHEMA,
};

fn json_payload(bytes: &[u8]) -> Payload {
    Payload {
        bytes: bytes.to_vec(),
        content_type: Some("application/json".to_string()),
        schema: None,
        metadata: FrameMetadata::new(),
    }
}

#[test]
fn dispatch_request_payload_sets_metadata_and_roundtrips() {
    let message = FrontendDispatchMessage::Dispatch(FrontendDispatchRequest {
        app_id: "notes".to_string(),
        method: "search".to_string(),
        payload: json_payload(br#"{"query":"kunkka"}"#),
    });

    let payload = encode_frontend_dispatch_message(&message).unwrap();

    assert_eq!(
        payload.content_type.as_deref(),
        Some(FRONTEND_DISPATCH_CONTENT_TYPE)
    );
    assert_eq!(payload.schema.as_deref(), Some(FRONTEND_DISPATCH_SCHEMA));
    assert_eq!(decode_frontend_dispatch_message(&payload).unwrap(), message);
}

#[test]
fn success_response_roundtrips() {
    let message = FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::Ok(
        json_payload(br#"{"items":[]}"#),
    ));

    let payload = encode_frontend_dispatch_message(&message).unwrap();

    assert_eq!(decode_frontend_dispatch_message(&payload).unwrap(), message);
}

#[test]
fn app_error_response_roundtrips() {
    let message = FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::AppError {
        code: "not_found".to_string(),
        message: "note not found".to_string(),
    });

    let payload = encode_frontend_dispatch_message(&message).unwrap();

    assert_eq!(decode_frontend_dispatch_message(&payload).unwrap(), message);
}

#[test]
fn platform_error_response_roundtrips() {
    let message =
        FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::PlatformError {
            code: "app_not_found".to_string(),
            message: "app not found: notes".to_string(),
        });

    let payload = encode_frontend_dispatch_message(&message).unwrap();

    assert_eq!(decode_frontend_dispatch_message(&payload).unwrap(), message);
}
