//! The native `px-fits` error type (ADR 006 P1-T6).
//!
//! This lives alongside, not yet in place of, the `fitsrs`-based `FitsError`
//! still defined at the crate root — that cutover happens in Phase 2 when
//! `FitsFile` is rewritten on top of the native reader. Until then this is
//! addressed as `px_fits::error::FitsError` and used by the new modules
//! (`header`, and later `hdu`/`image`/`table`) that don't depend on
//! `fitsrs` at all.

/// Errors produced by the native FITS reader/writer.
#[derive(thiserror::Error, Debug)]
pub enum FitsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("missing primary HDU")]
    MissingPrimaryHdu,

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

    #[error("region {0:?} is out of bounds for image shape {1:?}")]
    RegionOutOfBounds(String, Vec<u64>),

    #[error("unsupported compression type: {0}")]
    UnsupportedCompression(String),

    #[error("image processing error: {0}")]
    Processing(String),

    #[error("unknown fits error")]
    Unknown,
}
