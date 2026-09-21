//! Calibrating an observation set against master frames, writing every
//! calibrated frame to an output folder.
//!
//! The pipeline itself lives here; its execution lives in the sibling
//! modules, one per run environment (ADR 007 D5).

pub mod native;
pub mod siril;

use siril_sys::FitsExt;
use std::path::PathBuf;

#[derive(bon::Builder)]
pub struct CalibrateLightSetPipeline {
    pub ext: FitsExt,
    pub raw_folder: PathBuf,
    pub bias: Option<PathBuf>,
    pub dark: Option<PathBuf>,
    pub flat: Option<PathBuf>,
    pub out_folder: PathBuf,
}
