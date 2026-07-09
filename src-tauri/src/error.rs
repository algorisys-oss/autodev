use serde::Serialize;

/// Errors returned across the Tauri command boundary.
///
/// Every variant serializes to a plain string so the frontend receives a
/// readable message rather than an opaque object.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("could not resolve the home directory")]
    NoHomeDir,

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("pty error: {0}")]
    Pty(String),

    #[error("git error: {0}")]
    Git(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Result alias for anything that crosses a command boundary.
pub type AppResult<T> = Result<T, AppError>;
