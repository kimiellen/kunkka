pub mod fs;
pub mod http;
pub mod llm;
pub mod permissions;
pub mod shell;
pub mod sqlite;

use crate::app_manifest::AppRegistry;
use crate::approval::ApprovalStore;
use kunkka_ipc::Payload;
use std::path::Path;

pub use kunkka_worker_sdk::capability::{
    CapabilityError, CapabilityRequest, CapabilityResponse, CAPABILITY_CONTENT_TYPE,
    CAPABILITY_SCHEMA,
};

pub fn encode_capability_request(request: &CapabilityRequest) -> crate::Result<Payload> {
    kunkka_worker_sdk::capability::encode_capability_request(request)
        .map_err(|e| crate::CoreError::InvalidCoreFrame(format!("capability encode: {e}")))
}

pub fn decode_capability_request(payload: &Payload) -> crate::Result<CapabilityRequest> {
    kunkka_worker_sdk::capability::decode_capability_request(payload)
        .map_err(|e| crate::CoreError::InvalidCoreFrame(format!("capability decode: {e}")))
}

pub fn encode_capability_response(response: &CapabilityResponse) -> crate::Result<Payload> {
    kunkka_worker_sdk::capability::encode_capability_response(response)
        .map_err(|e| crate::CoreError::InvalidCoreFrame(format!("capability encode: {e}")))
}

pub fn decode_capability_response(payload: &Payload) -> crate::Result<CapabilityResponse> {
    kunkka_worker_sdk::capability::decode_capability_response(payload)
        .map_err(|e| crate::CoreError::InvalidCoreFrame(format!("capability decode: {e}")))
}

pub async fn handle_capability_request(
    app_registry: &AppRegistry,
    approvals: &mut ApprovalStore,
    request: CapabilityRequest,
    sqlite_connections: Option<&mut sqlite::SqliteConnectionStore>,
    data_dir: &Path,
    llm_state: Option<&llm::LlmState>,
) -> CapabilityResponse {
    let result = handle_capability_inner(
        app_registry,
        approvals,
        &request,
        sqlite_connections,
        data_dir,
        llm_state,
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
    llm_state: Option<&llm::LlmState>,
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
        "llm" => {
            let state = llm_state.ok_or_else(|| CapabilityError {
                code: "unavailable".to_string(),
                message: "LLM state not available".to_string(),
            })?;
            llm::handle_llm_request(manifest, &request.method, &request.params, state).await
        }
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
