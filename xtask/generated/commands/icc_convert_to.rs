use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// icc_convert_to profile [intent]
/// ```
///
/// Converts the current image to the specified ICC profile.
/// One of the following special arguments may be provided to use the respective built-in profiles: **sRGB**, **sRGBlinear**, **Rec2020**, **Rec2020linear**, **graysrgb**, **grayrec2020**, **graylinear** or **working** to set the working mono or RGB color profile, (for mono images only) **linear**, or the path to an ICC profile file may be provided. If a built-in profile is specified with a monochrome image loaded, the Gray profile with the corresponding TRC will be used.
///
/// A second argument may be provided to specify the color transform intent: this should be one of **perceptual**, **relative** (for relative colorimetric), **saturation** or **absolute** (for absolute colorimetric)
///
#[derive(Builder)]
pub struct IccConvertTo {}

impl Command for IccConvertTo {
    fn name() -> &'static str {
        "icc_convert_to"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
