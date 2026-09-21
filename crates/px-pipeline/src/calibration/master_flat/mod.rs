//! Creating a master flat from a set of raw flat frames.
//!
//! A flat records the optical system's per-pixel response — vignetting, dust,
//! filter transmission. Unlike bias and dark it is divided out rather than
//! subtracted, and the flats themselves must be bias-calibrated first, since
//! the pedestal is additive and would distort a multiplicative correction.
//!
//! The pipeline itself lives here; its execution lives in the sibling
//! modules, one per run environment (ADR 007 D5).

pub mod native;
pub mod siril;

use siril_sys::FitsExt;
use std::path::PathBuf;

#[derive(bon::Builder)]
pub struct CreateMasterFlatPipeline {
    pub ext: FitsExt,
    pub raw_folder: PathBuf,
    pub bias: PathBuf,
    pub filter: String,
    pub out_folder: PathBuf,
}
