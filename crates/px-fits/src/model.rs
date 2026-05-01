//! Standard model files used in Astronomy
//!

use std::path::PathBuf;

use chrono::NaiveDateTime;

use crate::Binning;

#[derive(Debug)]
pub struct MasterDark {
    pub source: PathBuf,
    pub path: PathBuf,
    pub temperature: f64,
    pub gain: i64,
    pub offset: i64,
    pub exposure: f64,
    pub binning: Binning,
    pub frame_count: usize,
    pub capture_date: NaiveDateTime, // TODO: rotation
}

#[derive(Debug)]
pub struct MasterBias {
    pub source: PathBuf,
    pub path: PathBuf,
    pub temperature: f64,
    pub gain: i64,
    pub offset: i64,
    pub binning: Binning,
    pub frame_count: usize,
    pub capture_date: NaiveDateTime, // TODO: rotation
}

#[derive(Debug)]
pub struct MasterFlat {
    pub source: PathBuf,
    pub path: PathBuf,
    pub temperature: f64,
    pub gain: i64,
    pub offset: i64,
    pub filter: String,
    pub binning: Binning,
    pub frame_count: usize,
    pub capture_date: NaiveDateTime, // TODO: rotation
}

#[derive(Debug)]
pub struct CalibratedLight {
    pub source: PathBuf,
    pub path: PathBuf,
    pub temperature: f64,
    pub gain: i64,
    pub offset: i64,
    pub exposure: f64,
    pub filter: String,
    pub binning: Binning,
    pub frame_count: usize,
    pub target_name: String,
    pub target_ra: Option<f64>,
    pub target_dec: Option<f64>,
    pub capture_date: NaiveDateTime,
    pub site_lat: Option<f64>,
    pub site_long: Option<f64>, // TODO: rotation
}

#[derive(Debug)]
pub struct MasterLight {
    pub sources: Vec<PathBuf>,
    pub path: PathBuf,
    pub exposure: f64,
    pub filter: String,
    pub binning: Binning,
    pub frame_count: usize,
    pub target_name: String,
    pub target_ra: Option<f64>,
    pub target_dec: Option<f64>,
}
