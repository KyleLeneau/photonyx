pub mod block;
pub mod card;
pub mod display;
pub mod error;
pub mod hdu;
pub mod header;
pub mod image;
pub mod reader;
pub mod source;
pub mod table;
pub mod writer;

pub use error::FitsError;
pub use header::BitPix;
pub use image::{ImageHdu, Pixel, Region, Scaling};
pub use table::{AsciiTableHdu, BinTableHdu, Cell, ColumnDef};
pub use writer::{CardEdit, FitsWriter, HeaderBuilder, ImageWriter, update_header};

use std::fmt::Display;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};

use crate::card::Value;
use crate::hdu::DiscoveredHdu;
use crate::header::Header;
use crate::reader::FitsReader;

/// The primary HDU of an opened [`FitsFile`]: its header plus data-unit
/// byte-range metadata. A thin wrapper over [`DiscoveredHdu`] so
/// `FitsFile::primary_hdu` has a stable, native type in place of the
/// `fitsrs`-specific one it used to be (ADR 006 Phase 2 cutover).
pub struct PrimaryHdu(DiscoveredHdu);

impl PrimaryHdu {
    pub fn get_header(&self) -> &Header {
        &self.0.header
    }

    /// Byte offset of the data unit within the file.
    pub fn get_data_unit_byte_offset(&self) -> u64 {
        self.0.data_offset
    }

    /// On-disk size of the data unit, including block padding.
    pub fn get_data_unit_byte_size(&self) -> u64 {
        self.0.data_padded_len
    }
}

pub struct FitsFile {
    #[allow(dead_code)]
    pub file_path: PathBuf,
    pub primary_hdu: PrimaryHdu,
}

impl FitsFile {
    pub fn new(path: PathBuf) -> Result<Self, FitsError> {
        let reader = FitsReader::open(&path)?;
        let primary = reader.primary()?;
        Ok(Self {
            file_path: path,
            primary_hdu: PrimaryHdu(primary),
        })
    }

    pub fn is_color(&self) -> bool {
        let header = self.primary_hdu.get_header();

        let bayer = header
            .get_string("BAYERPAT")
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        let three_dim = header.naxis().map(|n| n.len() > 2).unwrap_or(false);

        bayer || three_dim
    }

    pub fn headers(&self) -> Vec<String> {
        self.primary_hdu
            .get_header()
            .cards()
            .iter()
            .map(|c| c.keyword.clone())
            .collect()
    }

    /// All cards in file order. The `fitsrs`-specific `ValueMapIter` this
    /// used to return no longer makes sense once `fitsrs` is gone from the
    /// runtime dependency graph; nothing outside this crate depended on
    /// that concrete type (only on iterating key/value pairs), so this
    /// returns the native `Card` sequence instead.
    pub fn key_values(&self) -> impl Iterator<Item = &card::Card> {
        self.primary_hdu.get_header().cards().iter()
    }

    pub fn header_rows(&self) -> Vec<(String, String, String)> {
        self.primary_hdu
            .get_header()
            .cards()
            .iter()
            .map(|c| {
                let val_str = match &c.value {
                    Value::Integer(i) => i.to_string(),
                    Value::Float(f) => format!("{f}"),
                    Value::Logical(b) => (if *b { "T" } else { "F" }).to_string(),
                    Value::String(s) => s.clone(),
                    Value::Complex(re, im) => format!("({re}, {im})"),
                    Value::Undefined => "undefined".to_string(),
                    Value::Commentary(s) => s.clone(),
                    Value::Invalid(raw) => raw.clone(),
                };
                let comment = c.comment.clone().unwrap_or_default();
                (c.keyword.clone(), val_str, comment)
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
        let naxis = header.naxis().unwrap_or_default();
        write!(
            f,
            "PRIMARY: HEAD naxis: {:?}; bitpix: {:?}; dimensions: {}; start byte: {}; byte size: {}.",
            naxis,
            header.bitpix(),
            naxis
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
/// Walks the directory once (case-insensitive extension match) and returns
/// entries sorted by path. Earlier versions walked the directory once per
/// extension, which produced an extension-grouped, filesystem-order result
/// that callers relying on `paths.first()` for "the earliest frame" (e.g.
/// `ObservationMetadata::from`) could not depend on.
pub fn all_fits_files(raw_folder: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in raw_folder.read_dir()? {
        let path = entry?.path();
        let is_fits = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("fit") || e.eq_ignore_ascii_case("fits"))
            .unwrap_or(false);
        if is_fits {
            files.push(path);
        }
    }
    files.sort();
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

/// Source-compatibility shim for the pre-Phase-2 `HeaderUtil` trait.
/// `header::Header`'s own inherent methods (`get_i64`, `get_f64`,
/// `get_date_utc`, ...) are the native API; this trait exists only so
/// existing consumer call sites (`px-pipeline`) that spell out
/// `get_float`/`get_int`/`get_binning` keep compiling unchanged.
pub trait HeaderUtil {
    fn get_string(&self, key: &str) -> Option<String>;
    fn get_float(&self, key: &str) -> Option<f64>;
    fn get_int(&self, key: &str) -> Option<i64>;
    fn get_date_utc(&self, key: &str) -> Option<DateTime<FixedOffset>>;
    fn get_binning(&self) -> Binning;
}

impl HeaderUtil for Header {
    fn get_string(&self, key: &str) -> Option<String> {
        Header::get_string(self, key)
    }

    fn get_float(&self, key: &str) -> Option<f64> {
        self.get_f64(key)
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        self.get_i64(key)
    }

    fn get_date_utc(&self, key: &str) -> Option<DateTime<FixedOffset>> {
        Header::get_date_utc(self, key)
    }

    fn get_binning(&self) -> Binning {
        Binning {
            x: self.get_i64("XBINNING").unwrap_or(1) as u8,
            y: self.get_i64("YBINNING").unwrap_or(1) as u8,
        }
    }
}
