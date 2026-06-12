use kunkka_ipc::{FrameMetadata, Payload};
use kunkka_worker_sdk::{
    decode_worker_message, encode_worker_message, AppId, DispatchWorkerRequest,
    DispatchWorkerResponse, RegisterWorkerRequest, RegisterWorkerResponse, WorkerAppError,
    WorkerCapability, WorkerId, WorkerProtocolMessage, WORKER_PROTOCOL_CONTENT_TYPE,
    WORKER_PROTOCOL_SCHEMA,
};

fn payload(bytes: &[u8]) -> Payload {
    Payload {
        bytes: bytes.to_vec(),
        content_type: Some("application/json".to_string()),
        schema: Some("example.notes.v1".to_string()),
        metadata: FrameMetadata::new(),
    }
}

fn sample_request() -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new("worker-1"),
        app_id: AppId::new("example-app"),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: Some("Search notes".to_string()),
        }],
    }
}

#[test]
fn register_worker_message_roundtrips_through_payload() {
    let message = WorkerProtocolMessage::RegisterWorker(sample_request());

    let payload = encode_worker_message(&message).unwrap();
    let decoded = decode_worker_message(&payload).unwrap();

    assert_eq!(
        payload.content_type.as_deref(),
        Some(WORKER_PROTOCOL_CONTENT_TYPE)
    );
    assert_eq!(payload.schema.as_deref(), Some(WORKER_PROTOCOL_SCHEMA));
    assert_eq!(decoded, message);
}

#[test]
fn register_worker_accepted_message_roundtrips_through_payload() {
    let response = RegisterWorkerResponse {
        worker_id: WorkerId::new("worker-1"),
        accepted: true,
        message: None,
    };

    let message = WorkerProtocolMessage::RegisterWorkerAccepted(response);

    let payload = encode_worker_message(&message).unwrap();
    let decoded = decode_worker_message(&payload).unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn dispatch_worker_request_roundtrips_through_payload() {
    let message = WorkerProtocolMessage::DispatchWorker(DispatchWorkerRequest {
        app_id: AppId::new("notes"),
        method: "search".to_string(),
        payload: payload(br#"{"query":"kunkka"}"#),
    });

    let encoded = encode_worker_message(&message).unwrap();
    let decoded = decode_worker_message(&encoded).unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn dispatch_worker_success_response_roundtrips_through_payload() {
    let message = WorkerProtocolMessage::DispatchWorkerResult(DispatchWorkerResponse::Ok(payload(
        br#"{"items":[]}"#,
    )));

    let encoded = encode_worker_message(&message).unwrap();
    let decoded = decode_worker_message(&encoded).unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn dispatch_worker_error_response_roundtrips_through_payload() {
    let message =
        WorkerProtocolMessage::DispatchWorkerResult(DispatchWorkerResponse::Err(WorkerAppError {
            code: "not_found".to_string(),
            message: "note missing".to_string(),
        }));

    let encoded = encode_worker_message(&message).unwrap();
    let decoded = decode_worker_message(&encoded).unwrap();

    assert_eq!(decoded, message);
}
