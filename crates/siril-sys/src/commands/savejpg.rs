use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savejpg filename [quality]
/// ```
///
/// Saves current image into a JPG file: **filename**.jpg.
///
/// The compression quality can be adjusted using the optional **quality** value, 100 being the best and default, while a lower value increases the compression ratio
///
#[derive(Builder)]
pub struct Savejpg {
    // #[builder(start_fn, into)]
    // filename: String,
}

impl Command for Savejpg {
    fn name() -> &'static str {
        "savejpg"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
