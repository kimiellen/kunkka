use crate::Result;
use kunkka_ipc::{FrameMetadata, Payload};
use serde::{Deserialize, Serialize};

pub const FRONTEND_DISPATCH_CONTENT_TYPE: &str =
    "application/vnd.kunkka.frontend-dispatch.v1+postcard";
pub const FRONTEND_DISPATCH_SCHEMA: &str = "kunkka.frontend-dispatch.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontendDispatchRequest {
    pub app_id: String,
    pub method: String,
    pub payload: Payload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrontendDispatchResponse {
    Ok(Payload),
    AppError { code: String, message: String },
    PlatformError { code: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrontendDispatchMessage {
    Dispatch(FrontendDispatchRequest),
    DispatchResult(FrontendDispatchResponse),
}

pub fn encode_frontend_dispatch_message(message: &FrontendDispatchMessage) -> Result<Payload> {
    let bytes = postcard::to_stdvec(message)?;

    Ok(Payload {
        bytes,
        content_type: Some(FRONTEND_DISPATCH_CONTENT_TYPE.to_string()),
        schema: Some(FRONTEND_DISPATCH_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_frontend_dispatch_message(payload: &Payload) -> Result<FrontendDispatchMessage> {
    Ok(postcard::from_bytes(&payload.bytes)?)
}
