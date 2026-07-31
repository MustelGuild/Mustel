use thiserror::Error;

/// Core error type for Mustel application.
#[derive(Error, Debug)]
pub enum MustelError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Security error: {0}")]
    Security(String),

    #[error("Query execution error: {0}")]
    Query(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(String),

    #[allow(dead_code)]
    #[error("SQL parsing error: {0}")]
    SqlParse(String),

    #[error("Cancelled by user")]
    UserCancelled,

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, MustelError>;
