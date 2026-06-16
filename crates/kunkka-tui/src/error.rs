use std::fmt;

#[derive(Debug)]
pub enum TuiError {
    CoreUnavailable(String),
    CoreIpc(String),
    UnexpectedCoreResponse(String),
}

impl fmt::Display for TuiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TuiError::CoreUnavailable(msg) => write!(f, "core unavailable: {msg}"),
            TuiError::CoreIpc(msg) => write!(f, "core IPC error: {msg}"),
            TuiError::UnexpectedCoreResponse(msg) => write!(f, "unexpected response: {msg}"),
        }
    }
}

impl std::error::Error for TuiError {}
