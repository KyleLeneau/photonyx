//! Pixel-space image processing for FITS data (ADR 006, Phase 9).
//!
//! `px-fits` is a pure format crate: it reads and writes FITS bytes with no
//! opinion on what the pixels mean photographically. This crate sits on top
//! of it and owns everything that interprets pixel values — debayering,
//! display normalization, autostretch, and (eventually) calibration,
//! stacking, and composition.
//!
//! Currently: the preview pipeline behind [`decode_preview`], ported from
//! `astroimage`. See `docs/adr/006-native-fits-implementation.md` for the
//! phased plan.

pub mod bayer;
pub mod binning;
pub mod color;
pub mod debayer;
pub mod downscale;
pub mod error;
pub mod preview;
pub mod stretch;

pub use bayer::BayerPattern;
pub use error::ImageProcError;
pub use preview::{MAX_DISPLAY_DIM, PreviewImage, decode_preview};
