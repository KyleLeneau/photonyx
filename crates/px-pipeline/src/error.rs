use px_fits::FitsError;
use siril_sys::SirilError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Siril error: {0}")]
    SirilError(#[from] SirilError),

    #[error("fits error: {0}")]
    FitsError(#[from] FitsError),

    #[error("file(s) not found: {0}")]
    FileNotFound(String),

    #[error("Output file(s) not found: {0}")]
    OutputFileNotFound(String),
}
