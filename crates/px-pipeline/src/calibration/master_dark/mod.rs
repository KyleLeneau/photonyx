//! Creating a master dark from a set of raw dark frames.
//!
//! A dark is a full-length exposure with the shutter closed, so it holds the
//! bias pedestal plus whatever thermal signal accumulated over that exposure.
//! Because it contains the bias, a light calibrated with this master should
//! *not* also have a master bias subtracted.
//!
//! The pipeline itself lives here; its execution lives in the sibling
//! modules, one per run environment (ADR 007 D5).

pub mod native;
pub mod siril;

use siril_sys::FitsExt;
use std::path::PathBuf;

#[derive(bon::Builder)]
pub struct CreateMasterDarkPipeline {
    pub ext: FitsExt,
    pub raw_folder: PathBuf,
    pub out_folder: PathBuf,
}
