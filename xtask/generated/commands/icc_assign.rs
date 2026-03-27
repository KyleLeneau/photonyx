use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// icc_assign profile
/// ```
///
/// Assigns the ICC profile specified in the argument to the current image.
/// One of the following special arguments may be provided to use the respective built-in profiles: **sRGB**, **sRGBlinear**, **Rec2020**, **Rec2020linear**, **working** to set the working mono or RGB color profile, (for mono images only) **linear**, or the path to an ICC profile file may be provided. If a built-in profile is specified with a monochrome image loaded, the Gray profile with the corresponding TRC will be used
///
#[derive(Builder)]
pub struct IccAssign {}

impl Command for IccAssign {
    fn name() -> &'static str {
        "icc_assign"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
