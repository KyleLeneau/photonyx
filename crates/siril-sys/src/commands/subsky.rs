use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// subsky { -rbf | degree } [-dither] [-samples=20] [-tolerance=1.0] [-smooth=0.5] [-existing]
/// ```
///
/// Computes a synthetic background gradient using either the polynomial function model of **degree** degrees or the RBF model (if **-rbf** is provided instead) and subtracts it from the image.
/// The number of samples per horizontal line and the tolerance to exclude brighter areas can be adjusted with the optional arguments. Tolerance is in MAD units: median + tolerance \* mad.
/// Dithering, required for low dynamic gradients, can be enabled with **-dither**.
/// For RBF, the additional smoothing parameter is also available. To use pre-existing background samples (e.g. if you have set background samples using a Python script) the **-existing** argument must be used
///
#[derive(Builder)]
pub struct Subsky {}

impl Command for Subsky {
    fn name() -> &'static str {
        "subsky"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
