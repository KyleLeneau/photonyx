use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// wavelet nbr_layers type
/// ```
///
/// Computes the wavelet transform of the loaded image on (**nbr_layers**\ =1...n) layer(s) using linear (**type**\ =1) or bspline (**type**\ =2) version of the 'à trous' algorithm. The result is stored in a file as a structure containing the layers, ready for weighted reconstruction with WRECONS.
///
/// See also EXTRACT
///
/// Links: :ref:`wrecons <wrecons>`, :ref:`extract <extract>`
///
#[derive(Builder)]
pub struct Wavelet {}

impl Command for Wavelet {
    fn name() -> &'static str {
        "wavelet"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
