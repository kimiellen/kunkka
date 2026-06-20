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
pub struct CoreListApprovalsRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApproval {
    pub approval_id: String,
    pub app_id: String,
    pub capability: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreListApprovalsResponse {
    pub approvals: Vec<PendingApproval>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreApproveApprovalRequest {
    pub approval_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreRejectApprovalRequest {
    pub approval_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreApprovalDecisionResponse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeFlavor {
    Latte,
    Macchiato,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreGetThemeRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreGetThemeResponse {
    pub flavor: ThemeFlavor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreSetThemeRequest {
    pub flavor: ThemeFlavor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreSetThemeResponse;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeChangedEvent {
    pub flavor: ThemeFlavor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreControlMessage {
    Ping(CorePingRequest),
    Pong(CorePingResponse),
    Status(CoreStatusRequest),
    StatusResult(CoreStatusResponse),
    ListPendingApprovals(CoreListApprovalsRequest),
    PendingApprovalsResult(CoreListApprovalsResponse),
    ApprovePendingApproval(CoreApproveApprovalRequest),
    RejectPendingApproval(CoreRejectApprovalRequest),
    ApprovalDecisionResult(CoreApprovalDecisionResponse),
    GetTheme(CoreGetThemeRequest),
    GetThemeResult(CoreGetThemeResponse),
    SetTheme(CoreSetThemeRequest),
    SetThemeResult(CoreSetThemeResponse),
    ThemeChanged(ThemeChangedEvent),
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
