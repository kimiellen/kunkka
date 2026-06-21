use crate::cli::{ApprovalCommand, CliCommand};
use crate::error::CliError;
use crate::output::LlmProviderTestResult;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreApproveApprovalRequest, CoreControlMessage,
    CoreListApprovalsRequest, CorePingRequest, CoreRejectApprovalRequest, CoreStatusRequest,
};
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message, FrontendDispatchMessage,
    FrontendDispatchRequest, FrontendDispatchResponse,
};
use kunkka_worker_sdk::capability::{
    CapabilityRequest, CapabilityResponse, CAPABILITY_CONTENT_TYPE, CAPABILITY_SCHEMA,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

const JSON_CONTENT_TYPE: &str = "application/json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellRunParams {
    pub command: String,
    pub approval_id: Option<String>,
}

// LLM types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPresetInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderInfo {
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub available_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRoleInfo {
    pub name: String,
    pub description: String,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModelInfo {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAddProviderRequest {
    pub name: String,
    pub config: LlmProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub provider_type: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub available_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRemoveProviderRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAddRoleRequest {
    pub name: String,
    pub config: LlmRoleConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRoleConfig {
    pub description: String,
    pub provider: String,
    pub model: String,
    pub parameters: LlmModelParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModelParameters {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRemoveRoleRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmApplyPresetRequest {
    pub preset_name: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmApplyRolePresetRequest {
    pub preset_name: String,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTestProviderRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUpdateProviderRequest {
    pub name: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub available_models: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUpdateRoleRequest {
    pub name: String,
    pub description: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsageRecordsRequest {
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSetDefaultRoleRequest {
    pub role_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsageSummaryResponse {
    pub total_requests: u64,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsageRecordResponse {
    pub timestamp: u64,
    pub provider: String,
    pub model: String,
    pub role: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmDefaultRoleResponse {
    pub role_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellRunOutcome {
    Completed(ShellRunResult),
    PendingApproval(PendingApprovalReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellRunResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApprovalReceipt {
    pub approval_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApprovalItem {
    pub approval_id: String,
    pub app_id: String,
    pub capability: String,
    pub summary: String,
}

pub fn core_message_for_command(command: &CliCommand) -> Option<CoreControlMessage> {
    match command {
        CliCommand::Ping => Some(CoreControlMessage::Ping(CorePingRequest)),
        CliCommand::Status => Some(CoreControlMessage::Status(CoreStatusRequest)),
        CliCommand::Approvals { command } => Some(match command {
            ApprovalCommand::List => {
                CoreControlMessage::ListPendingApprovals(CoreListApprovalsRequest)
            }
            ApprovalCommand::Approve { approval_id } => {
                CoreControlMessage::ApprovePendingApproval(CoreApproveApprovalRequest {
                    approval_id: approval_id.clone(),
                })
            }
            ApprovalCommand::Reject { approval_id } => {
                CoreControlMessage::RejectPendingApproval(CoreRejectApprovalRequest {
                    approval_id: approval_id.clone(),
                })
            }
        }),
        CliCommand::Shell { .. } => None,
        CliCommand::Dispatch { .. } => None,
        CliCommand::Llm { .. } => None,
    }
}

pub async fn send_core_control(
    socket_path: &Path,
    message: CoreControlMessage,
) -> Result<CoreControlMessage, CliError> {
    let mut connection = IpcConnection::connect(socket_path)
        .await
        .map_err(|err| CliError::CoreUnavailable(err.to_string()))?;

    let request_id = RequestId(1);
    let session_id = SessionId(1);
    let payload =
        encode_control_message(&message).map_err(|err| CliError::CoreIpc(err.to_string()))?;
    let frame = Frame::Request {
        request_id,
        session_id,
        source: EndpointId::new("cli"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    connection
        .send_frame(&frame)
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?;

    let response = connection
        .recv_frame()
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?
        .ok_or_else(|| CliError::CoreIpc("core closed connection".to_string()))?;

    let Frame::Response {
        request_id: response_request_id,
        payload,
        ..
    } = response
    else {
        return Err(CliError::UnexpectedCoreResponse(
            "expected response frame".to_string(),
        ));
    };

    if response_request_id != request_id {
        return Err(CliError::UnexpectedCoreResponse(format!(
            "response request_id mismatch: expected {}, got {}",
            request_id.0, response_request_id.0
        )));
    }

    decode_control_message(&payload).map_err(|err| CliError::CoreIpc(err.to_string()))
}

pub async fn send_frontend_dispatch(
    socket_path: &Path,
    request: FrontendDispatchRequest,
) -> Result<FrontendDispatchResponse, CliError> {
    let mut connection = IpcConnection::connect(socket_path)
        .await
        .map_err(|err| CliError::CoreUnavailable(err.to_string()))?;

    let request_id = RequestId(1);
    let session_id = SessionId(1);
    let payload = encode_frontend_dispatch_message(&FrontendDispatchMessage::Dispatch(request))
        .map_err(|err| CliError::CoreIpc(err.to_string()))?;
    let frame = Frame::Request {
        request_id,
        session_id,
        source: EndpointId::new("cli"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    connection
        .send_frame(&frame)
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?;

    let response = connection
        .recv_frame()
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?
        .ok_or_else(|| CliError::CoreIpc("core closed connection".to_string()))?;

    let Frame::Response {
        request_id: response_request_id,
        payload,
        ..
    } = response
    else {
        return Err(CliError::UnexpectedCoreResponse(
            "expected response frame".to_string(),
        ));
    };

    if response_request_id != request_id {
        return Err(CliError::UnexpectedCoreResponse(format!(
            "response request_id mismatch: expected {}, got {}",
            request_id.0, response_request_id.0
        )));
    }

    match decode_frontend_dispatch_message(&payload)
        .map_err(|err| CliError::CoreIpc(err.to_string()))?
    {
        FrontendDispatchMessage::DispatchResult(response) => Ok(response),
        _ => Err(CliError::UnexpectedCoreResponse(
            "expected frontend dispatch result".to_string(),
        )),
    }
}

pub fn build_frontend_dispatch_request(
    app_id: String,
    method: String,
    payload: serde_json::Value,
) -> FrontendDispatchRequest {
    FrontendDispatchRequest {
        app_id,
        method,
        payload: Payload {
            bytes: serde_json::to_vec(&payload).unwrap_or_default(),
            content_type: Some(JSON_CONTENT_TYPE.to_string()),
            schema: None,
            metadata: FrameMetadata::new(),
        },
    }
}

pub async fn send_shell_request(
    socket_path: &Path,
    app_id: String,
    command: String,
    approval_id: Option<String>,
) -> Result<ShellRunOutcome, CliError> {
    let params = postcard::to_stdvec(&ShellRunParams {
        command,
        approval_id,
    })
    .map_err(|err| CliError::CoreIpc(format!("failed to encode shell params: {err}")))?;
    let request = CapabilityRequest {
        app_id,
        capability: "shell".to_string(),
        method: "run".to_string(),
        params,
    };
    let payload = postcard::to_stdvec(&request)
        .map_err(|err| CliError::CoreIpc(format!("failed to encode capability request: {err}")))?;

    let mut connection = IpcConnection::connect(socket_path)
        .await
        .map_err(|err| CliError::CoreUnavailable(err.to_string()))?;

    let request_id = RequestId(1);
    let session_id = SessionId(1);
    let frame = Frame::Request {
        request_id,
        session_id,
        source: EndpointId::new("cli"),
        target: EndpointId::new("core"),
        payload: Payload {
            bytes: payload,
            content_type: Some(CAPABILITY_CONTENT_TYPE.to_string()),
            schema: Some(CAPABILITY_SCHEMA.to_string()),
            metadata: FrameMetadata::new(),
        },
        metadata: FrameMetadata::new(),
    };

    connection
        .send_frame(&frame)
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?;

    let response = connection
        .recv_frame()
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?
        .ok_or_else(|| CliError::CoreIpc("core closed connection".to_string()))?;

    let Frame::Response {
        request_id: response_request_id,
        payload,
        ..
    } = response
    else {
        return Err(CliError::UnexpectedCoreResponse(
            "expected response frame".to_string(),
        ));
    };

    if response_request_id != request_id {
        return Err(CliError::UnexpectedCoreResponse(format!(
            "response request_id mismatch: expected {}, got {}",
            request_id.0, response_request_id.0
        )));
    }

    let response = postcard::from_bytes::<CapabilityResponse>(&payload.bytes)
        .map_err(|err| CliError::CoreIpc(format!("failed to decode capability response: {err}")))?;

    match response.result {
        Ok(bytes) => postcard::from_bytes(&bytes)
            .map_err(|err| CliError::CoreIpc(format!("failed to decode shell outcome: {err}"))),
        Err(error) => Err(CliError::CorePlatform {
            code: error.code,
            message: error.message,
        }),
    }
}

pub async fn list_pending_approvals(
    socket_path: &Path,
) -> Result<Vec<PendingApprovalItem>, CliError> {
    match send_core_control(
        socket_path,
        CoreControlMessage::ListPendingApprovals(CoreListApprovalsRequest),
    )
    .await?
    {
        CoreControlMessage::PendingApprovalsResult(result) => Ok(result
            .approvals
            .into_iter()
            .map(|approval| PendingApprovalItem {
                approval_id: approval.approval_id,
                app_id: approval.app_id,
                capability: approval.capability,
                summary: approval.summary,
            })
            .collect()),
        other => Err(CliError::UnexpectedCoreResponse(format!(
            "expected pending approvals result, got {other:?}"
        ))),
    }
}

pub async fn approve_pending_approval(
    socket_path: &Path,
    approval_id: String,
) -> Result<(), CliError> {
    match send_core_control(
        socket_path,
        CoreControlMessage::ApprovePendingApproval(CoreApproveApprovalRequest { approval_id }),
    )
    .await?
    {
        CoreControlMessage::ApprovalDecisionResult(_) => Ok(()),
        other => Err(CliError::UnexpectedCoreResponse(format!(
            "expected approval decision result, got {other:?}"
        ))),
    }
}

pub async fn reject_pending_approval(
    socket_path: &Path,
    approval_id: String,
) -> Result<(), CliError> {
    match send_core_control(
        socket_path,
        CoreControlMessage::RejectPendingApproval(CoreRejectApprovalRequest { approval_id }),
    )
    .await?
    {
        CoreControlMessage::ApprovalDecisionResult(_) => Ok(()),
        other => Err(CliError::UnexpectedCoreResponse(format!(
            "expected approval decision result, got {other:?}"
        ))),
    }
}

// LLM capability helpers

/// Send a generic LLM capability request and return the raw response bytes
async fn send_llm_request(
    socket_path: &Path,
    method: &str,
    params: Vec<u8>,
) -> Result<Vec<u8>, CliError> {
    let request = CapabilityRequest {
        app_id: "kunkka-cli".to_string(),
        capability: "llm".to_string(),
        method: method.to_string(),
        params,
    };
    let payload = postcard::to_stdvec(&request)
        .map_err(|err| CliError::CoreIpc(format!("failed to encode LLM request: {err}")))?;

    let mut connection = IpcConnection::connect(socket_path)
        .await
        .map_err(|err| CliError::CoreUnavailable(err.to_string()))?;

    let request_id = RequestId(1);
    let session_id = SessionId(1);
    let frame = Frame::Request {
        request_id,
        session_id,
        source: EndpointId::new("cli"),
        target: EndpointId::new("core"),
        payload: Payload {
            bytes: payload,
            content_type: Some(CAPABILITY_CONTENT_TYPE.to_string()),
            schema: Some(CAPABILITY_SCHEMA.to_string()),
            metadata: FrameMetadata::new(),
        },
        metadata: FrameMetadata::new(),
    };

    connection
        .send_frame(&frame)
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?;

    let response = connection
        .recv_frame()
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?
        .ok_or_else(|| CliError::CoreIpc("core closed connection".to_string()))?;

    let Frame::Response {
        request_id: response_request_id,
        payload,
        ..
    } = response
    else {
        return Err(CliError::UnexpectedCoreResponse(
            "expected response frame".to_string(),
        ));
    };

    if response_request_id != request_id {
        return Err(CliError::UnexpectedCoreResponse(format!(
            "response request_id mismatch: expected {}, got {}",
            request_id.0, response_request_id.0
        )));
    }

    let response = postcard::from_bytes::<CapabilityResponse>(&payload.bytes)
        .map_err(|err| CliError::CoreIpc(format!("failed to decode capability response: {err}")))?;

    response.result.map_err(|error| CliError::CorePlatform {
        code: error.code,
        message: error.message,
    })
}

/// LLM response type for parsing generic responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmResponse {
    Presets(Vec<LlmPresetInfo>),
    Providers(Vec<LlmProviderInfo>),
    Roles(Vec<LlmRoleInfo>),
    ProviderDetails(Vec<LlmProviderInfo>),
    RoleDetails(Vec<LlmRoleInfo>),
    Models(Vec<LlmModelInfo>),
    ProviderTestResult(LlmProviderTestResult),
    UsageSummary(LlmUsageSummaryResponse),
    UsageRecords(Vec<LlmUsageRecordResponse>),
    DefaultRole(Option<String>),
    Success,
}

pub async fn llm_list_presets(socket_path: &Path) -> Result<Vec<LlmPresetInfo>, CliError> {
    let bytes = send_llm_request(socket_path, "list_presets", Vec::new()).await?;
    let response: LlmResponse = postcard::from_bytes(&bytes)
        .map_err(|err| CliError::CoreIpc(format!("failed to decode presets: {err}")))?;
    match response {
        LlmResponse::Presets(presets) => Ok(presets),
        other => Err(CliError::UnexpectedCoreResponse(format!(
            "expected presets response, got {other:?}"
        ))),
    }
}

pub async fn llm_apply_preset(
    socket_path: &Path,
    preset_name: String,
    api_key: String,
) -> Result<(), CliError> {
    let params = postcard::to_stdvec(&LlmApplyPresetRequest {
        preset_name,
        api_key,
    })
    .map_err(|err| CliError::CoreIpc(format!("failed to encode apply preset params: {err}")))?;
    send_llm_request(socket_path, "apply_preset", params).await?;
    Ok(())
}

pub async fn llm_list_providers(socket_path: &Path) -> Result<Vec<LlmProviderInfo>, CliError> {
    let bytes = send_llm_request(socket_path, "list_providers_detail", Vec::new()).await?;
    let response: LlmResponse = postcard::from_bytes(&bytes)
        .map_err(|err| CliError::CoreIpc(format!("failed to decode providers: {err}")))?;
    match response {
        LlmResponse::Providers(providers) => Ok(providers),
        other => Err(CliError::UnexpectedCoreResponse(format!(
            "expected providers response, got {other:?}"
        ))),
    }
}

pub async fn llm_add_provider(
    socket_path: &Path,
    name: String,
    base_url: String,
    api_key: String,
    models: Vec<String>,
) -> Result<(), CliError> {
    let params = postcard::to_stdvec(&LlmAddProviderRequest {
        name,
        config: LlmProviderConfig {
            provider_type: "api_key".to_string(),
            base_url,
            api_key: Some(api_key),
            available_models: models,
        },
    })
    .map_err(|err| CliError::CoreIpc(format!("failed to encode add provider params: {err}")))?;
    send_llm_request(socket_path, "add_provider", params).await?;
    Ok(())
}

pub async fn llm_remove_provider(socket_path: &Path, name: String) -> Result<(), CliError> {
    let params = postcard::to_stdvec(&LlmRemoveProviderRequest { name }).map_err(|err| {
        CliError::CoreIpc(format!("failed to encode remove provider params: {err}"))
    })?;
    send_llm_request(socket_path, "remove_provider", params).await?;
    Ok(())
}

pub async fn llm_list_roles(socket_path: &Path) -> Result<Vec<LlmRoleInfo>, CliError> {
    let bytes = send_llm_request(socket_path, "list_roles_detail", Vec::new()).await?;
    let response: LlmResponse = postcard::from_bytes(&bytes)
        .map_err(|err| CliError::CoreIpc(format!("failed to decode roles: {err}")))?;
    match response {
        LlmResponse::Roles(roles) => Ok(roles),
        LlmResponse::RoleDetails(roles) => Ok(roles),
        other => Err(CliError::UnexpectedCoreResponse(format!(
            "expected roles response, got {other:?}"
        ))),
    }
}

pub async fn llm_add_role(
    socket_path: &Path,
    name: String,
    description: String,
    provider: String,
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<(), CliError> {
    let params = postcard::to_stdvec(&LlmAddRoleRequest {
        name,
        config: LlmRoleConfig {
            description,
            provider,
            model,
            parameters: LlmModelParameters {
                temperature,
                max_tokens,
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
            },
        },
    })
    .map_err(|err| CliError::CoreIpc(format!("failed to encode add role params: {err}")))?;
    send_llm_request(socket_path, "add_role", params).await?;
    Ok(())
}

pub async fn llm_remove_role(socket_path: &Path, name: String) -> Result<(), CliError> {
    let params = postcard::to_stdvec(&LlmRemoveRoleRequest { name })
        .map_err(|err| CliError::CoreIpc(format!("failed to encode remove role params: {err}")))?;
    send_llm_request(socket_path, "remove_role", params).await?;
    Ok(())
}

pub async fn llm_update_provider(
    socket_path: &Path,
    name: String,
    api_key: Option<String>,
    base_url: Option<String>,
    models: Option<Vec<String>>,
) -> Result<(), CliError> {
    let params = postcard::to_stdvec(&LlmUpdateProviderRequest {
        name,
        api_key,
        base_url,
        available_models: models,
    })
    .map_err(|err| CliError::CoreIpc(format!("failed to encode update provider params: {err}")))?;
    send_llm_request(socket_path, "update_provider", params).await?;
    Ok(())
}

pub async fn llm_update_role(
    socket_path: &Path,
    name: String,
    description: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<(), CliError> {
    let params = postcard::to_stdvec(&LlmUpdateRoleRequest {
        name,
        description,
        provider,
        model,
        temperature,
        max_tokens,
    })
    .map_err(|err| CliError::CoreIpc(format!("failed to encode update role params: {err}")))?;
    send_llm_request(socket_path, "update_role", params).await?;
    Ok(())
}

pub async fn llm_test_provider(
    socket_path: &Path,
    name: String,
) -> Result<LlmProviderTestResult, CliError> {
    let params = postcard::to_stdvec(&LlmTestProviderRequest { name }).map_err(|err| {
        CliError::CoreIpc(format!("failed to encode test provider params: {err}"))
    })?;
    let bytes = send_llm_request(socket_path, "test_provider", params).await?;
    let response: LlmResponse = postcard::from_bytes(&bytes)
        .map_err(|err| CliError::CoreIpc(format!("failed to decode test result: {err}")))?;
    match response {
        LlmResponse::ProviderTestResult(result) => Ok(result),
        other => Err(CliError::UnexpectedCoreResponse(format!(
            "expected provider test result, got {other:?}"
        ))),
    }
}

pub async fn llm_list_role_presets(socket_path: &Path) -> Result<Vec<LlmPresetInfo>, CliError> {
    let bytes = send_llm_request(socket_path, "list_role_presets", Vec::new()).await?;
    let response: LlmResponse = postcard::from_bytes(&bytes)
        .map_err(|err| CliError::CoreIpc(format!("failed to decode role presets: {err}")))?;
    match response {
        LlmResponse::Presets(presets) => Ok(presets),
        other => Err(CliError::UnexpectedCoreResponse(format!(
            "expected presets response, got {other:?}"
        ))),
    }
}

pub async fn llm_apply_role_preset(
    socket_path: &Path,
    preset_name: String,
    provider: String,
    model: String,
) -> Result<(), CliError> {
    let params = postcard::to_stdvec(&LlmApplyRolePresetRequest {
        preset_name,
        provider,
        model,
    })
    .map_err(|err| {
        CliError::CoreIpc(format!("failed to encode apply role preset params: {err}"))
    })?;
    send_llm_request(socket_path, "apply_role_preset", params).await?;
    Ok(())
}

pub async fn llm_usage_summary(socket_path: &Path) -> Result<LlmUsageSummaryResponse, CliError> {
    let bytes = send_llm_request(socket_path, "usage_summary", Vec::new()).await?;
    let response: LlmResponse = postcard::from_bytes(&bytes)
        .map_err(|err| CliError::CoreIpc(format!("failed to decode usage summary: {err}")))?;
    match response {
        LlmResponse::UsageSummary(summary) => Ok(summary),
        other => Err(CliError::UnexpectedCoreResponse(format!(
            "expected usage summary response, got {other:?}"
        ))),
    }
}

pub async fn llm_usage_records(
    socket_path: &Path,
    limit: usize,
) -> Result<Vec<LlmUsageRecordResponse>, CliError> {
    let params = postcard::to_stdvec(&LlmUsageRecordsRequest { limit }).map_err(|err| {
        CliError::CoreIpc(format!("failed to encode usage records params: {err}"))
    })?;
    let bytes = send_llm_request(socket_path, "usage_records", params).await?;
    let response: LlmResponse = postcard::from_bytes(&bytes)
        .map_err(|err| CliError::CoreIpc(format!("failed to decode usage records: {err}")))?;
    match response {
        LlmResponse::UsageRecords(records) => Ok(records),
        other => Err(CliError::UnexpectedCoreResponse(format!(
            "expected usage records response, got {other:?}"
        ))),
    }
}

pub async fn llm_clear_usage(socket_path: &Path) -> Result<(), CliError> {
    send_llm_request(socket_path, "clear_usage", Vec::new()).await?;
    Ok(())
}

pub async fn llm_set_default_role(
    socket_path: &Path,
    role_name: Option<String>,
) -> Result<(), CliError> {
    let params = postcard::to_stdvec(&LlmSetDefaultRoleRequest { role_name }).map_err(|err| {
        CliError::CoreIpc(format!("failed to encode set default role params: {err}"))
    })?;
    send_llm_request(socket_path, "set_default_role", params).await?;
    Ok(())
}

pub async fn llm_get_default_role(socket_path: &Path) -> Result<Option<String>, CliError> {
    let bytes = send_llm_request(socket_path, "get_default_role", Vec::new()).await?;
    let response: LlmResponse = postcard::from_bytes(&bytes)
        .map_err(|err| CliError::CoreIpc(format!("failed to decode default role: {err}")))?;
    match response {
        LlmResponse::DefaultRole(role_name) => Ok(role_name),
        other => Err(CliError::UnexpectedCoreResponse(format!(
            "expected default role response, got {other:?}"
        ))),
    }
}
