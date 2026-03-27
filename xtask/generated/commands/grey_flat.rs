use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// grey_flat
/// ```
///
/// Equalizes the mean intensity of RGB layers in the loaded CFA image. This is the same process used on flats during calibration when the option equalize CFA is used
///
#[derive(Builder)]
pub struct GreyFlat {}

impl Command for GreyFlat {
    fn name() -> &'static str {
        "grey_flat"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
