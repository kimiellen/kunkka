use thiserror::Error;

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("codec error: {0}")]
    Codec(#[from] postcard::Error),

    #[error("connection closed")]
    ConnectionClosed,

    #[error("frame too large: {size} bytes exceeds max {max} bytes")]
    FrameTooLarge { size: usize, max: usize },
}
