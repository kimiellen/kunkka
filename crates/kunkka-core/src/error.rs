use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("HOME is missing or is not an absolute path")]
    MissingHome,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ipc error: {0}")]
    Ipc(#[from] kunkka_ipc::IpcError),
}
