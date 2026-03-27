use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// fmedian ksize modulation
/// ```
///
/// Performs a median filter of size **ksize** x **ksize** (**ksize** MUST be odd) to the loaded image with a modulation parameter **modulation**.
///
/// The output pixel is computed as : out=mod x m + (1 − mod) x in, where m is the median-filtered pixel value. A modulation's value of 1 will apply no modulation
///
#[derive(Builder)]
pub struct Fmedian {}

impl Command for Fmedian {
    fn name() -> &'static str {
        "fmedian"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
