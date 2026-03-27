use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// clahe cliplimit tileSize
/// ```
///
/// Equalizes the histogram of an image using Contrast Limited Adaptive Histogram Equalization.
///
/// **cliplimit** sets the threshold for contrast limiting.
/// **tilesize** sets the size of grid for histogram equalization. Input image will be divided into equally sized rectangular tiles
///
#[derive(Builder)]
pub struct Clahe {}

impl Command for Clahe {
    fn name() -> &'static str {
        "clahe"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
