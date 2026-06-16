pub mod fs;
pub mod permissions;

use kunkka_ipc::{FrameMetadata, Payload};
use serde::{Deserialize, Serialize};

pub const CAPABILITY_CONTENT_TYPE: &str = "application/vnd.kunkka.capability.v1+postcard";
pub const CAPABILITY_SCHEMA: &str = "kunkka.capability.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub app_id: String,
    pub capability: String,
    pub method: String,
    pub params: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityResponse {
    pub result: Result<Vec<u8>, CapabilityError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityError {
    pub code: String,
    pub message: String,
}

pub fn encode_capability_request(request: &CapabilityRequest) -> crate::Result<Payload> {
    let bytes = postcard::to_stdvec(request)
        .map_err(|e| crate::CoreError::InvalidCoreFrame(format!("capability encode: {e}")))?;
    Ok(Payload {
        bytes,
        content_type: Some(CAPABILITY_CONTENT_TYPE.to_string()),
        schema: Some(CAPABILITY_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_capability_request(payload: &Payload) -> crate::Result<CapabilityRequest> {
    postcard::from_bytes(&payload.bytes)
        .map_err(|e| crate::CoreError::InvalidCoreFrame(format!("capability decode: {e}")))
}

pub fn encode_capability_response(response: &CapabilityResponse) -> crate::Result<Payload> {
    let bytes = postcard::to_stdvec(response)
        .map_err(|e| crate::CoreError::InvalidCoreFrame(format!("capability encode: {e}")))?;
    Ok(Payload {
        bytes,
        content_type: Some(CAPABILITY_CONTENT_TYPE.to_string()),
        schema: Some(CAPABILITY_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_capability_response(payload: &Payload) -> crate::Result<CapabilityResponse> {
    postcard::from_bytes(&payload.bytes)
        .map_err(|e| crate::CoreError::InvalidCoreFrame(format!("capability decode: {e}")))
}

pub async fn handle_capability_request(
    worker_manager: &crate::worker_dispatch::WorkerManager,
    request: CapabilityRequest,
) -> CapabilityResponse {
    let result = handle_capability_inner(worker_manager, &request).await;
    CapabilityResponse { result }
}

async fn handle_capability_inner(
    worker_manager: &crate::worker_dispatch::WorkerManager,
    request: &CapabilityRequest,
) -> Result<Vec<u8>, CapabilityError> {
    if request.app_id.is_empty() {
        return Err(CapabilityError {
            code: "invalid_request".to_string(),
            message: "capability request app_id is empty".to_string(),
        });
    }

    let manifest = worker_manager
        .app_registry()
        .get(&request.app_id)
        .ok_or_else(|| CapabilityError {
            code: "app_not_found".to_string(),
            message: format!("app not found: {}", request.app_id),
        })?;

    match request.capability.as_str() {
        "fs" => fs::handle_fs_request(manifest, &request.method, &request.params).await,
        _ => Err(CapabilityError {
            code: "unknown_capability".to_string(),
            message: format!("unknown capability: {}", request.capability),
        }),
    }
}
