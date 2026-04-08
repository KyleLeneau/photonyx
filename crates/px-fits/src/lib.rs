// Soon to be wrapper around https://github.com/cds-astro/fitsrs
// * Lazy data loading
// * pure rust
// * doesn't write images well but that's ok
// * can support wasm if all data loaded outside of wasm (fetch)

use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};
use fitsrs::card::Value;
use fitsrs::{
    Fits, HDU, fits,
    hdu::header::{ValueMapIter, extension::image::Image},
};
use regex::Regex;
use std::io;
use std::path::Path;
use std::{
    fmt::{Debug, Display},
    fs::File,
    io::BufReader,
    path::PathBuf,
};

#[derive(thiserror::Error, Debug)]
pub enum FitsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Fits error: {0}")]
    Internal(#[from] fitsrs::error::Error),

    #[error("Missing primary hdu")]
    MissingPrimaryHDU,
}

pub struct FitsFile {
    #[allow(dead_code)]
    file_path: PathBuf,
    primary_hdu: fits::HDU<Image>,
}

impl FitsFile {
    pub fn new(path: PathBuf) -> Result<Self, FitsError> {
        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let mut fits_reader = Fits::from_reader(reader);

        // Get the Primary HDU header
        let primary_hdu = match fits_reader.next().ok_or(FitsError::MissingPrimaryHDU)?? {
            HDU::Primary(img) => img,
            _ => return Err(FitsError::MissingPrimaryHDU),
        };

        Ok(Self {
            file_path: path,
            primary_hdu,
        })
    }

    pub fn is_color(&self) -> bool {
        self.primary_hdu
            .get_header()
            .get_xtension()
            .get_naxis()
            .iter()
            .count()
            > 2
    }

    pub fn headers(&self) -> Vec<String> {
        self.primary_hdu
            .get_header()
            .keywords()
            .map(|k| k.to_string())
            .collect()
    }

    pub fn key_values(&self) -> ValueMapIter<'_> {
        self.primary_hdu.get_header().iter()
    }

    pub fn header_rows(&self) -> Vec<(String, String, String)> {
        self.key_values()
            .map(|(key, value)| {
                let (val_str, comment) = match value {
                    Value::Integer { value, comment } => (
                        value.to_string(),
                        comment.as_deref().unwrap_or("").to_string(),
                    ),
                    Value::Float { value, comment } => (
                        format!("{value}"),
                        comment.as_deref().unwrap_or("").to_string(),
                    ),
                    Value::Logical { value, comment } => (
                        if *value { "T" } else { "F" }.to_string(),
                        comment.as_deref().unwrap_or("").to_string(),
                    ),
                    Value::String { value, comment } => {
                        (value.clone(), comment.as_deref().unwrap_or("").to_string())
                    }
                    Value::Undefined => ("undefined".to_string(), String::new()),
                    Value::Invalid(raw) => (raw.clone(), String::new()),
                };
                (key.to_string(), val_str, comment)
            })
            .collect()
    }
}

impl Display for FitsFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let header = self.primary_hdu.get_header();
        let img = header.get_xtension();
        write!(
            f,
            "PRIMARY: HEAD naxis: {:?}; bitpix: {:?}; dimensions: {}; start byte: {}; byte size: {}.",
            img.get_naxis(),
            img.get_bitpix(),
            img.get_naxis()
                .iter()
                .map(|d| d.to_string())
                .reduce(|mut s, d| {
                    s.push('x');
                    s.push_str(&d);
                    s
                })
                .unwrap_or_else(|| String::from("0")),
            self.primary_hdu.get_data_unit_byte_offset(),
            self.primary_hdu.get_data_unit_byte_size()
        )
    }
}

pub fn all_fits_files(raw_folder: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for ext in &["fit", "fits"] {
        for entry in raw_folder.read_dir()? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                files.push(path);
            }
        }
    }
    Ok(files)
}

pub fn all_color_raw_frames(raw_files: &Vec<PathBuf>) -> Result<bool, FitsError> {
    let mut all_color = true;
    for raw_file in raw_files {
        if !FitsFile::new(raw_file.clone())?.is_color() {
            all_color = false;
            break;
        }
    }

    Ok(all_color)
}

#[derive(Debug)]
pub struct CalibrationMetadata {
    obs_date_utc: Option<DateTime<FixedOffset>>,
    obs_date_local: Option<NaiveDateTime>,
    exposure: Option<f64>,
    temperature: Option<f64>,
    filter: Option<String>,
    _offset: Option<i64>,
    _gain: Option<i64>,
    // TODO: binning
}

impl CalibrationMetadata {
    pub fn from(path: &Path) -> Result<Self, FitsError> {
        let file = FitsFile::new(path.to_path_buf())?;
        let header = file.primary_hdu.get_header();

        let get_string = |key: &str| -> Option<String> {
            match header.get(key)? {
                Value::String { value, .. } => Some(value.clone()),
                _ => None,
            }
        };

        let get_float = |key: &str| -> Option<f64> {
            match header.get(key)? {
                Value::Float { value, .. } => Some(*value),
                Value::Integer { value, .. } => Some(*value as f64),
                _ => None,
            }
        };

        let get_int = |key: &str| -> Option<i64> {
            match header.get(key)? {
                Value::Integer { value, .. } => Some(*value),
                _ => None,
            }
        };

        let obs_date_local = path.file_stem().and_then(|s| s.to_str()).and_then(|stem| {
            Regex::new(r"(\d{8}-\d{6})")
                .ok()?
                .captures(stem)
                .and_then(|caps| NaiveDateTime::parse_from_str(&caps[1], "%Y%m%d-%H%M%S").ok())
        });

        Ok(Self {
            obs_date_local,
            obs_date_utc: get_string("DATE-OBS").and_then(|s| {
                DateTime::parse_from_rfc3339(&s).ok().or_else(|| {
                    NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f")
                        .ok()
                        .map(|ndt| Utc.from_utc_datetime(&ndt).fixed_offset())
                })
            }),
            exposure: get_float("EXPOSURE"),
            temperature: get_float("SET-TEMP"),
            filter: get_string("FILTER"),
            _offset: get_int("OFFSET"),
            _gain: get_int("GAIN"),
        })
    }

    fn best_obs_date(&self) -> Option<NaiveDateTime> {
        self.obs_date_local
            .or_else(|| self.obs_date_utc.map(|dt| dt.naive_local()))
    }

    /// Returns a formatted master bias name that matches this metadata
    ///
    pub fn master_bias_name(&self) -> String {
        let date = self.best_obs_date().unwrap().format("%Y-%m-%d").to_string();
        format!(
            "{date}_BIAS_master_{}C",
            self.temperature.unwrap_or_default()
        )
    }

    /// Returns a formatted master dark name that matches this metadata
    ///
    pub fn master_dark_name(&self) -> String {
        let date = self.best_obs_date().unwrap().format("%Y-%m-%d").to_string();
        format!(
            "{date}_DARK_master_{}s_{}C",
            self.exposure.unwrap_or_default(),
            self.temperature.unwrap_or_default()
        )
    }

    /// Returns a formatted master flat name that matches this metadata
    ///
    pub fn master_flat_name(&self, filter: String) -> String {
        let date = self.best_obs_date().unwrap().format("%Y-%m-%d").to_string();
        format!(
            "{date}_FLAT_master_{}C_{}",
            self.temperature.unwrap_or_default(),
            self.filter.clone().unwrap_or(filter)
        )
    }
}
