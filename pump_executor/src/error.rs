use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),
    #[error("API request failed: {error_message}")]
    SkipError{error_message: String},
    #[error("Blockchain interaction failed: {0}")]
    Chain(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON deserialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Denom mismatch: expected {expected}, got {got}")]
    DenomMismatch { expected: String, got: String },
}
