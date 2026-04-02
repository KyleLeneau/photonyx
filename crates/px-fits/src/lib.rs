// Soon to be wrapper around https://github.com/cds-astro/fitsrs
// * Lazy data loading
// * pure rust
// * doesn't write images well but that's ok
// * can support wasm if all data loaded outside of wasm (fetch)

use fitsrs::{Fits, HDU, fits, hdu::header::extension::image::Image};
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
