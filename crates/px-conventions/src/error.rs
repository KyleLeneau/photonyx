use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConventionsError {
    #[error("convention not found")]
    NotFound,

    #[error("invalid path format: {0}")]
    InvalidFormat(String),

    #[error("project already exists at `{0}`")]
    AlreadyExists(PathBuf),

    #[error("failed to read fs convention: {0}")]
    Io(#[from] std::io::Error),
}
