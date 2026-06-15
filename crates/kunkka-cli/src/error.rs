use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("core unavailable: {0}")]
    CoreUnavailable(String),
}
