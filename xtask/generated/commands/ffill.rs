use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// ffill value [x y width height]
/// ```
///
/// Same command as FILL but this is a symmetric fill of a region defined by the mouse or with BOXSELECT. Used to process an image in the Fourier (FFT) domain
///
/// Links: :ref:`fill <fill>`, :ref:`boxselect <boxselect>`
///
#[derive(Builder)]
pub struct Ffill {}

impl Command for Ffill {
    fn name() -> &'static str {
        "ffill"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
