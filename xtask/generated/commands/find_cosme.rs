use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// find_cosme cold_sigma hot_sigma
/// ```
///
/// Applies an automatic detection and replacement of cold and hot pixels in the loaded image, with the thresholds passed in arguments in sigma units
///
#[derive(Builder)]
pub struct FindCosme {}

impl Command for FindCosme {
    fn name() -> &'static str {
        "find_cosme"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
