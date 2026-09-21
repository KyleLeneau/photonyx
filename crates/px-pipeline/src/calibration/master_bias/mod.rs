//! Creating a master bias from a set of raw bias frames.
//!
//! A bias frame is a zero-length exposure: it captures the sensor's read-out
//! pedestal and read noise and nothing else. Median-stacking many of them
//! averages the noise away and leaves the pedestal, which every other frame
//! from that sensor carries and which must come off before anything else.
//!
//! The pipeline itself lives here; its execution lives in the sibling
//! modules, one per run environment (ADR 007 D5).

pub mod native;
pub mod siril;

use siril_sys::FitsExt;
use std::path::PathBuf;

#[derive(bon::Builder)]
pub struct CreateMasterBiasPipeline {
    pub ext: FitsExt,
    pub raw_folder: PathBuf,
    pub out_folder: PathBuf,
}
