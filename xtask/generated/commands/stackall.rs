use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// stackall
/// stackall { sum | min | max } [-maximize] [-upscale] [-32b]
/// stackall { med | median } [-nonorm, norm=] [-32b]
/// stackall { rej | mean } [rejection type] [sigma_low sigma_high] [-nonorm, norm=] [-overlap_norm] [-weight={noise|wfwhm|nbstars|nbstack}] [-feather=] [-rgb_equal] [-out=filename] [-maximize] [-upscale] [-32b]
/// ```
///
/// Opens all sequences in the current directory and stacks them with the optionally specified stacking type and filtering or with sum stacking. See STACK command for options description
///
/// Links: :ref:`stack <stack>`
///
#[derive(Builder)]
pub struct Stackall {}

impl Command for Stackall {
    fn name() -> &'static str {
        "stackall"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
