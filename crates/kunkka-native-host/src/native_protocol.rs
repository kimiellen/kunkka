use crate::{NativeHostError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NativeRequest {
    pub id: String,
    #[serde(flatten)]
    pub command: NativeCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum NativeCommand {
    Ping,
    Status,
    Dispatch {
        app_id: String,
        method: String,
        payload: serde_json::Value,
    },
    ApprovalsList,
    ApprovalApprove {
        approval_id: String,
    },
    ApprovalReject {
        approval_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeResponse {
    pub id: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<NativeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<NativeErrorBody>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NativeResult {
    Pong,
    Status {
        worker_count: u64,
        socket_path: String,
        runtime_ready: bool,
    },
    Dispatch {
        payload: serde_json::Value,
    },
    DispatchError {
        code: String,
        message: String,
    },
    PendingApprovals {
        approvals: Vec<NativePendingApproval>,
    },
    ApprovalDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativePendingApproval {
    pub approval_id: String,
    pub app_id: String,
    pub capability: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeErrorCode {
    InvalidRequest,
    CoreUnavailable,
    CoreIpcError,
    UnexpectedCoreResponse,
}

impl std::fmt::Display for NativeErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::InvalidRequest => "invalid_request",
            Self::CoreUnavailable => "core_unavailable",
            Self::CoreIpcError => "core_ipc_error",
            Self::UnexpectedCoreResponse => "unexpected_core_response",
        };

        formatter.write_str(value)
    }
}

pub fn decode_request(bytes: &[u8]) -> Result<NativeRequest> {
    let request: NativeRequest = serde_json::from_slice(bytes)?;

    if request.id.is_empty() {
        return Err(NativeHostError::InvalidRequest(
            "missing request id".to_string(),
        ));
    }

    if let NativeCommand::Dispatch { app_id, method, .. } = &request.command {
        if app_id.is_empty() {
            return Err(NativeHostError::InvalidRequest(
                "dispatch app_id is empty".to_string(),
            ));
        }
        if method.is_empty() {
            return Err(NativeHostError::InvalidRequest(
                "dispatch method is empty".to_string(),
            ));
        }
    }

    if let NativeCommand::ApprovalApprove { approval_id }
    | NativeCommand::ApprovalReject { approval_id } = &request.command
    {
        if approval_id.is_empty() {
            return Err(NativeHostError::InvalidRequest(
                "approval_id is empty".to_string(),
            ));
        }
    }

    Ok(request)
}

pub fn extract_request_id(bytes: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    value
        .get("id")
        .and_then(|id| id.as_str())
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
}

pub fn success_response(id: impl Into<String>, result: NativeResult) -> NativeResponse {
    NativeResponse {
        id: Some(id.into()),
        ok: true,
        result: Some(result),
        error: None,
    }
}

pub fn error_response(
    id: Option<String>,
    code: impl ToString,
    message: impl Into<String>,
) -> NativeResponse {
    NativeResponse {
        id,
        ok: false,
        result: None,
        error: Some(NativeErrorBody {
            code: code.to_string(),
            message: message.into(),
        }),
    }
}
