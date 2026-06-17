use crate::output::CliOutput;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("core unavailable: {0}")]
    CoreUnavailable(String),

    #[error("core ipc error: {0}")]
    CoreIpc(String),

    #[error("unexpected core response: {0}")]
    UnexpectedCoreResponse(String),

    #[error("core platform error: {code}: {message}")]
    CorePlatform { code: String, message: String },

    #[error("approval rejected by user")]
    ApprovalRejected,
}

impl CliError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::CoreUnavailable(_) => "core_unavailable",
            Self::CoreIpc(_) => "core_ipc_error",
            Self::UnexpectedCoreResponse(_) => "unexpected_core_response",
            Self::CorePlatform { .. } => "core_error",
            Self::ApprovalRejected => "approval_rejected",
        }
    }

    pub fn exit_code(&self) -> i32 {
        1
    }

    pub fn to_output(&self) -> CliOutput {
        match self {
            Self::CorePlatform { code, message } => CliOutput::error(code, message),
            _ => CliOutput::error(self.code(), self.to_string()),
        }
    }
}
