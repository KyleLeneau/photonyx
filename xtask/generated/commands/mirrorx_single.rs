use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// mirrorx_single image
/// ```
///
/// Flips the image about the horizontal axis, only if needed (if it's not already bottom-up). It takes the image file name as argument, allowing it to avoid reading image data entirely if no flip is required. Image is overwritten if a flip is made
///
#[derive(Builder)]
pub struct MirrorxSingle {}

impl Command for MirrorxSingle {
    fn name() -> &'static str {
        "mirrorx_single"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
