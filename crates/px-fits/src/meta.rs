use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, NaiveDateTime};
use px_fs::DatePath;

use crate::{Binning, FitsError, FitsFile, HeaderUtil};

#[derive(Debug)]
pub struct CalibrationMetadata {
    pub obs_date_utc: Option<DateTime<FixedOffset>>,
    pub obs_date_local: Option<NaiveDateTime>,
    pub exposure: Option<f64>,
    pub temperature: Option<f64>,
    pub filter: Option<String>,
    pub offset: Option<i64>,
    pub gain: Option<i64>,
    pub binning: Binning,
    // TODO: rotation
}

impl CalibrationMetadata {
    pub fn from(path: &Path) -> Result<Self, FitsError> {
        let file = FitsFile::new(path.to_path_buf())?;
        let header = file.primary_hdu.get_header();

        Ok(Self {
            obs_date_local: path.with_date_time(),
            obs_date_utc: header.get_date_utc("DATE-OBS"),
            exposure: header
                .get_float("EXPOSURE")
                .or_else(|| header.get_float("EXPTIME")),
            temperature: header.get_float("SET-TEMP"),
            filter: header.get_string("FILTER"),
            offset: header.get_int("OFFSET"),
            gain: header.get_int("GAIN"),
            binning: header.get_binning(),
        })
    }

    /// Returns the capture date from the file path or folder
    ///
    pub fn capture_date(&self) -> Option<NaiveDateTime> {
        self.obs_date_local
            .or_else(|| self.obs_date_utc.map(|dt| dt.naive_local()))
    }

    /// Returns a formatted master bias name that matches this metadata
    ///
    pub fn master_bias_name(&self) -> String {
        let date = self.capture_date().unwrap().format("%Y-%m-%d").to_string();
        format!(
            "{date}_BIAS_master_{}C",
            self.temperature.unwrap_or_default()
        )
    }

    /// Returns a formatted master dark name that matches this metadata
    ///
    pub fn master_dark_name(&self) -> String {
        let date = self.capture_date().unwrap().format("%Y-%m-%d").to_string();
        format!(
            "{date}_DARK_master_{}s_{}C",
            self.exposure.unwrap_or_default(),
            self.temperature.unwrap_or_default()
        )
    }

    /// Returns a formatted master flat name that matches this metadata
    ///
    pub fn master_flat_name(&self, filter: String) -> String {
        let date = self.capture_date().unwrap().format("%Y-%m-%d").to_string();
        format!(
            "{date}_FLAT_master_{}C_{}",
            self.temperature.unwrap_or_default(),
            self.filter.clone().unwrap_or(filter)
        )
    }
}

#[derive(Debug)]
pub struct ObservationMetadata {
    pub obs_date_utc: Option<DateTime<FixedOffset>>,
    pub obs_date_local: Option<NaiveDateTime>,
    pub exposure: Option<f64>,
    pub temperature: Option<f64>,
    pub filter: Option<String>,
    pub offset: Option<i64>,
    pub gain: Option<i64>,
    pub binning: Binning,
    pub frame_count: usize,
    pub target_name: String,
    pub target_ra: Option<f64>,
    pub target_dec: Option<f64>,
    pub site_lat: Option<f64>,
    pub site_long: Option<f64>,
    // TODO: rotation
}

impl ObservationMetadata {
    pub fn from(paths: Vec<PathBuf>) -> Result<Self, FitsError> {
        let first = paths.first().expect("missing first pp_ file");

        let file = FitsFile::new(first.to_path_buf())?;
        let header = file.primary_hdu.get_header();

        Ok(Self {
            obs_date_local: first.with_date_time(),
            obs_date_utc: header.get_date_utc("DATE-OBS"),
            exposure: header
                .get_float("EXPOSURE")
                .or_else(|| header.get_float("EXPTIME")),
            temperature: header.get_float("SET-TEMP"),
            filter: header.get_string("FILTER"),
            offset: header.get_int("OFFSET"),
            gain: header.get_int("GAIN"),
            binning: header.get_binning(),
            frame_count: paths.len(),
            target_name: header.get_string("OBJECT").unwrap_or_default(),
            target_ra: header.get_float("RA"),
            target_dec: header.get_float("DEC"),
            site_lat: header.get_float("SITELAT"),
            site_long: header.get_float("SITELONG"),
        })
    }

    /// Returns the capture date from the file path or folder
    ///
    pub fn capture_date(&self) -> Option<NaiveDateTime> {
        self.obs_date_local
            .or_else(|| self.obs_date_utc.map(|dt| dt.naive_local()))
    }
}

#[derive(Debug)]
pub struct LinearStackMetadata {
    pub total_exposure: Option<f64>,
    pub filter: Option<String>,
    pub binning: Binning,
    pub frame_count: i64,
    pub target_name: String,
    pub target_ra: Option<f64>,
    pub target_dec: Option<f64>,
}

impl LinearStackMetadata {
    pub fn from(path: PathBuf) -> Result<Self, FitsError> {
        let file = FitsFile::new(path.to_path_buf())?;
        let header = file.primary_hdu.get_header();

        Ok(Self {
            total_exposure: header.get_float("LIVETIME"),
            filter: header.get_string("FILTER"),
            binning: header.get_binning(),
            frame_count: header.get_int("STACKCNT").unwrap_or_default(),
            target_name: header.get_string("OBJECT").unwrap_or_default(),
            target_ra: header.get_float("RA"),
            target_dec: header.get_float("DEC"),
        })
    }
}
