use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// extract_Ha [-upscale]
/// ```
///
/// Extracts H-Alpha signal from the loaded CFA image. It reads the Bayer matrix information from the image or the preferences and exports only the red filter data as a new half-sized FITS file. If the argument **-upscale** is provided, the output will be upscaled x2 to match the full sensor resolution, for example to match other images produced by the same family of sensors. A new file is created, its name is prefixed with "Ha\_"
///
#[derive(Builder)]
pub struct ExtractHa {}

impl Command for ExtractHa {
    fn name() -> &'static str {
        "extract_Ha"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
