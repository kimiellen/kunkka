use crate::Result;
use kunkka_ipc::{FrameMetadata, Payload};
use serde::{Deserialize, Serialize};

pub const CORE_CONTROL_CONTENT_TYPE: &str = "application/vnd.kunkka.core-control.v1+postcard";
pub const CORE_CONTROL_SCHEMA: &str = "kunkka.core-control.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorePingRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorePingResponse;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreStatusRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreStatusResponse {
    pub worker_count: u64,
    pub socket_path: String,
    pub runtime_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreControlMessage {
    Ping(CorePingRequest),
    Pong(CorePingResponse),
    Status(CoreStatusRequest),
    StatusResult(CoreStatusResponse),
}

pub fn encode_control_message(message: &CoreControlMessage) -> Result<Payload> {
    let bytes = postcard::to_stdvec(message)?;

    Ok(Payload {
        bytes,
        content_type: Some(CORE_CONTROL_CONTENT_TYPE.to_string()),
        schema: Some(CORE_CONTROL_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_control_message(payload: &Payload) -> Result<CoreControlMessage> {
    Ok(postcard::from_bytes(&payload.bytes)?)
}
