#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Transcode error: {0}")]
    Transcode(String),

    #[error("Token error: {0}")]
    Token(#[from] crate::token::TokenError),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    Internal(String),
}

impl From<anyhow::Error> for MediaError {
    fn from(e: anyhow::Error) -> Self {
        MediaError::Internal(e.to_string())
    }
}
