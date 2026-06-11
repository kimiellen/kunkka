use crate::{NativeHostError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NativeRequest {
    pub id: String,
    pub command: NativeCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCommand {
    Ping,
    Status,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeErrorBody {
    pub code: NativeErrorCode,
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
    code: NativeErrorCode,
    message: impl Into<String>,
) -> NativeResponse {
    NativeResponse {
        id,
        ok: false,
        result: None,
        error: Some(NativeErrorBody {
            code,
            message: message.into(),
        }),
    }
}
