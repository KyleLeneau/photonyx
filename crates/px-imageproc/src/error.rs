//! The `px-imageproc` error type.

/// Errors produced by `px-imageproc`.
#[derive(thiserror::Error, Debug)]
pub enum ImageProcError {
    #[error("FITS error: {0}")]
    Fits(#[from] px_fits::FitsError),

    #[error("image processing error: {0}")]
    Processing(String),

    /// The configured memory budget cannot hold what the stage needs. Raised
    /// rather than overrunning the budget (ADR 007 performance targets).
    #[error("memory budget too small to hold {what}")]
    InsufficientMemory { what: String },
}
