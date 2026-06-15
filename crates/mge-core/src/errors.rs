use thiserror::Error;

pub type Result<T> = std::result::Result<T, MgeError>;

#[derive(Debug, Error)]
pub enum MgeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("messagepack encode error: {0}")]
    MessagePackEncode(#[from] rmp_serde::encode::Error),

    #[error("messagepack decode error: {0}")]
    MessagePackDecode(#[from] rmp_serde::decode::Error),

    #[error("storage format error: {0}")]
    StorageFormat(String),

    #[error("invalid marker: {0}")]
    InvalidMarker(String),

    #[error("store is not initialized: {0}")]
    NotInitialized(String),

    #[error("store is locked: {0}")]
    StoreLocked(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),
}
