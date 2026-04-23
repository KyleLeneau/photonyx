//! Standard model files used in Astronomy
//!

use std::path::PathBuf;

use crate::Binning;

#[derive(Debug)]
pub struct MasterDark {
    pub path: PathBuf,
    pub temperature: f64,
    pub gain: i64,
    pub offset: i64,
    pub exposure: f64,
    pub binning: Binning, // TODO: rotation
}

#[derive(Debug)]
pub struct MasterBias {
    pub path: PathBuf,
    pub temperature: f64,
    pub gain: i64,
    pub offset: i64,
    pub binning: Binning, // TODO: rotation
}

#[derive(Debug)]
pub struct MasterFlat {
    pub path: PathBuf,
    pub temperature: f64,
    pub gain: i64,
    pub offset: i64,
    pub filter: String,
    pub binning: Binning, // TODO: rotation

}
