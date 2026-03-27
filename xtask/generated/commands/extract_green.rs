use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// extract_Green
/// ```
///
/// Extracts green signal from the loaded CFA image. It reads the Bayer matrix information from the image or the preferences and exports only the averaged green filter data as a new half-sized FITS file. A new file is created, its name is prefixed with "Green\_"
///
#[derive(Builder)]
pub struct ExtractGreen {}

impl Command for ExtractGreen {
    fn name() -> &'static str {
        "extract_Green"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
