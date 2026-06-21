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

    #[error("protocol error: {0}")]
    Protocol(#[from] kunkka_protocol::ProtocolError),

    #[error("worker sdk error: {0}")]
    WorkerSdk(#[from] kunkka_worker_sdk::WorkerSdkError),

    #[error("invalid worker frame: {0}")]
    InvalidWorkerFrame(String),

    #[error("invalid core frame: {0}")]
    InvalidCoreFrame(String),

    #[error("app not found: {0}")]
    AppNotFound(String),

    #[error("manifest invalid: {0}")]
    ManifestInvalid(String),

    #[error("worker start failed: {0}")]
    WorkerStartFailed(String),

    #[error("worker start timeout: {0}")]
    WorkerStartTimeout(String),

    #[error("worker unavailable: {0}")]
    WorkerUnavailable(String),

    #[error("dispatch ipc error: {0}")]
    DispatchIpcError(String),

    #[error("unexpected worker response: {0}")]
    UnexpectedWorkerResponse(String),

    #[error("database error: {0}")]
    Database(String),

    #[error("config error: {0}")]
    Config(String),
}
