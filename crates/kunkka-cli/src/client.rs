use crate::cli::{ApprovalCommand, CliCommand};
use crate::error::CliError;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreApproveApprovalRequest, CoreControlMessage,
    CoreListApprovalsRequest, CorePingRequest, CoreRejectApprovalRequest, CoreStatusRequest,
};
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message, FrontendDispatchMessage,
    FrontendDispatchRequest, FrontendDispatchResponse,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

const JSON_CONTENT_TYPE: &str = "application/json";
const CAPABILITY_CONTENT_TYPE: &str = "application/vnd.kunkka.capability.v1+postcard";
const CAPABILITY_SCHEMA: &str = "kunkka.capability.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellRunRequest {
    pub app_id: String,
    pub capability: String,
    pub method: String,
    pub params: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellRunParams {
    pub command: String,
    pub approval_id: Option<String>,
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
    let request = ShellRunRequest {
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
