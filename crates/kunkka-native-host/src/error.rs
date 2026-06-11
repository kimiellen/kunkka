use crate::native_protocol::NativeErrorCode;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, NativeHostError>;

#[derive(Debug, Error)]
pub enum NativeHostError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("core unavailable: {0}")]
    CoreUnavailable(String),

    #[error("core ipc error: {0}")]
    CoreIpc(String),

    #[error("unexpected core response: {0}")]
    UnexpectedCoreResponse(String),
}

impl NativeHostError {
    pub fn code(&self) -> NativeErrorCode {
        match self {
            Self::InvalidRequest(_) | Self::Json(_) | Self::Io(_) => {
                NativeErrorCode::InvalidRequest
            }
            Self::CoreUnavailable(_) => NativeErrorCode::CoreUnavailable,
            Self::CoreIpc(_) => NativeErrorCode::CoreIpcError,
            Self::UnexpectedCoreResponse(_) => NativeErrorCode::UnexpectedCoreResponse,
        }
    }
}
