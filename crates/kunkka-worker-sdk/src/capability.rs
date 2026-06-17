use crate::{AppId, Result, WorkerSdkError};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CAPABILITY_CONTENT_TYPE: &str = "application/vnd.kunkka.capability.v1+postcard";
pub const CAPABILITY_SCHEMA: &str = "kunkka.capability.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub app_id: String,
    pub capability: String,
    pub method: String,
    pub params: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityResponse {
    pub result: std::result::Result<Vec<u8>, CapabilityError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityError {
    pub code: String,
    pub message: String,
}

pub fn encode_capability_request(request: &CapabilityRequest) -> Result<Payload> {
    let bytes = postcard::to_stdvec(request)?;
    Ok(Payload {
        bytes,
        content_type: Some(CAPABILITY_CONTENT_TYPE.to_string()),
        schema: Some(CAPABILITY_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_capability_request(payload: &Payload) -> Result<CapabilityRequest> {
    Ok(postcard::from_bytes(&payload.bytes)?)
}

pub fn encode_capability_response(response: &CapabilityResponse) -> Result<Payload> {
    let bytes = postcard::to_stdvec(response)?;
    Ok(Payload {
        bytes,
        content_type: Some(CAPABILITY_CONTENT_TYPE.to_string()),
        schema: Some(CAPABILITY_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_capability_response(payload: &Payload) -> Result<CapabilityResponse> {
    Ok(postcard::from_bytes(&payload.bytes)?)
}

pub async fn call_capability(
    socket_path: impl AsRef<Path>,
    app_id: &AppId,
    capability: &str,
    method: &str,
    params: Vec<u8>,
) -> Result<CapabilityResponse> {
    let request_id = RequestId(1);
    let session_id = SessionId(1);
    let payload = encode_capability_request(&CapabilityRequest {
        app_id: app_id.as_str().to_string(),
        capability: capability.to_string(),
        method: method.to_string(),
        params,
    })?;

    let frame = Frame::Request {
        request_id,
        session_id,
        source: EndpointId::new("worker-sdk"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    let mut connection = IpcConnection::connect(socket_path).await?;
    connection.send_frame(&frame).await?;

    let response = connection
        .recv_frame()
        .await?
        .ok_or(kunkka_ipc::IpcError::ConnectionClosed)?;

    let Frame::Response {
        request_id: response_request_id,
        payload,
        ..
    } = response
    else {
        return Err(WorkerSdkError::Protocol(
            "expected capability response frame".to_string(),
        ));
    };

    if response_request_id != request_id {
        return Err(WorkerSdkError::Protocol(format!(
            "response request_id mismatch: expected {}, got {}",
            request_id.0, response_request_id.0
        )));
    }

    decode_capability_response(&payload)
}
