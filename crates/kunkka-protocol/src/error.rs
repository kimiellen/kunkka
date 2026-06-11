use thiserror::Error;

pub type Result<T> = std::result::Result<T, ProtocolError>;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("codec error: {0}")]
    Codec(#[from] postcard::Error),
}
