//! Error types for SFX Root application.

use thiserror::Error;

/// Application-wide error type.
#[derive(Error, Debug)]
pub enum SfxError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database not open")]
    DatabaseNotOpen,

    #[error("Database not found at {0}")]
    DatabaseNotFound(String),

    #[error("Directory not found: {0}")]
    DirectoryNotFound(String),

    #[error("Directory already indexed: {0}")]
    DirectoryAlreadyIndexed(String),

    #[error("Indexing cancelled")]
    IndexingCancelled,

    #[error("Audio parsing error: {0}")]
    AudioParse(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

// Implement Serialize for Tauri command error responses
impl serde::Serialize for SfxError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Result type alias for SFX operations.
pub type Result<T> = std::result::Result<T, SfxError>;
