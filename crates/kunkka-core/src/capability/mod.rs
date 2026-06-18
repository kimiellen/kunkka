pub mod fs;
pub mod http;
pub mod permissions;
pub mod shell;
pub mod sqlite;

use crate::app_manifest::AppRegistry;
use crate::approval::ApprovalStore;
use kunkka_ipc::{FrameMetadata, Payload};
use serde::{Deserialize, Serialize};
use std::path::Path;

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
    app_registry: &AppRegistry,
    approvals: &mut ApprovalStore,
    request: CapabilityRequest,
    sqlite_connections: Option<&mut sqlite::SqliteConnectionStore>,
    data_dir: &Path,
) -> CapabilityResponse {
    let result = handle_capability_inner(
        app_registry,
        approvals,
        &request,
        sqlite_connections,
        data_dir,
    )
    .await;
    CapabilityResponse { result }
}

async fn handle_capability_inner(
    app_registry: &AppRegistry,
    approvals: &mut ApprovalStore,
    request: &CapabilityRequest,
    sqlite_connections: Option<&mut sqlite::SqliteConnectionStore>,
    data_dir: &Path,
) -> Result<Vec<u8>, CapabilityError> {
    if request.app_id.is_empty() {
        return Err(CapabilityError {
            code: "invalid_request".to_string(),
            message: "capability request app_id is empty".to_string(),
        });
    }

    let manifest = app_registry
        .get(&request.app_id)
        .ok_or_else(|| CapabilityError {
            code: "app_not_found".to_string(),
            message: format!("app not found: {}", request.app_id),
        })?;

    match request.capability.as_str() {
        "fs" => fs::handle_fs_request(manifest, &request.method, &request.params).await,
        "http" => http::handle_http_request(manifest, &request.method, &request.params).await,
        "shell" => {
            shell::handle_shell_request(manifest, &request.method, &request.params, approvals).await
        }
        "sqlite" => {
            let store = sqlite_connections.ok_or_else(|| CapabilityError {
                code: "unavailable".to_string(),
                message: "sqlite connection store not available".to_string(),
            })?;
            sqlite::handle_sqlite_request(
                manifest,
                &request.method,
                &request.params,
                store,
                data_dir,
            )
            .await
        }
        _ => Err(CapabilityError {
            code: "unknown_capability".to_string(),
            message: format!("unknown capability: {}", request.capability),
        }),
    }
}
