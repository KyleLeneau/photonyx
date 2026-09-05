//! Pixel-space image processing for FITS data (ADR 006, Phase 9).
//!
//! `px-fits` is a pure format crate: it reads and writes FITS bytes with no
//! opinion on what the pixels mean photographically. This crate sits on top
//! of it and owns everything that interprets pixel values — debayering,
//! display normalization, autostretch, and (eventually) calibration,
//! stacking, and composition.
//!
//! Currently empty: scaffolding for ADR 006 Phase 9 (P9-T1). See
//! `docs/adr/006-native-fits-implementation.md` for the phased plan.

pub mod error;

pub use error::ImageProcError;
