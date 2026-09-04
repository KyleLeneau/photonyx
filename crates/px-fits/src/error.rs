//! The `px-fits` error type (ADR 006). Re-exported at the crate root as
//! `px_fits::FitsError` — the same path consumer crates have always used —
//! so the Phase 2 cutover from `fitsrs` didn't require touching any
//! consumer's `use` statements or error-handling code.

/// Errors produced by `px-fits`.
#[derive(thiserror::Error, Debug)]
pub enum FitsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("missing primary HDU")]
    MissingPrimaryHdu,

    #[error("HDU index {0} does not exist in this file")]
    HduIndexOutOfRange(usize),

    #[error("header has no END card within the scanned data")]
    MissingEnd,

    #[error("invalid BITPIX value: {0} (must be one of 8, 16, 32, 64, -32, -64)")]
    InvalidBitpix(i64),

    #[error("NAXIS{axis} is negative: {value}")]
    NegativeNaxis { axis: usize, value: i64 },

    #[error("NAXIS dimensions overflow when computing total element count")]
    NaxisOverflow,

    #[error("declared data size exceeds available source length")]
    DataSizeExceedsSource,

    #[error("HDU {index} is not an image (kind: {kind})")]
    NotAnImage { index: usize, kind: String },

    #[error("buffer length {got} does not match expected {expected}")]
    BufferLenMismatch { expected: usize, got: usize },

    #[error("region {0:?} is out of bounds for image shape {1:?}")]
    RegionOutOfBounds(String, Vec<u64>),

    #[error("unsupported compression type: {0}")]
    UnsupportedCompression(String),

    #[error("image processing error: {0}")]
    Processing(String),

    #[error("unknown fits error")]
    Unknown,
}
