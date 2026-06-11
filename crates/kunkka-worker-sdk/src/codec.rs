use crate::{Result, WorkerProtocolMessage};
use kunkka_ipc::{FrameMetadata, Payload};

pub const WORKER_PROTOCOL_CONTENT_TYPE: &str = "application/vnd.kunkka.worker.v1+postcard";
pub const WORKER_PROTOCOL_SCHEMA: &str = "kunkka.worker.v1";

pub fn encode_worker_message(message: &WorkerProtocolMessage) -> Result<Payload> {
    let bytes = postcard::to_stdvec(message)?;

    Ok(Payload {
        bytes,
        content_type: Some(WORKER_PROTOCOL_CONTENT_TYPE.to_string()),
        schema: Some(WORKER_PROTOCOL_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_worker_message(payload: &Payload) -> Result<WorkerProtocolMessage> {
    Ok(postcard::from_bytes(&payload.bytes)?)
}
