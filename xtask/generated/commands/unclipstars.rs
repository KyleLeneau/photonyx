use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// unclipstars
/// ```
///
/// Re-profiles clipped stars of the loaded image to desaturate them, scaling the output so that all pixel values are <= 1.0
///
#[derive(Builder)]
pub struct Unclipstars {}

impl Command for Unclipstars {
    fn name() -> &'static str {
        "unclipstars"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
