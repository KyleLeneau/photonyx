use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// rgbcomp red green blue [-out=result_filename] [-nosum]
/// rgbcomp -lum=image { rgb_image | red green blue } [-out=result_filename] [-nosum]
/// ```
///
/// Creates an RGB composition using three independent images, or an LRGB composition using the optional luminance image and three monochrome images or a color image. Result image is called composed_rgb.fit or composed_lrgb.fit unless another name is provided in the optional argument. Another optional argument, **-nosum** tells Siril not to sum exposure times. This impacts FITS keywords such as LIVETIME and STACKCNT
///
#[derive(Builder)]
pub struct Rgbcomp {}

impl Command for Rgbcomp {
    fn name() -> &'static str {
        "rgbcomp"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
