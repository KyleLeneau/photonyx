use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savepng filename
/// ```
///
/// Saves current image into a PNG file: **filename**.png, with 16 bits per channel if the loaded image is 16 or 32 bits, and 8 bits per channel if the loaded image is 8 bits
///
#[derive(Builder)]
pub struct Savepng {}

impl Command for Savepng {
    fn name() -> &'static str {
        "savepng"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
