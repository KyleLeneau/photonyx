use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// fftd modulus phase
/// ```
///
/// Applies a Fast Fourier Transform to the loaded image. **modulus** and **phase** given in argument are the names of the saved in FITS files
///
#[derive(Builder)]
pub struct Fftd {}

impl Command for Fftd {
    fn name() -> &'static str {
        "fftd"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
