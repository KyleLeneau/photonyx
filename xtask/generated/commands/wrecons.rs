use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// wrecons c1 c2 c3 ...
/// ```
///
/// Reconstructs to current image from the layers previously computed with wavelets and weighted with coefficients **c1**, **c2**, ..., **cn** according to the number of layers used for wavelet transform, after the use of WAVELET
///
/// Links: :ref:`wavelet <wavelet>`
///
#[derive(Builder)]
pub struct Wrecons {}

impl Command for Wrecons {
    fn name() -> &'static str {
        "wrecons"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
