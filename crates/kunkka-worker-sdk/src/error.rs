use thiserror::Error;

pub type Result<T> = std::result::Result<T, WorkerSdkError>;

#[derive(Debug, Error)]
pub enum WorkerSdkError {
    #[error("ipc error: {0}")]
    Ipc(#[from] kunkka_ipc::IpcError),

    #[error("codec error: {0}")]
    Codec(#[from] postcard::Error),

    #[error("protocol error: {0}")]
    Protocol(String),
}
