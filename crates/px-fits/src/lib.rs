pub mod display;
mod meta;
mod model;

use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};
use fitsrs::hdu::header::Header;
pub use meta::*;
pub use model::*;

// Soon to be wrapper around https://github.com/cds-astro/fitsrs
// * Lazy data loading
// * pure rust
// * doesn't write images well but that's ok
// * can support wasm if all data loaded outside of wasm (fetch)

use fitsrs::card::Value;
use fitsrs::{
    Fits, HDU, fits,
    hdu::header::{ValueMapIter, extension::image::Image},
};
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

    #[error("unknown fits error")]
    UnKnown,

    #[error("image processing error: {0}")]
    Processing(String),
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
        let header = self.primary_hdu.get_header();

        let bayer = match header.get("BAYERPAT") {
            Some(Value::String { value, .. }) => !value.is_empty(),
            _ => false,
        };

        let three_dim = header.get_xtension().get_naxis().iter().count() > 2;

        bayer || three_dim
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

    pub fn filter(&self) -> Option<String> {
        self.header_rows()
            .into_iter()
            .find(|(key, _, _)| key == "FILTER")
            .map(|(_, value, _)| value)
            .filter(|v| !v.is_empty())
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

/// Utility to check if all the files in a path are .fit or .fits
///
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

/// Utility to check if all the files are fits files with 3 channels of Bayered images
///
pub fn all_color_raw_frames(raw_files: &Vec<PathBuf>) -> Result<bool, FitsError> {
    let mut all_color = true;
    for raw_file in raw_files {
        let file = FitsFile::new(raw_file.clone())?;
        if !file.is_color() {
            all_color = false;
            break;
        }
    }

    Ok(all_color)
}

#[allow(unused)]
#[derive(Debug, Clone, Copy)]
pub struct Binning {
    x: u8,
    y: u8,
}
impl Default for Binning {
    fn default() -> Self {
        Self { x: 1, y: 1 }
    }
}

impl std::fmt::Display for Binning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.x, self.y)
    }
}

pub(crate) trait HeaderUtil {
    fn get_string(&self, key: &str) -> Option<String>;
    fn get_float(&self, key: &str) -> Option<f64>;
    fn get_int(&self, key: &str) -> Option<i64>;
    fn get_date_utc(&self, key: &str) -> Option<DateTime<FixedOffset>>;
    fn get_binning(&self) -> Binning;
}

impl HeaderUtil for Header<Image> {
    fn get_string(&self, key: &str) -> Option<String> {
        match self.get(key)? {
            Value::String { value, .. } => Some(value.clone()),
            _ => None,
        }
    }

    fn get_float(&self, key: &str) -> Option<f64> {
        match self.get(key)? {
            Value::Float { value, .. } => Some(*value),
            Value::Integer { value, .. } => Some(*value as f64),
            _ => None,
        }
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        match self.get(key)? {
            Value::Integer { value, .. } => Some(*value),
            _ => None,
        }
    }

    fn get_date_utc(&self, key: &str) -> Option<DateTime<FixedOffset>> {
        self.get_string(key).and_then(|s| {
            DateTime::parse_from_rfc3339(&s).ok().or_else(|| {
                NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f")
                    .ok()
                    .map(|ndt| Utc.from_utc_datetime(&ndt).fixed_offset())
            })
        })
    }

    fn get_binning(&self) -> Binning {
        Binning {
            x: self.get_int("XBINNING").unwrap_or(1) as u8,
            y: self.get_int("YBINNING").unwrap_or(1) as u8,
        }
    }
}
