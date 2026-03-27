use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savebmp filename
/// ```
///
/// Saves current image under the form of a bitmap file with 8-bit per channel: **filename**.bmp (BMP 24-bit)
///
#[derive(Builder)]
pub struct Savebmp {}

impl Command for Savebmp {
    fn name() -> &'static str {
        "savebmp"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
