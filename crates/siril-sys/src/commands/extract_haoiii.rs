use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// extract_HaOIII [-resample=]
/// ```
///
/// Extracts H-Alpha and O-III signals from the loaded CFA image. It reads the Bayer matrix information from the image or the preferences and exports only the red filter data for H-Alpha as a new half-sized FITS file (like EXTRACTHA) and keeps the three others for O-III with an interpolated replacement for the red pixel. The output files names start with the prefix "Ha\_" and "OIII\_"
///
/// The optional argument **-resample={ha|oiii}** sets whether to upsample the Ha image or downsample the OIII image to have images the same size. If this argument is not provided, no resampling will be carried out and the OIII image will have twice the height and width of the Ha image
///
#[derive(Builder)]
pub struct ExtractHaOIII {}

impl Command for ExtractHaOIII {
    fn name() -> &'static str {
        "extract_HaOIII"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
