use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savepnm filename
/// ```
///
/// Saves current image under the form of a NetPBM file format with 16-bit per channel.
///
/// The extension of the output will be **filename**.ppm for RGB image and **filename**.pgm for gray-level image
///
#[derive(Builder)]
pub struct Savepnm {}

impl Command for Savepnm {
    fn name() -> &'static str {
        "savepnm"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
